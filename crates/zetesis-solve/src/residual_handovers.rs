//! Exact observations at the GPU decode, CPU search and slate publication boundaries.

use std::time::Duration;

use crate::FormulaResidualStatistics;

#[cfg(feature = "gpu")]
pub(crate) mod decoded;
pub(crate) mod sampling;

/// Cumulative handover attempts and the latest attempt in this solver session.
/// A retry retains previously performed work. Only successful publication counts a slate.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ResidualHandoverStatistics {
    /// Attempts that obtained an authenticated slate ticket.
    pub attempts: u64,
    /// Attempts that ended before publication.
    pub incomplete_attempts: u64,
    /// Successfully published slates.
    pub committed_slates: u64,
    /// Published slates that required no residual handover.
    pub zero_residual_slates: u64,
    /// Observed CPU query work, including performed prefixes of interrupted attempts.
    pub query_work: u128,
    /// Whether all entered query work is included in the cumulative sum.
    pub complete_work: bool,
    /// Whether the cumulative attempt counts remain representable.
    pub counts_exact: bool,
    /// Latest attempted slate, absent before the first ticket or after ordinal overflow.
    pub last_attempt: Option<ResidualHandoverAttempt>,
    /// Observation coverage for committed worlds and batches. At settlement this
    /// includes available quantiles over the committed prefix. The enclosing
    /// outcome reports whether the solve completed or stopped.
    pub samples: Option<sampling::ResidualRunSamples>,
}

impl Default for ResidualHandoverStatistics {
    fn default() -> Self {
        Self {
            attempts: 0,
            incomplete_attempts: 0,
            committed_slates: 0,
            zero_residual_slates: 0,
            query_work: 0,
            complete_work: true,
            counts_exact: true,
            last_attempt: None,
            samples: None,
        }
    }
}

/// One ticket attempt. Until decoding finishes, reason counts and open atoms
/// describe the authenticated prefix that reached the coordinator.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct ResidualHandoverAttempt {
    /// One-based attempt ordinal within this session.
    pub ordinal: u64,
    /// Proposal sequence within this stream. Retrying uses the same sequence.
    pub slate_sequence: u64,
    /// The stream reissued its oldest uncommitted slate.
    pub retry: bool,
    /// The whole slate passed the device prefix checks.
    pub decode_complete: bool,
    /// The complete slate was published.
    pub committed: bool,
    /// Decoded device residuals by reason, before CPU certification or search.
    pub residuals: FormulaResidualStatistics,
    /// Sum of open semantic atoms over those residuals.
    pub open_atoms: u128,
    /// Joined slot observations, absent before completion or if recording was unavailable.
    pub search: Option<ResidualSearchStatistics>,
    /// Original-slot observations grouped by decoded reason, when both streams were available.
    pub by_reason: Option<ResidualHandoverByReason>,
    /// Peak admitted optional observation payload during this attempt.
    pub peak_observation_bytes: u64,
}

/// Aggregate of the original residual slots in one CPU completion attempt.
/// An interrupted query contributes only the work it performed.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct ResidualSearchStatistics {
    /// Summed terminal dispositions and performed work.
    pub totals: ResidualReasonSearch,
    /// Deepest visited level, when at least one level was visited and all depths were observed.
    pub deepest: Option<usize>,
    /// Maximum of the summed visits per depth across the slate, when observed.
    pub width: Option<u64>,
    /// Sum of per-query host intervals, which may overlap in wall time.
    pub host_time: Option<Duration>,
}

/// Search observations joined to their GPU reasons using the original slate slots.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct ResidualHandoverByReason {
    /// The device reached a propagation fixed point.
    pub fixed_point: ResidualReasonSearch,
    /// The device reached its round allowance.
    pub round_limit: ResidualReasonSearch,
    /// The next sweep did not fit the device work allowance.
    pub work_limit: ResidualReasonSearch,
}

/// Exact sums and dispositions over a population of handed-over regions.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct ResidualReasonSearch {
    /// Open semantic atoms across the decoded regions.
    pub open_atoms: u128,
    /// Residual slots that did not enter certification or search.
    pub unentered: u64,
    /// Slots settled by an existing CPU certificate without a regional search.
    pub certified: u64,
    /// Queries that found no countermodel in the supplied region.
    pub exhausted: u64,
    /// Queries that independently validated a proper countermodel.
    pub witnesses: u64,
    /// Slots that stopped during checking.
    pub stopped: u64,
    /// Regions visited by the entered queries.
    pub visited: u128,
    /// Attempted decisions, including an attempt whose children could not be published.
    pub attempted_splits: u128,
    /// Splits that published both children.
    pub published_splits: u128,
    /// Work actually performed by these queries.
    pub work: u128,
}

