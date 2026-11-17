//! Milestone 2 §1: interleaved original/candidate measurement and the decision it supports.
//!
//! The engine times the original and the candidate in alternating order, round after round
//! (original first, then candidate first, and so on), each time in a fresh process with the
//! same warm-up and fresh inputs. Slow drift on the machine (thermal throttling, a background
//! job) then affects both sides alike instead of favouring whichever ran second.
//!
//! [`decide`] turns the retained raw samples into a verdict. It never makes a speed claim from
//! evidence that is missing, malformed, too small or too noisy: those are [`Verdict::Inconclusive`].
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Original,
    Candidate,
}

/// Which side runs first in round `round` (counting from 0): alternating, so neither side
/// always benefits from going second.
pub fn order(round: usize) -> [Side; 2] {
    if round.is_multiple_of(2) {
        [Side::Original, Side::Candidate]
    } else {
        [Side::Candidate, Side::Original]
    }
}

/// One round's raw timings, in nanoseconds per sample, for each side.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Round {
    pub order: [Side; 2],
    pub original_ns: Vec<f64>,
    pub candidate_ns: Vec<f64>,
}

/// What the measurement supports, given the configured policy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    /// Every round's speedup (original median / candidate median) lies in `low..=high`;
    /// `estimate` is the median of those per-round speedups.
    Measured { estimate: f64, low: f64, high: f64 },
    /// No speed claim can be made, for the stated reason.
    Inconclusive { reason: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    pub rounds: Vec<Round>,
    /// The policy in force when it was decided.
    pub max_relative_mad: f64,
    pub minimum_samples: usize,
    pub verdict: Verdict,
}

/// The median, or NaN for no values.
pub fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

/// Median absolute deviation relative to the median: 0 is perfectly steady.
pub fn relative_mad(values: &[f64]) -> f64 {
    let m = median(values);
    let deviations: Vec<f64> = values.iter().map(|v| (v - m).abs()).collect();
    median(&deviations) / m
}

/// Decide what `rounds` show. Each side needs at least `minimum_samples` valid samples per
/// round, and its relative MAD in each round must not exceed `max_relative_mad`.
pub fn decide(rounds: &[Round], minimum_samples: usize, max_relative_mad: f64) -> Verdict {
    let inconclusive = |reason: String| Verdict::Inconclusive { reason };
    if rounds.is_empty() {
        return inconclusive("no measurement rounds ran".into());
    }
    let mut speedups = vec![];
    for (i, round) in rounds.iter().enumerate() {
        let n = i + 1;
        if round.order != order(i) {
            return inconclusive(format!("round {n} did not follow the interleaved order"));
        }
        for (side, samples) in [
            ("original", &round.original_ns),
            ("candidate", &round.candidate_ns),
        ] {
            if samples.len() < minimum_samples.max(1) {
                return inconclusive(format!(
                    "round {n}: {} {side} sample(s), at least {} required",
                    samples.len(),
                    minimum_samples.max(1)
                ));
            }
            if !samples.iter().all(|s| s.is_finite() && *s > 0.0) {
                return inconclusive(format!(
                    "round {n}: {side} has a missing, zero or non-finite timing"
                ));
            }
            let noise = relative_mad(samples);
            if noise > max_relative_mad {
                return inconclusive(format!(
                    "round {n}: {side} timings too noisy (relative MAD {noise:.3} exceeds {max_relative_mad})"
                ));
            }
        }
        speedups.push(median(&round.original_ns) / median(&round.candidate_ns));
    }
    let low = speedups.iter().copied().fold(f64::INFINITY, f64::min);
    let high = speedups.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Verdict::Measured {
        estimate: median(&speedups),
        low,
        high,
    }
}

impl Verdict {
    /// Whether this supports accepting a candidate that must be at least `minimum` times as
    /// fast. The slowest round has to clear it; an inconclusive measurement never does.
    pub fn clears(&self, minimum: f64) -> bool {
        matches!(self, Verdict::Measured { low, .. } if *low >= minimum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(i: usize, original: &[f64], candidate: &[f64]) -> Round {
        Round {
            order: order(i),
            original_ns: original.to_vec(),
            candidate_ns: candidate.to_vec(),
        }
    }

    #[test]
    fn rounds_alternate_which_side_goes_first() {
        assert_eq!(order(0), [Side::Original, Side::Candidate]);
        assert_eq!(order(1), [Side::Candidate, Side::Original]);
        assert_eq!(order(2), order(0));
    }

    #[test]
    fn steady_rounds_give_an_estimate_and_its_range() {
        let rounds = [
            round(0, &[200.0, 202.0, 198.0], &[100.0, 101.0, 99.0]),
            round(1, &[300.0, 303.0, 297.0], &[100.0, 100.0, 100.0]),
            round(2, &[250.0, 250.0, 250.0], &[100.0, 100.0, 100.0]),
        ];
        let verdict = decide(&rounds, 3, 0.1);
        assert_eq!(
            verdict,
            Verdict::Measured {
                estimate: 2.5,
                low: 2.0,
                high: 3.0
            }
        );
        assert!(verdict.clears(2.0));
        assert!(!verdict.clears(2.01), "the slowest round decides");
    }

    #[test]
    fn missing_or_insufficient_data_is_never_a_claim() {
        for (rounds, expected) in [
            (vec![], "no measurement rounds"),
            (
                vec![round(0, &[], &[1.0, 1.0, 1.0])],
                "0 original sample(s)",
            ),
            (vec![round(0, &[1.0, 1.0], &[1.0, 1.0, 1.0])], "at least 3"),
            (
                vec![round(0, &[1.0, 1.0, 1.0], &[1.0, 0.0, 1.0])],
                "zero or non-finite",
            ),
            (
                vec![round(0, &[1.0, f64::NAN, 1.0], &[1.0, 1.0, 1.0])],
                "zero or non-finite",
            ),
            (
                vec![round(0, &[1.0, f64::INFINITY, 1.0], &[1.0, 1.0, 1.0])],
                "zero or non-finite",
            ),
        ] {
            let verdict = decide(&rounds, 3, 0.5);
            assert!(
                matches!(&verdict, Verdict::Inconclusive { reason } if reason.contains(expected)),
                "{expected}: {verdict:?}"
            );
            assert!(!verdict.clears(0.000001), "{expected}");
        }
    }

    #[test]
    fn noisy_rounds_are_inconclusive_however_fast_they_look() {
        let rounds = [round(0, &[1000.0, 100.0, 3000.0], &[1.0, 1.0, 1.0])];
        let verdict = decide(&rounds, 3, 0.25);
        assert!(
            matches!(&verdict, Verdict::Inconclusive { reason } if reason.contains("too noisy")),
            "{verdict:?}"
        );
        assert!(!verdict.clears(1.0));
        // The same data passes a looser, explicitly configured policy.
        assert!(decide(&rounds, 3, 10.0).clears(1.0));
    }

    #[test]
    fn rounds_out_of_order_are_refused() {
        let mut swapped = round(0, &[1.0; 3], &[1.0; 3]);
        swapped.order = order(1);
        assert!(matches!(
            decide(&[swapped], 3, 0.5),
            Verdict::Inconclusive { .. }
        ));
    }
}
