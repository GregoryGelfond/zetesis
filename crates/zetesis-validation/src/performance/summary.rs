//! Exact inclusive quartiles over separately qualified observation populations.
use serde::Serialize;

use super::{Phase, Producer, Report};

/// Exact value in quarter units: `whole + quarters / 4`.
/// This avoids narrowing raw nanoseconds or bytes through floating point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Quartile {
    /// Integer component in the distribution's stated unit.
    pub whole: u128,
    /// Fractional quarter units, always zero through three.
    pub quarters: u8,
}

/// Inclusive interpolation at ranks `(n - 1) × {0, 1/4, 1/2, 3/4, 1}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Distribution {
    /// Number of observations in this population alone.
    pub samples: usize,
    /// Smallest observation, in raw units.
    pub minimum: u128,
    /// First inclusive quartile.
    pub q1: Quartile,
    /// Exact median.
    pub median: Quartile,
    /// Third inclusive quartile.
    pub q3: Quartile,
    /// Largest observation, in raw units.
    pub maximum: u128,
}

/// One solver's independent wall-time and child-memory populations for one case.
#[derive(Clone, Debug, Serialize)]
pub struct Summary {
    /// Manifest-relative source path.
    pub case: String,
    /// Native or independent reference producer.
    pub producer: Producer,
    /// Direct timed invocations only; no helper, qualification or diagnostics.
    pub wall_nanoseconds: Distribution,
    /// Fresh-helper observations only, absent when none were requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_peak_rss_bytes: Option<Distribution>,
}

pub(super) fn collect(report: &Report) -> Vec<Summary> {
    report
        .schedule
        .cases()
        .iter()
        .flat_map(|case| {
            [Producer::Native, Producer::Reference].map(|producer| {
                let samples = || {
                    report.samples.iter().filter(|sample| {
                        sample.slot.case == *case && sample.slot.producer == producer
                    })
                };
                let wall = samples()
                    .filter(|sample| sample.slot.phase == Phase::Timed)
                    .map(|sample| {
                        sample
                            .capture
                            .elapsed_ns
                            .expect("qualified timed capture has elapsed observation")
                    })
                    .collect();
                let memory = samples()
                    .filter_map(|sample| {
                        (sample.slot.phase == Phase::Memory)
                            .then_some(sample.memory)
                            .flatten()
                            .map(|memory| u128::from(memory.peak_rss_bytes))
                    })
                    .collect();
                Summary {
                    case: case.path().into(),
                    producer,
                    wall_nanoseconds: distribution(wall)
                        .expect("complete schedule has timed observations"),
                    child_peak_rss_bytes: distribution(memory),
                }
            })
        })
        .collect()
}

fn distribution(mut values: Vec<u128>) -> Option<Distribution> {
    values.sort_unstable();
    Some(Distribution {
        samples: values.len(),
        minimum: *values.first()?,
        maximum: *values.last()?,
        q1: interpolate(&values, 1),
        median: interpolate(&values, 2),
        q3: interpolate(&values, 3),
    })
}

fn interpolate(values: &[u128], quarter: usize) -> Quartile {
    // Populations have at most 41 values; quarter is in 1..=3. The interpolated
    // whole lies between adjacent values, even at u128::MAX. Divide before
    // multiplying so no intermediate sum/product exceeds the upper endpoint.
    let rank = (values.len() - 1) * quarter;
    let index = rank / 4;
    let remainder = rank % 4;
    let lower = values[index];
    let distance = values.get(index + 1).unwrap_or(&lower) - lower;
    let residue = (distance % 4) * remainder as u128;
    Quartile {
        whole: lower + (distance / 4) * remainder as u128 + residue / 4,
        quarters: u8::try_from(residue % 4).expect("quarter remainder is below four"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inclusive_quartiles_preserve_fractional_units() {
        let result = distribution(vec![12, 1, 6, 3]).unwrap();
        assert_eq!(
            result.q1,
            Quartile {
                whole: 2,
                quarters: 2
            }
        );
        assert_eq!(
            result.median,
            Quartile {
                whole: 4,
                quarters: 2
            }
        );
        assert_eq!(
            result.q3,
            Quartile {
                whole: 7,
                quarters: 2
            }
        );
    }

    #[test]
    fn interpolation_does_not_overflow_its_endpoints() {
        let result = distribution(vec![0, u128::MAX]).unwrap();
        assert_eq!(
            result.median,
            Quartile {
                whole: u128::MAX / 2,
                quarters: 2
            }
        );
    }

    #[test]
    fn an_absent_population_has_no_distribution() {
        assert_eq!(distribution(Vec::new()), None);
    }
}
