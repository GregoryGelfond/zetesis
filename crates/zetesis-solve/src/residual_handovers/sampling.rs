//! Bounded host observations of committed device batches and their residual worlds.

#[cfg(feature = "gpu")]
mod collector;
#[cfg(feature = "gpu")]
mod reservoir;
#[cfg(feature = "gpu")]
mod values;

#[cfg(feature = "gpu")]
pub(crate) use collector::RunSamples;

/// Whether both reported ranks represent all eligible members or a uniform sample.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResidualQuantiles {
    /// No usable sample, incomplete coverage, or an empty population.
    #[default]
    Unavailable,
    /// Every eligible value is present.
    Exact,
    /// A uniform sample of a completely observed population.
    Estimated,
}

/// Counts, complete reductions and terminal ranks for one observation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResidualSampleMetric {
    /// Eligible members, absent when their eligibility was not observable.
    pub eligible: Option<u64>,
    /// Members whose value was observed before sampling, absent on count overflow.
    pub known: Option<u64>,
    /// Eligible observed members still present in the sample.
    pub retained: usize,
    /// Exact sum when coverage is complete and addition was representable.
    pub sum: Option<u128>,
    /// Exact maximum of a nonempty, completely observed population, or one proved by its bounds.
    pub maximum: Option<u128>,
    /// Integer midpoint median, rounded down for an even sample.
    pub median: Option<u128>,
    /// Ninetieth percentile by nearest rank.
    pub p90: Option<u128>,
    /// Coverage of the two reported ranks.
    pub quantiles: ResidualQuantiles,
}

/// Committed observations for all reasons together, or for one reason.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResidualSampleGroup {
    /// Open atom count per residual world, including certified worlds.
    pub open_atoms: ResidualSampleMetric,
    /// Visited regions per entered CPU query.
    pub visited: ResidualSampleMetric,
    /// Attempted splits per entered query.
    pub attempted_splits: ResidualSampleMetric,
    /// Successfully published splits per entered query.
    pub published_splits: ResidualSampleMetric,
    /// Deepest visited level per query with a visit, including the root at zero.
    pub depth: ResidualSampleMetric,
    /// Performed work per entered query.
    pub work: ResidualSampleMetric,
    /// Per-query host intervals in nanoseconds.
    pub query_ns: ResidualSampleMetric,
    /// Residual worlds per committed batch, including zero.
    pub residuals: ResidualSampleMetric,
    /// Widest level across the batch's queries in this group.
    pub width: ResidualSampleMetric,
    /// Sum of query intervals per committed batch in nanoseconds. Worker intervals may overlap.
    pub batch_ns: ResidualSampleMetric,
}

/// One sampling frame, independently funded from the run observation allowance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResidualSamplePopulation {
    /// Number of committed members, absent after a missing population or overflow.
    pub seen: Option<u64>,
    /// Effective record limit. An allocation refusal disables retention for this frame.
    pub capacity: usize,
    /// Number of stored slots, including retained missing observations.
    pub retained: usize,
}

/// Bounded GPU run statistics. Sampling never contributes a membership verdict.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResidualRunSamples {
    /// Replay seed for the independently tagged `SplitMix64` streams.
    pub seed: u64,
    /// Maximum accepted bytes for the run owner and its two samples.
    pub byte_limit: u64,
    /// Admitted heap payload while observing the reported populations.
    pub retained_bytes: u64,
    /// Largest observed payload, including a rejected allocator excess and terminal summaries.
    pub peak_bytes: u64,
    /// Committed residual worlds, whether or not CPU completion entered a query.
    pub worlds: ResidualSamplePopulation,
    /// Committed batches, including batches without residuals.
    pub batches: ResidualSamplePopulation,
    /// Terminal summaries in all, fixed-point, round-limit, work-limit order.
    /// Absent during the run or when terminal observation storage was unavailable.
    pub groups: Option<Vec<ResidualSampleGroup>>,
}

#[cfg(feature = "gpu")]
impl ResidualRunSamples {
    fn try_snapshot(&self) -> Self {
        self.snapshot_with_limit(self.byte_limit.saturating_sub(self.retained_bytes))
    }

    fn snapshot_with_limit(&self, available: u64) -> Self {
        let mut peak = u128::from(self.peak_bytes);
        let groups = self.groups.as_ref().and_then(|input| {
            let cap =
                reservoir::bytes::<ResidualSampleGroup>(input.len()).min(u128::from(available));
            let mut output =
                reservoir::reserve(input.len(), cap, u128::from(self.retained_bytes), &mut peak)?;
            output.extend_from_slice(input);
            Some(output)
        });
        Self {
            seed: self.seed,
            byte_limit: self.byte_limit,
            retained_bytes: self.retained_bytes,
            peak_bytes: u64::try_from(peak).unwrap_or(u64::MAX),
            worlds: self.worlds,
            batches: self.batches,
            groups,
        }
    }
}

#[cfg(all(feature = "gpu", test))]
mod tests;
