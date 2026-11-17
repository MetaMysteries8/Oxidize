//! Milestone 2 §1: user-defined whole-application workloads, timed on the original and the
//! accepted workspace in interleaved rounds, with CPU time and peak RSS where the platform
//! reports them. Unavailable measurements are listed as unsupported, never left blank.
use crate::sampling::{self, Round, Side, Verdict};
use serde::{Deserialize, Serialize};

/// The Python wrapper that times one workload command; `oxidize` runs it inside the sandbox.
#[doc(hidden)]
pub const WRAPPER: &str = include_str!("workload.py");

/// What the wrapper reports for one side of one round.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkloadSide {
    pub wall_ns: Vec<f64>,
    /// User + system CPU of each run's process tree. `None` where unsupported.
    pub cpu_ns: Option<Vec<f64>>,
    /// Largest single process across the warm-up and timed runs, in KiB. `None` where unsupported.
    pub peak_rss_kib: Option<u64>,
    pub unsupported: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkloadRound {
    pub order: [Side; 2],
    pub original: WorkloadSide,
    pub candidate: WorkloadSide,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkloadResult {
    pub name: String,
    pub command: Vec<String>,
    pub rounds: Vec<WorkloadRound>,
    /// Original wall time / accepted wall time.
    pub wall: Verdict,
    /// Original CPU / accepted CPU, or why it can't be claimed.
    pub cpu: Verdict,
    /// Peak RSS across all rounds, per side, in KiB, where reported.
    pub original_peak_rss_kib: Option<u64>,
    pub candidate_peak_rss_kib: Option<u64>,
    /// Measurements this platform couldn't make, and other caveats, deduplicated.
    pub unsupported: Vec<String>,
}

/// One `oxidize workloads` run, kept in the checkpoint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkloadRun {
    /// The accepted attempt the original was compared against.
    pub against: usize,
    pub isolation: String,
    pub os: String,
    pub arch: String,
    pub results: Vec<WorkloadResult>,
}

/// Decide wall and CPU verdicts for one workload's rounds.
pub fn summarize(
    name: &str,
    command: &[String],
    rounds: Vec<WorkloadRound>,
    minimum_samples: usize,
    max_relative_mad: f64,
) -> WorkloadResult {
    let wall: Vec<Round> = rounds
        .iter()
        .map(|r| Round {
            order: r.order,
            original_ns: r.original.wall_ns.clone(),
            candidate_ns: r.candidate.wall_ns.clone(),
        })
        .collect();
    let cpu: Option<Vec<Round>> = rounds
        .iter()
        .map(|r| {
            Some(Round {
                order: r.order,
                original_ns: r.original.cpu_ns.clone()?,
                candidate_ns: r.candidate.cpu_ns.clone()?,
            })
        })
        .collect();
    let cpu = match cpu {
        Some(cpu) => sampling::decide(&cpu, minimum_samples, max_relative_mad),
        None => Verdict::Inconclusive {
            reason: "CPU time is unsupported on this platform".into(),
        },
    };
    let peak = |side: fn(&WorkloadRound) -> &WorkloadSide| {
        rounds
            .iter()
            .map(|r| side(r).peak_rss_kib)
            .try_fold(0, |max, p| p.map(|p| p.max(max)))
    };
    let mut unsupported: Vec<String> = rounds
        .iter()
        .flat_map(|r| {
            r.original
                .unsupported
                .iter()
                .chain(&r.candidate.unsupported)
        })
        .cloned()
        .collect();
    unsupported.sort();
    unsupported.dedup();
    WorkloadResult {
        name: name.into(),
        command: command.to_vec(),
        wall: sampling::decide(&wall, minimum_samples, max_relative_mad),
        cpu,
        original_peak_rss_kib: peak(|r| &r.original),
        candidate_peak_rss_kib: peak(|r| &r.candidate),
        unsupported,
        rounds,
    }
}

