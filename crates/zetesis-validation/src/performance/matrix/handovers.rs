//! Exact regional handover observations and their extraction from native JSON.
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, value::RawValue};

use super::{DeviceWork, FormulaResidualStatistics};

/// The device handover attempts and accumulated observed CPU query work.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidualHandoverStatistics {
    /// Ticket attempts including retries.
    pub attempts: u64,
    /// Attempts that ended without publication.
    pub incomplete_attempts: u64,
    /// Slates published after completion.
    pub committed_slates: u64,
    /// Committed slates with no GPU residual.
    pub zero_residual_slates: u64,
    /// Observed query work, including performed prefixes.
    pub query_work: u128,
    /// Every entered query is represented in the work sum.
    pub complete_work: bool,
    /// Cumulative attempt counts remain representable.
    pub counts_exact: bool,
    /// Latest ticket attempt when its ordinal was retained.
    #[serde(deserialize_with = "nullable")]
    pub last_attempt: Option<ResidualHandoverAttempt>,
}

/// The observed stages of one ticket, including an interrupted prefix.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidualHandoverAttempt {
    /// One-based session attempt ordinal.
    pub ordinal: u64,
    /// In-stream proposal sequence, shared by retries of the same slate.
    pub slate_sequence: u64,
    /// The outstanding ticket was reissued.
    pub retry: bool,
    /// All submitted prefixes were decoded and authenticated.
    pub decode_complete: bool,
    /// The entire slate was published.
    pub committed: bool,
    /// Counts from the accepted GPU decode prefix.
    pub residuals: FormulaResidualStatistics,
    /// Open semantic atom count summed across that prefix.
    pub open_atoms: u128,
    /// Peak actual storage for the optional per-slot evidence.
    pub peak_observation_bytes: u64,
    /// CPU search summary when the receipt for this attempt was available.
    #[serde(deserialize_with = "nullable")]
    pub search: Option<ResidualSearchStatistics>,
    /// Search and open-atom totals associated with the same original slots as
    /// their decoded GPU reasons, when both ends of the join were retained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by_reason: Option<ResidualHandoverByReason>,
}

/// The three buckets of an observed slot-to-reason join.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidualHandoverByReason {
    /// The device reached a propagation fixed point.
    pub fixed_point: ResidualReasonSearch,
    /// The device reached its sweep ceiling.
    pub round_limit: ResidualReasonSearch,
    /// The device could not admit its next sweep.
    pub work_limit: ResidualReasonSearch,
}

/// Joined outcome and performed-work totals for one decoded reason.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidualReasonSearch {
    /// Open semantic atoms on this reason's handed-over roots.
    pub open_atoms: u128,
    /// Residual slots that did not enter completion.
    pub unentered: u64,
    /// Slots settled by an existing CPU certificate.
    pub certified: u64,
    /// Exhausted regional queries.
    pub exhausted: u64,
    /// Independently validated countermodels.
    pub witnesses: u64,
    /// Interrupted regional queries.
    pub stopped: u64,
    /// Actual region visits.
    pub visited: u128,
    /// Attempted split decisions.
    pub attempted_splits: u128,
    /// Successful split publications.
    pub published_splits: u128,
    /// Performed regional work including stopped prefixes.
    pub work: u128,
}
/// Exact per-attempt counts from the regional CPU completion owner.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidualSearchStatistics {
    /// Residual slots not entered.
    pub unentered: u64,
    /// Slots settled by a CPU certificate.
    pub certified: u64,
    /// Queries exhausted inside their regions.
    pub exhausted: u64,
    /// Queries returning an independently validated witness.
    pub witnesses: u64,
    /// Queries stopped before a membership decision.
    pub stopped: u64,
    /// Actual visited regions across the queries.
    pub visited: u128,
    /// Attempted split decisions, including refused publication.
    pub attempted_splits: u128,
    /// Successful split publications.
    pub published_splits: u128,
    /// Greatest measured split depth when available.
    #[serde(deserialize_with = "nullable")]
    pub deepest: Option<usize>,
    /// Maximum of the sums of visits at each depth, when available.
    #[serde(deserialize_with = "nullable")]
    pub width: Option<u64>,
    /// Performed regional search work.
    pub work: u128,
    /// Sum of measured per-query reduct-phase intervals, in nanoseconds.
    #[serde(deserialize_with = "nullable")]
    pub host_time_ns: Option<u128>,
}

// An optional observation is explicit null inside a present typed receipt.
// Historical documents may omit the entire handover object.
fn nullable<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
    input: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(input)
}

#[derive(Deserialize)]
struct Document<'a> {
    #[serde(borrow)]
    statistics: Statistics<'a>,
}
#[derive(Deserialize)]
struct Statistics<'a> {
    #[serde(borrow)]
    execution: Execution<'a>,
}
#[derive(Deserialize)]
struct Execution<'a> {
    #[serde(borrow)]
    gpu_residual_handovers: &'a RawValue,
}

