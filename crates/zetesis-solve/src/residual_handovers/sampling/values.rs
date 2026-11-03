//! Whole-population reductions and terminal sample ranks.

use super::{ResidualQuantiles, ResidualSampleMetric};

/// Whether the member belongs to this metric, independently of whether its
/// numerical value was successfully observed.
#[derive(Clone, Copy, Debug)]
pub(super) enum Value {
    Outside,
    UnknownEligibility,
    Missing,
    Known(u128),
}

impl Value {
    pub(super) fn optional<T: Into<u128>>(value: Option<T>) -> Self {
        value.map_or(Self::Missing, |v| Self::Known(v.into()))
    }

    pub(super) fn measured(self) -> Option<u128> {
        if let Self::Known(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Totals {
    eligible: Option<u64>,
    known: Option<u64>,
    sum: Option<u128>,
    maximum: Option<u128>,
}

impl Default for Totals {
    fn default() -> Self {
        Self {
            eligible: Some(0),
            known: Some(0),
            sum: Some(0),
            maximum: None,
        }
    }
}

impl Totals {
    pub(super) fn observe(&mut self, value: Value) {
        match value {
            Value::Outside => return,
            Value::UnknownEligibility => {
                self.eligible = None;
                return;
            }
            Value::Missing => {}
            Value::Known(v) => {
                self.known = self.known.and_then(|n| n.checked_add(1));
                self.sum = self.sum.and_then(|sum| sum.checked_add(v));
                self.maximum = self.maximum.max(Some(v));
            }
        }
        self.eligible = self.eligible.and_then(|n| n.checked_add(1));
    }

    /// Selection mutates the terminal sample's order only. Each comparison
    /// projects a single field of an existing record. No scratch allocation
    /// is needed; the retained set stays identical for subsequent metrics.
    pub(super) fn finish<T>(
        &self,
        records: &mut [T],
        value: impl Fn(&T) -> Option<u128>,
    ) -> ResidualSampleMetric {
        let retained = if self.known == Some(0) {
            0
        } else {
            records.iter().filter(|r| value(r).is_some()).count()
        };
        let complete = self.known.is_some() && self.eligible == self.known;
        let mut metric = ResidualSampleMetric {
            eligible: self.eligible,
            known: self.known,
            retained,
            sum: complete.then_some(self.sum).flatten(),
            maximum: complete.then_some(self.maximum).flatten(),
            ..ResidualSampleMetric::default()
        };
        if complete && retained != 0 {
            let missing = records.len() - retained;
            let mut rank = |index: usize| {
                let (_, selected, _) = records.select_nth_unstable_by_key(missing + index, &value);
                value(selected)
            };
            let middle = rank((retained - 1) / 2);
            metric.median = if retained.is_multiple_of(2) {
                middle
                    .zip(rank(retained / 2))
                    .map(|(low, high)| low.midpoint(high))
            } else {
                middle
            };
            // ceil(9 n / 10) - 1, without multiplying a sample size.
            metric.p90 = rank(retained - retained / 10 - 1);
            metric.quantiles = if self.known == Some(retained as u64) {
                ResidualQuantiles::Exact
            } else {
                ResidualQuantiles::Estimated
            };
        }
        metric
    }
}

impl Totals {
    pub(super) fn observed_maximum(&self) -> Option<u128> {
        self.maximum
    }

    pub(super) fn missing_members(&mut self, count: Option<u64>) {
        self.eligible = self.eligible.and_then(|n| n.checked_add(count?));
    }
}

#[cfg(test)]
mod tests;
