//! Wire measurements for the two populations of committed GPU handovers.
use serde::{Deserialize, Serialize};

/// Whether the published ranks cover the complete eligible population.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResidualQuantiles {
    /// Measurement coverage or sample size was insufficient.
    Unavailable,
    /// All eligible values were retained.
    Exact,
    /// Ranks were computed from a smaller uniform sample.
    Estimated,
}

/// One population's total and its uniform reservoir.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidualSamplePopulation {
    /// Total members, when known.
    #[serde(deserialize_with = "super::handovers::nullable")]
    pub seen: Option<u64>,
    /// Maximum number of retained members.
    pub capacity: usize,
    /// Occupied positions, including missing measurements.
    pub retained: usize,
}

/// Coverage, exact reductions and ranks for one measurement.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidualSampleMetric {
    /// Members to which this measurement applies, when known.
    #[serde(deserialize_with = "super::handovers::nullable")]
    pub eligible: Option<u64>,
    /// Eligible members with a known value, absent when the count overflowed.
    #[serde(deserialize_with = "super::handovers::nullable")]
    pub known: Option<u64>,
    /// Usable values from the retained sample.
    pub retained: usize,
    /// Exact sum when representable with complete coverage.
    #[serde(deserialize_with = "super::handovers::nullable")]
    pub sum: Option<u128>,
    /// Population maximum when known.
    #[serde(deserialize_with = "super::handovers::nullable")]
    pub maximum: Option<u128>,
    /// Midpoint median rounded down in the metric's integer units.
    #[serde(deserialize_with = "super::handovers::nullable")]
    pub median: Option<u128>,
    /// Nearest-rank ninetieth percentile.
    #[serde(deserialize_with = "super::handovers::nullable")]
    pub p90: Option<u128>,
    /// Provenance of the published ranks.
    pub quantiles: ResidualQuantiles,
}

/// World, query and batch measurements with their respective denominators.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidualSampleGroup {
    /// Open atoms in each handed-over world.
    pub open_atoms: ResidualSampleMetric,
    /// Regions visited by each entered query.
    pub visited: ResidualSampleMetric,
    /// Splits attempted by the query.
    pub attempted_splits: ResidualSampleMetric,
    /// Splits published by the query.
    pub published_splits: ResidualSampleMetric,
    /// Deepest visited level of a query with visits.
    pub depth: ResidualSampleMetric,
    /// Query work units.
    pub work: ResidualSampleMetric,
    /// Each query's host interval in nanoseconds.
    pub query_ns: ResidualSampleMetric,
    /// Residual worlds per committed batch, including zero.
    pub residuals: ResidualSampleMetric,
    /// Maximum joint depth-level visits per batch.
    pub width: ResidualSampleMetric,
    /// Sum of the batch's query intervals in nanoseconds.
    pub batch_ns: ResidualSampleMetric,
}

/// Aggregate and per-reason views of the same committed observations.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidualSampleGroups {
    /// All stopping reasons.
    pub all: ResidualSampleGroup,
    /// Device reached a fixed point.
    pub fixed_point: ResidualSampleGroup,
    /// Device reached the round limit.
    pub round_limit: ResidualSampleGroup,
    /// Device reached its work limit.
    pub work_limit: ResidualSampleGroup,
}

/// Compact versioned run report. Raw sampled records stay with their run owner.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidualRunSamples {
    /// Wire format version.
    pub version: u32,
    /// Random-stream and sampling protocol.
    pub algorithm: String,
    /// Initial run seed.
    pub seed: u64,
    /// Independent observation byte allowance.
    pub byte_limit: u64,
    /// Currently retained accounted bytes.
    pub retained_bytes: u64,
    /// Largest observed storage demand.
    pub peak_bytes: u64,
    /// Population of residual worlds.
    pub worlds: ResidualSamplePopulation,
    /// Population of committed batches.
    pub batches: ResidualSamplePopulation,
    /// Settled groups when finalization and its storage were available.
    #[serde(deserialize_with = "super::handovers::nullable")]
    pub groups: Option<Box<ResidualSampleGroups>>,
}

impl ResidualRunSamples {
    pub(crate) fn check(&self, committed: Option<u64>) -> Result<(), String> {
        if self.version != 1 || self.algorithm != "splitmix64-reject-v1" {
            return Err("unsupported GPU residual sampling protocol".into());
        }
        if self.retained_bytes > self.byte_limit || self.peak_bytes < self.retained_bytes {
            return Err("inconsistent GPU residual sampling bytes".into());
        }
        for pool in [&self.worlds, &self.batches] {
            if pool.retained > pool.capacity
                || pool
                    .seen
                    .is_some_and(|seen| pool.retained as u128 > u128::from(seen))
            {
                return Err("inconsistent GPU residual sample population".into());
            }
        }
        if committed
            .zip(self.batches.seen)
            .is_some_and(|(a, b)| a != b)
        {
            return Err("sampled GPU batch population disagrees with commits".into());
        }
        if let Some(groups) = &self.groups {
            if groups
                .all
                .residuals
                .sum
                .zip(self.worlds.seen)
                .is_some_and(|(sum, seen)| sum != u128::from(seen))
            {
                return Err("residual batch counts disagree with the world population".into());
            }
            for group in [
                &groups.all,
                &groups.fixed_point,
                &groups.round_limit,
                &groups.work_limit,
            ] {
                for metric in [
                    &group.open_atoms,
                    &group.visited,
                    &group.attempted_splits,
                    &group.published_splits,
                    &group.depth,
                    &group.work,
                    &group.query_ns,
                ] {
                    metric.check(&self.worlds)?;
                }
                for metric in [&group.residuals, &group.width, &group.batch_ns] {
                    metric.check(&self.batches)?;
                    if metric
                        .eligible
                        .zip(self.batches.seen)
                        .is_some_and(|(a, b)| a != b)
                    {
                        return Err("residual batch metric omitted committed batches".into());
                    }
                }
            }
        }
        Ok(())
    }
}

impl ResidualSampleMetric {
    fn check(&self, pool: &ResidualSamplePopulation) -> Result<(), String> {
        let sample = self.retained as u128;
        if self.retained > pool.retained
            || self.known.is_some_and(|n| sample > u128::from(n))
            || self
                .known
                .zip(self.eligible)
                .is_some_and(|(known, n)| known > n)
            || self
                .known
                .zip(pool.seen)
                .is_some_and(|(known, n)| known > n)
            || self
                .eligible
                .zip(pool.seen)
                .is_some_and(|(eligible, seen)| eligible > seen)
        {
            return Err("inconsistent residual metric coverage".into());
        }
        let complete = self.known.is_some() && self.eligible == self.known;
        let known = self.known.map(u128::from);
        let ranks = self.median.zip(self.p90);
        let valid = match self.quantiles {
            ResidualQuantiles::Unavailable => self.median.is_none() && self.p90.is_none(),
            ResidualQuantiles::Exact => {
                complete && sample > 0 && Some(sample) == known && ranks.is_some()
            }
            ResidualQuantiles::Estimated => {
                complete && sample > 0 && known.is_some_and(|n| sample < n) && ranks.is_some()
            }
        };
        if !valid
            || !complete && self.sum.is_some()
            || ranks.is_some_and(|(m, p)| m > p)
            || self.known == Some(0) && self.maximum.is_some()
            || self
                .maximum
                .zip(self.sum)
                .is_some_and(|(max, sum)| max > sum)
            || self.maximum.zip(self.p90).is_some_and(|(max, p)| p > max)
        {
            return Err("inconsistent residual metric result".into());
        }
        Ok(())
    }
}