/// Retain a typed receipt only on the general formula device route. The normal
/// decoded Value suffices unless a wide integer lost precision during parsing.
pub(super) fn attach(
    device: &mut DeviceWork,
    document: &Value,
    source: Option<&[u8]>,
) -> Result<(), String> {
    let value = &document["statistics"]["execution"]["gpu_residual_handovers"];
    if value.is_null() {
        return Ok(());
    }
    let DeviceWork::Formula {
        gpu_residual_handovers,
        ..
    } = device
    else {
        return Err("GPU residual handovers require general formula device execution".into());
    };
    let receipt = match ResidualHandoverStatistics::deserialize(value) {
        Ok(measured) => measured,
        Err(initial) => {
            let raw = source
                .ok_or_else(|| format!("invalid or inexact GPU residual handover: {initial}"))?;
            let fields: Document<'_> = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
            serde_json::from_str(fields.statistics.execution.gpu_residual_handovers.get())
                .map_err(|e| format!("invalid GPU residual handover: {e}"))?
        }
    };
    receipt.check()?;
    *gpu_residual_handovers = Some(Box::new(receipt));
    Ok(())
}

impl ResidualHandoverStatistics {
    fn check(&self) -> Result<(), String> {
        if self.counts_exact
            && (self
                .committed_slates
                .checked_add(self.incomplete_attempts)
                .is_none_or(|sum| sum > self.attempts)
                || self.zero_residual_slates > self.committed_slates
                || self.last_attempt.as_ref().map_or(self.attempts != 0, |a| {
                    a.ordinal == 0 || a.ordinal != self.attempts
                }))
        {
            return Err("inconsistent residual handover attempt counts".into());
        }
        let Some(attempt) = &self.last_attempt else {
            return Ok(());
        };
        if attempt.committed && !attempt.decode_complete {
            return Err("committed residual handover has incomplete decode".into());
        }
        let Some(search) = &attempt.search else {
            if let Some(groups) = &attempt.by_reason
                && !(attempt.decode_complete
                    && attempt.open_atoms == 0
                    && [
                        attempt.residuals.fixed_point,
                        attempt.residuals.round_limit,
                        attempt.residuals.work_limit,
                    ] == [0; 3]
                    && *groups == ResidualHandoverByReason::default())
            {
                return Err("residual reason join lacks its CPU search receipt".into());
            }
            return Ok(());
        };
        let reported = u128::from(attempt.residuals.round_limit)
            + u128::from(attempt.residuals.work_limit)
            + u128::from(attempt.residuals.fixed_point);
        let slots = [
            search.unentered,
            search.certified,
            search.exhausted,
            search.witnesses,
            search.stopped,
        ]
        .into_iter()
        .map(u128::from)
        .sum::<u128>();
        if !attempt.decode_complete
            || slots != reported
            || search.work > self.query_work
            || search.published_splits > search.attempted_splits
            || search.width.is_some_and(|w| u128::from(w) > search.visited)
            || search.deepest.is_some_and(|d| d as u128 >= search.visited)
            || attempt.committed && (search.unentered != 0 || search.stopped != 0)
        {
            return Err("inconsistent joined residual search counts".into());
        }
        if let Some(groups) = &attempt.by_reason {
            groups.check(attempt, search)?;
        }
        Ok(())
    }
}

impl ResidualHandoverByReason {
    fn check(
        &self,
        attempt: &ResidualHandoverAttempt,
        summary: &ResidualSearchStatistics,
    ) -> Result<(), String> {
        let groups = [
            (&self.fixed_point, attempt.residuals.fixed_point),
            (&self.round_limit, attempt.residuals.round_limit),
            (&self.work_limit, attempt.residuals.work_limit),
        ];
        for (bucket, count) in groups {
            let slots = [
                bucket.unentered,
                bucket.certified,
                bucket.exhausted,
                bucket.witnesses,
                bucket.stopped,
            ]
            .into_iter()
            .map(u128::from)
            .sum::<u128>();
            if slots != u128::from(count) || bucket.published_splits > bucket.attempted_splits {
                return Err("residual CPU slots contradict their decoded reason".into());
            }
        }
        // Each additive field covers the same original slot population.
        let agrees = |expected: u128, field: fn(&ResidualReasonSearch) -> u128| {
            groups.iter().try_fold(0_u128, |total, (bucket, _)| {
                total.checked_add(field(bucket))
            }) == Some(expected)
        };
        if !agrees(attempt.open_atoms, |s| s.open_atoms)
            || !agrees(u128::from(summary.unentered), |s| u128::from(s.unentered))
            || !agrees(u128::from(summary.certified), |s| u128::from(s.certified))
            || !agrees(u128::from(summary.exhausted), |s| u128::from(s.exhausted))
            || !agrees(u128::from(summary.witnesses), |s| u128::from(s.witnesses))
            || !agrees(u128::from(summary.stopped), |s| u128::from(s.stopped))
            || !agrees(summary.visited, |s| s.visited)
            || !agrees(summary.attempted_splits, |s| s.attempted_splits)
            || !agrees(summary.published_splits, |s| s.published_splits)
            || !agrees(summary.work, |s| s.work)
        {
            return Err("residual per-reason totals contradict their joined search summary".into());
        }
        Ok(())
    }
}
