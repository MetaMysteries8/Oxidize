use crate::Evidence;
use oxidize_core::{ComparisonPolicy, FloatTolerance};
use serde_json::Value;

/// The existing entry point stays exact, regardless of any configured opt-ins.
pub fn equivalent(a: &Evidence, b: &Evidence) -> bool {
    equivalent_with_policy(a, b, &ComparisonPolicy::default())
}

pub fn equivalent_with_policy(a: &Evidence, b: &Evidence, policy: &ComparisonPolicy) -> bool {
    differential_mismatch(a, b, policy).is_none()
}

pub(crate) fn has_argument_evidence(observation: &Value) -> bool {
    observation
        .get("arguments_after")
        .is_some_and(|a| a.get("args").is_some() && a.get("kwargs").is_some())
}

pub(crate) fn has_file_effect_evidence(observation: &Value, paths: &[String]) -> bool {
    if paths.is_empty() {
        return true;
    }
    let Some(effects) = observation.get("file_effects").and_then(Value::as_object) else {
        return false;
    };
    effects.len() == paths.len()
        && paths.iter().all(|path| {
            effects
                .get(path)
                .and_then(Value::as_object)
                .is_some_and(|states| {
                    states.len() == 2
                        && ["before", "after"]
                            .iter()
                            .all(|key| states.get(*key).is_some_and(valid_file_state))
                })
        })
}

fn valid_file_state(state: &Value) -> bool {
    if state.is_null() {
        return true;
    }
    let Some(file) = state.as_object() else {
        return false;
    };
    file.len() == 2
        && file
            .get("size_bytes")
            .and_then(Value::as_u64)
            .is_some_and(|size| size <= 1_048_576)
        && file
            .get("sha256")
            .and_then(Value::as_str)
            .is_some_and(|hash| hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit()))
}

pub(crate) fn differential_mismatch(
    a: &Evidence,
    b: &Evidence,
    policy: &ComparisonPolicy,
) -> Option<String> {
    if let Err(error) = policy.validate() {
        return Some(format!("invalid comparison policy: {error}"));
    }
    if a.observation_schema_version != 2
        || b.observation_schema_version != 2
        || !a
            .observations
            .iter()
            .chain(&b.observations)
            .all(has_argument_evidence)
    {
        return Some(
            "argument mutation evidence is unavailable; replay legacy evidence before comparison"
                .into(),
        );
    }
    if a.observations.len() != b.observations.len() {
        return Some("behavior case counts differ".into());
    }
    if a.file_effect_paths != b.file_effect_paths
        || a.observations
            .iter()
            .chain(&b.observations)
            .any(|o| !has_file_effect_evidence(o, &a.file_effect_paths))
    {
        return Some("configured file-effect evidence is unavailable or scopes differ".into());
    }
    if a.import_stdout != b.import_stdout || a.import_stderr != b.import_stderr {
        return Some("module import stdout/stderr differ".into());
    }
    for (index, (original, candidate)) in a.observations.iter().zip(&b.observations).enumerate() {
        if original.get("arguments_after") != candidate.get("arguments_after") {
            return Some(format!("case {}: argument mutation differs", index + 1));
        }
        if original.get("file_effects") != candidate.get("file_effects") {
            return Some(format!(
                "case {}: configured file effects differ",
                index + 1
            ));
        }
        let same =
            original
                .as_object()
                .zip(candidate.as_object())
                .is_some_and(|(original, candidate)| {
                    original.len() == candidate.len()
                        && original.iter().all(|(key, value)| {
                            candidate.get(key).is_some_and(|other| match key.as_str() {
                                "return" => {
                                    returns_match(value, other, &policy.return_float_tolerance)
                                }
                                "stdout" | "stderr" if policy.normalize_output_line_endings => {
                                    output_matches(value, other)
                                }
                                _ => value == other,
                            })
                        })
                });
        if !same {
            return Some(format!(
                "case {}: return value/type, error, or stdout/stderr differ",
                index + 1
            ));
        }
    }
    None
}

fn output_matches(a: &Value, b: &Value) -> bool {
    let normalize = |text: &str| text.replace("\r\n", "\n").replace('\r', "\n");
    a.as_str()
        .zip(b.as_str())
        .is_some_and(|(a, b)| normalize(a) == normalize(b))
}

fn returns_match(a: &Value, b: &Value, tolerance: &Option<FloatTolerance>) -> bool {
    if a == b {
        return true;
    }
    let Some((a, b)) = a.as_array().zip(b.as_array()) else {
        return false;
    };
    if a.len() != 2 || b.len() != 2 || a[0] != b[0] {
        return false;
    }
    match a[0].as_str() {
        Some("float") => tolerance.as_ref().is_some_and(|tolerance| {
            a[1].as_f64()
                .zip(b[1].as_f64())
                .is_some_and(|(a, b)| floats_match(a, b, tolerance))
        }),
        Some("list" | "tuple") => a[1].as_array().zip(b[1].as_array()).is_some_and(|(a, b)| {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| returns_match(a, b, tolerance))
        }),
        Some("dict") => a[1].as_array().zip(b[1].as_array()).is_some_and(|(a, b)| {
            a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| {
                    a.as_array().zip(b.as_array()).is_some_and(|(a, b)| {
                        a.len() == 2
                            && b.len() == 2
                            && a[0] == b[0]
                            && returns_match(&a[1], &b[1], tolerance)
                    })
                })
        }),
        _ => false,
    }
}

fn floats_match(a: f64, b: f64, tolerance: &FloatTolerance) -> bool {
    if !a.is_finite() || !b.is_finite() {
        return false;
    }
    let difference = (a - b).abs();
    if difference <= tolerance.absolute {
        return true;
    }
    let scale = a.abs().max(b.abs());
    // Scaling before subtraction avoids overflow for opposite-sign finite extremes.
    scale > 0.0 && (a / scale - b / scale).abs() <= tolerance.relative
}
