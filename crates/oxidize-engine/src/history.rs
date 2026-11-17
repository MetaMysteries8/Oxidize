//! Milestone 2 §3: the checkpoint's event history and how older checkpoints are upgraded.
//!
//! The history lives inside `state.json` and is written in the same atomic write as the state
//! change it describes. State and history therefore never disagree, even after a crash.
//!
//! Upgrade strategy: a checkpoint is read as raw JSON first and its `schema_version` decides
//! what happens next.
//! - The current version loads as is.
//! - An older version is upgraded one step at a time in memory, then validated. The original
//!   bytes are kept as `state.v<N>.json`, written atomically and never overwritten. Then the
//!   upgraded state replaces `state.json` atomically. If the process is interrupted in between,
//!   the next open finds the old state and an identical backup and simply upgrades again.
//! - A newer, unknown or malformed version is refused and nothing on disk is touched.
//!
//! An upgrade records one `schema_upgraded` event. History from before the upgrade was never
//! recorded, so none is invented.
use crate::{Phase, State};
use anyhow::{Context, Result, bail, ensure};
use oxidize_core::{atomic_json, atomic_write};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::Path};

/// The `state.json` layout this version writes. 1 = v0.1.x (no history), 2 = event history.
pub const CHECKPOINT_SCHEMA_VERSION: u32 = 2;
/// The layout of one history event, recorded in each event.
pub const EVENT_SCHEMA_VERSION: u32 = 1;

/// One entry in the checkpoint's history. `seq` counts from 1 with no gaps.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub seq: usize,
    pub version: u32,
    /// Wall-clock time the event was recorded, in milliseconds since the Unix epoch.
    pub unix_ms: u64,
    #[serde(flatten)]
    pub kind: EventKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum EventKind {
    /// A new session captured its baseline. Only ever the first event.
    SessionCreated,
    /// An older checkpoint was upgraded; `backup` is the kept original, next to `state.json`.
    SchemaUpgraded {
        from: u32,
        to: u32,
        backup: String,
    },
    AttemptStarted {
        attempt: usize,
        component: String,
        base: Option<usize>,
    },
    AttemptResumed {
        attempt: usize,
    },
    PhaseChanged {
        attempt: usize,
        phase: Phase,
    },
    /// The accepted pointer moved to `attempt` from `previous`.
    Accepted {
        attempt: usize,
        previous: Option<usize>,
    },
    Rejected {
        attempt: usize,
        error: String,
    },
    /// The accepted pointer moved back from `attempt` to `restored`.
    RolledBack {
        attempt: usize,
        restored: Option<usize>,
    },
    /// `workloads` whole-application workloads were timed against accepted `attempt`.
    WorkloadsMeasured {
        attempt: usize,
        workloads: usize,
    },
}

impl EventKind {
    fn attempt(&self) -> Option<usize> {
        match self {
            Self::SessionCreated | Self::SchemaUpgraded { .. } => None,
            Self::AttemptStarted { attempt, .. }
            | Self::AttemptResumed { attempt }
            | Self::PhaseChanged { attempt, .. }
            | Self::Accepted { attempt, .. }
            | Self::Rejected { attempt, .. }
            | Self::RolledBack { attempt, .. }
            | Self::WorkloadsMeasured { attempt, .. } => Some(*attempt),
        }
    }
}

impl State {
    /// Append an event. It reaches disk with the next save, together with the state it describes.
    pub(crate) fn record(&mut self, kind: EventKind) {
        let unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
        self.events.push(Event {
            seq: self.events.len() + 1,
            version: EVENT_SCHEMA_VERSION,
            unix_ms,
            kind,
        });
    }
}

/// Read `state.json`, upgrading an older checkpoint as described in the module docs.
pub(crate) fn load(directory: &Path) -> Result<State> {
    let path = directory.join("state.json");
    let bytes = fs::read(&path)?;
    let raw: Value = serde_json::from_slice(&bytes)
        .context("checkpoint is corrupt; original project is untouched")?;
    let version = raw
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok());
    match version {
        Some(CHECKPOINT_SCHEMA_VERSION) => {
            let state: State = serde_json::from_value(raw)
                .context("checkpoint is corrupt; original project is untouched")?;
            validate(&state)?;
            Ok(state)
        }
        Some(1) => {
            let state = upgrade_v1(raw)?;
            validate(&state)?;
            let backup = directory.join("state.v1.json");
            if backup.exists() {
                // Left by an interrupted upgrade of this same checkpoint, or not ours to replace.
                ensure!(
                    fs::read(&backup)? == bytes,
                    "{} exists and differs from the checkpoint being upgraded; move it aside to continue",
                    backup.display()
                );
            } else {
                atomic_write(&backup, &bytes)?;
            }
            atomic_json(&path, &state)?;
            Ok(state)
        }
        Some(v) if v > CHECKPOINT_SCHEMA_VERSION => bail!(
            "checkpoint schema {v} was written by a newer Oxidize (this one reads up to {CHECKPOINT_SCHEMA_VERSION}); use that version or a newer one"
        ),
        _ => bail!("unsupported checkpoint schema; original project is untouched"),
    }
}

/// v0.1.x → 2: add the event history, starting with the upgrade itself.
fn upgrade_v1(mut raw: Value) -> Result<State> {
    let object = raw.as_object_mut().context("checkpoint is not an object")?;
    ensure!(
        !object.contains_key("events"),
        "schema 1 checkpoint unexpectedly has an event history"
    );
    object.insert("schema_version".into(), CHECKPOINT_SCHEMA_VERSION.into());
    object.insert("events".into(), Value::Array(vec![]));
    let mut state: State = serde_json::from_value(raw)
        .context("schema 1 checkpoint is corrupt; original project is untouched")?;
    state.record(EventKind::SchemaUpgraded {
        from: 1,
        to: CHECKPOINT_SCHEMA_VERSION,
        backup: "state.v1.json".into(),
    });
    Ok(state)
}

fn validate(state: &State) -> Result<()> {
    state.config.validate()?;
    ensure!(
        state
            .attempts
            .iter()
            .enumerate()
            .all(|(i, a)| a.id == i + 1 && a.base.is_none_or(|b| b < a.id && b > 0)),
        "invalid checkpoint history"
    );
    ensure!(
        state
            .current
            .is_none_or(|id| id > 0 && id <= state.attempts.len()),
        "invalid current checkpoint"
    );
    let events = &state.events;
    ensure!(
        matches!(
            events.first().map(|e| &e.kind),
            Some(EventKind::SessionCreated | EventKind::SchemaUpgraded { .. })
        ) && events.iter().enumerate().all(|(i, e)| {
            e.seq == i + 1
                && e.version == EVENT_SCHEMA_VERSION
                && (i == 0 || e.kind != EventKind::SessionCreated)
                && e.kind
                    .attempt()
                    .is_none_or(|a| a > 0 && a <= state.attempts.len())
        }),
        "invalid event history"
    );
    Ok(())
}