fn verdict_text(v: &Verdict) -> String {
    match v {
        Verdict::Measured {
            estimate,
            low,
            high,
        } => format!("**{estimate:.3}x** (range {low:.3}x to {high:.3}x)"),
        Verdict::Inconclusive { reason } => format!("no claim ({reason})"),
    }
}

/// The report's application-workload section.
pub fn report_section(runs: &[WorkloadRun]) -> String {
    let Some(run) = runs.last() else {
        return "## Application workloads\n\nNone measured. Configure `[[performance.workloads]]` and run `oxidize workloads`.\n\n".into();
    };
    let mut text = format!(
        "## Application workloads\n\nLatest run: original vs accepted attempt {} on {}/{}, isolation `{}`, interleaved rounds as above. Ratios are original / accepted, so above 1x means the accepted workspace was faster. Peak RSS is the largest single process, not a total.\n\n",
        run.against, run.os, run.arch, run.isolation
    );
    for r in &run.results {
        let kib = |k: Option<u64>| k.map_or_else(|| "unsupported".into(), |k| format!("{k} KiB"));
        text += &format!(
            "- `{}` (`{}`): wall time {}; CPU time {}; peak RSS original {}, accepted {}.{}\n",
            r.name,
            r.command.join(" "),
            verdict_text(&r.wall),
            verdict_text(&r.cpu),
            kib(r.original_peak_rss_kib),
            kib(r.candidate_peak_rss_kib),
            if r.unsupported.is_empty() {
                String::new()
            } else {
                format!(" Not measured / caveats: {}.", r.unsupported.join("; "))
            }
        );
    }
    text + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn side(wall: f64, cpu: Option<f64>, rss: Option<u64>) -> WorkloadSide {
        WorkloadSide {
            wall_ns: vec![wall; 3],
            cpu_ns: cpu.map(|c| vec![c; 3]),
            peak_rss_kib: rss,
            unsupported: if cpu.is_none() {
                vec!["CPU time and peak RSS: unavailable".into()]
            } else {
                vec![]
            },
        }
    }

    #[test]
    fn wall_cpu_and_peak_memory_are_summarized_per_side() {
        let rounds = (0..2)
            .map(|i| WorkloadRound {
                order: sampling::order(i),
                original: side(200.0, Some(100.0), Some(9000 + i as u64)),
                candidate: side(100.0, Some(50.0), Some(4000)),
            })
            .collect();
        let r = summarize("cli", &["x".into()], rounds, 3, 0.25);
        assert!(r.wall.clears(2.0) && r.cpu.clears(2.0));
        assert_eq!(r.original_peak_rss_kib, Some(9001));
        assert_eq!(r.candidate_peak_rss_kib, Some(4000));
        assert!(r.unsupported.is_empty());
        assert!(
            report_section(&[WorkloadRun {
                against: 1,
                isolation: "bubblewrap".into(),
                os: "linux".into(),
                arch: "x86_64".into(),
                results: vec![r],
            }])
            .contains("wall time **2.000x**")
        );
    }

    #[test]
    fn unsupported_measurements_are_named_not_blank() {
        let rounds = vec![WorkloadRound {
            order: sampling::order(0),
            original: side(200.0, None, None),
            candidate: side(100.0, None, None),
        }];
        let r = summarize("cli", &["x".into()], rounds, 3, 0.25);
        assert!(r.wall.clears(1.0), "wall time is still measured");
        assert!(!r.cpu.clears(0.000001));
        assert_eq!(r.original_peak_rss_kib, None);
        let section = report_section(&[WorkloadRun {
            against: 1,
            isolation: "trust-code".into(),
            os: "windows".into(),
            arch: "x86_64".into(),
            results: vec![r],
        }]);
        assert!(
            section.contains("CPU time no claim (CPU time is unsupported"),
            "{section}"
        );
        assert!(
            section.contains("peak RSS original unsupported"),
            "{section}"
        );
        assert!(section.contains("Not measured / caveats: CPU time and peak RSS: unavailable"));
        assert!(report_section(&[]).contains("None measured"));
    }
}