#[cfg(feature = "gpu")]
impl ResidualReasonSearch {
    fn observe(&mut self, slot: &zetesis_stable::RegionalSlotObservation) {
        use zetesis_stable::RegionalSlotState;
        match slot.state {
            RegionalSlotState::Unentered => self.unentered += 1,
            RegionalSlotState::Certified => self.certified += 1,
            RegionalSlotState::NoWitnessInSuppliedRegion => self.exhausted += 1,
            RegionalSlotState::ValidatedCountermodel => self.witnesses += 1,
            RegionalSlotState::Stopped(_) => self.stopped += 1,
        }
        if let Some(query) = &slot.query {
            self.work += u128::from(query.work);
            self.visited += query.visited as u128;
            self.attempted_splits += query.attempted_splits as u128;
            self.published_splits += query.published_splits as u128;
        }
    }
}

#[cfg(feature = "gpu")]
impl ResidualHandoverByReason {
    fn from_slots(
        decoded: &decoded::DecodeJournal,
        slots: &[Option<zetesis_stable::RegionalSlotObservation>],
    ) -> Option<Self> {
        let mut groups = Self::default();
        let mut keys = decoded.rows()?.iter();
        for (index, observation) in slots.iter().enumerate() {
            let Some(observation) = observation else {
                continue;
            };
            let key = keys.next()?;
            if key.slot != index {
                return None;
            }
            let group = match key.reason {
                zetesis_wgpu::ResidualReason::FixedPoint => &mut groups.fixed_point,
                zetesis_wgpu::ResidualReason::RoundLimit => &mut groups.round_limit,
                zetesis_wgpu::ResidualReason::WorkLimit => &mut groups.work_limit,
            };
            group.open_atoms += u128::from(key.open_atoms);
            group.observe(observation);
        }
        keys.next().is_none().then_some(groups)
    }
}

#[cfg(feature = "gpu")]
pub(crate) fn open_atoms(root: &zetesis_algebra::region::Region) -> u64 {
    let (held, cut) = root.decision_words();
    let decided: usize = held
        .iter()
        .zip(cut)
        .map(|(h, c)| (h | c).count_ones() as usize)
        .sum();
    (root.len() - decided) as u64
}

#[cfg(feature = "gpu")]
impl ResidualHandoverStatistics {
    pub(crate) fn begin(&mut self, slate_sequence: u64, retry: bool) {
        let Some(next) = self.attempts.checked_add(1) else {
            self.last_attempt = None;
            self.counts_exact = false;
            self.complete_work = false;
            return;
        };
        self.attempts = next;
        self.last_attempt = Some(ResidualHandoverAttempt {
            ordinal: next,
            slate_sequence,
            retry,
            ..ResidualHandoverAttempt::default()
        });
    }

    pub(crate) fn decoded(&mut self, reason: zetesis_wgpu::ResidualReason, open_atoms: u64) {
        let Some(attempt) = &mut self.last_attempt else {
            return;
        };
        *attempt.residuals.count_for(reason) += 1;
        attempt.open_atoms += u128::from(open_atoms);
    }

    pub(crate) fn join(
        &mut self,
        receipt: Option<&zetesis_stable::RegionalObservations>,
        decoded: &decoded::DecodeJournal,
        may_have_queried: bool,
    ) {
        let Some(attempt) = &mut self.last_attempt else {
            return;
        };
        let Some((receipt, slots)) = receipt.and_then(|r| Some((r, r.slots()?))) else {
            if may_have_queried && attempt.residuals != FormulaResidualStatistics::default() {
                self.complete_work = false;
            }
            return;
        };
        let mut search = ResidualSearchStatistics {
            width: receipt.width(),
            host_time: receipt.host_time(),
            ..ResidualSearchStatistics::default()
        };
        search.totals.open_atoms = attempt.open_atoms;
        let mut depths_complete = true;
        for slot in slots.iter().flatten() {
            search.totals.observe(slot);
            if let Some(query) = &slot.query {
                depths_complete &= query.visited == 0 || query.deepest.is_some();
                search.deepest = search.deepest.max(query.deepest);
            }
        }
        if !depths_complete {
            search.deepest = None;
        }
        if let Some(total) = self.query_work.checked_add(search.totals.work) {
            self.query_work = total;
        } else {
            self.complete_work = false;
            self.query_work = u128::MAX;
        }
        attempt.by_reason = ResidualHandoverByReason::from_slots(decoded, slots);
        attempt.search = Some(search);
    }

    pub(crate) fn finish(&mut self, committed: bool) {
        let Some(attempt) = &mut self.last_attempt else {
            return;
        };
        attempt.committed = committed;
        if committed {
            self.committed_slates += 1;
            if attempt.residuals == FormulaResidualStatistics::default() {
                self.zero_residual_slates += 1;
            }
        } else {
            self.incomplete_attempts += 1;
        }
    }
}

#[cfg(all(test, feature = "gpu"))]
mod tests;
