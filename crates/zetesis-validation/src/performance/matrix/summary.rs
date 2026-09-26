use super::{Decision, Producer, Report};
use crate::performance::{
    Phase,
    series::{Timing, median},
};
use serde::Serialize;
use std::collections::BTreeMap;

/// Count of one actual disposition, including qualifications and skipped slots.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct DecisionCount {
    /// Actual outcome; failures never enter successful timing populations.
    pub decision: Decision,
    /// Fixed schedule positions with this outcome.
    pub positions: usize,
}

/// One source/producer cell's compact observations, without raw answer streams.
#[derive(Clone, Debug, Serialize)]
pub struct CellSummary {
    /// Index into the report's unchanged source-entry list.
    pub case: usize,
    /// Reference solver or requested native profile index.
    pub producer: Producer,
    /// Counts over every requested phase, including unlaunched positions.
    pub decisions: Vec<DecisionCount>,
    /// Non-pass explanations grouped by schedule phase and original blocker.
    /// Details come from retained samples; missing legacy reasons stay explicit.
    pub reasons: BTreeMap<String, usize>,
    /// Available only when every timed position passed and its duration fits u64.
    pub timing: Option<Timing>,
    /// Median of successfully validated separate memory rounds, or unavailable.
    pub peak_rss_bytes: Option<u64>,
}

/// Compact typed campaign view; input identities and profiles remain borrowed.
/// Computing it scans retained positions and copies durations, counts and
/// failure details. It never serializes or decodes raw captured answer streams.
#[derive(Debug, Serialize)]
pub struct Summary<'a> {
    /// Version of this compact view, distinct from full campaign report schemas.
    pub schema: u32,
    /// Stable machine-view identifier.
    pub format: &'static str,
    /// Every campaign requirement qualified.
    pub passed: bool,
    /// Every fixed schedule position has a retained disposition.
    pub accounted: bool,
    /// Source entry paths in fixed case order.
    pub cases: &'a [String],
    /// Exact generated or constant-amended workload identities, when selected.
    pub workloads: Option<&'a [super::Workload]>,
    /// Exact requested native profiles.
    pub profiles: &'a [crate::selected::NativeExecution],
    /// Whether the reference was measured or used only for its census.
    pub reference_policy: super::ReferencePolicy,
    /// Sealed executable and source identities.
    pub before: &'a [crate::selected::FileSeal],
    /// Source-major, reference-first compact cells.
    pub cells: Vec<CellSummary>,
}

pub(super) fn summarize(report: &Report) -> Summary<'_> {
    let mut cells = Vec::new();
    for case in 0..report.cases.len() {
        for producer in std::iter::once(Producer::Reference)
            .chain((0..report.plan.profiles.len()).map(|profile| Producer::Native { profile }))
        {
            let mut decisions: Vec<DecisionCount> = Vec::new();
            let mut reasons = BTreeMap::new();
            let mut intervals = Some(Vec::new());
            let mut memory = Vec::new();
            for sample in report
                .samples
                .iter()
                .filter(|sample| sample.slot.case == case && sample.slot.producer == producer)
            {
                if let Some(count) = decisions
                    .iter_mut()
                    .find(|count| count.decision == sample.decision)
                {
                    count.positions += 1;
                } else {
                    decisions.push(DecisionCount {
                        decision: sample.decision,
                        positions: 1,
                    });
                }
                if sample.decision != Decision::Pass {
                    *reasons.entry(reason(sample, &report.samples)).or_insert(0) += 1;
                }
                if sample.slot.phase == Phase::Timed {
                    let duration = (sample.decision == Decision::Pass)
                        .then(|| sample.capture.as_ref()?.elapsed_ns()?.try_into().ok())
                        .flatten();
                    match (&mut intervals, duration) {
                        (Some(intervals), Some(duration)) => intervals.push(duration),
                        _ => intervals = None,
                    }
                }
                if sample.slot.phase == Phase::Memory && sample.decision == Decision::Pass {
                    memory.extend(sample.memory.map(|memory| memory.peak_rss_bytes));
                }
            }
            let timing = intervals.and_then(|mut values| {
                let median_ns = median(&mut values)?;
                Some(Timing {
                    samples: values.len(),
                    minimum_ns: values[0],
                    median_ns,
                    maximum_ns: values[values.len() - 1],
                })
            });
            cells.push(CellSummary {
                case,
                producer,
                decisions,
                reasons,
                timing,
                peak_rss_bytes: median(&mut memory),
            });
        }
    }
    Summary {
        schema: 1,
        format: "zetesis_benchmark_summary",
        passed: report.passed(),
        accounted: report.accounted(),
        cases: &report.cases,
        workloads: report.workloads.as_deref(),
        profiles: &report.plan.profiles,
        reference_policy: report.plan.reference_policy,
        before: &report.before,
        cells,
    }
}

fn reason(sample: &super::Sample, samples: &[super::Sample]) -> String {
    let detail = sample
        .detail
        .as_deref()
        .unwrap_or("reason unavailable in retained sample");
    let mut reason = format!("{:?}: {:?}: {detail}", sample.slot.phase, sample.decision);
    if let Some(index) = sample.blocked_by {
        use std::fmt::Write;
        if let Some(blocker) = samples.get(index) {
            write!(
                reason,
                "; blocked by sample {index} ({:?}, {:?}): {}",
                blocker.slot.phase,
                blocker.decision,
                blocker
                    .detail
                    .as_deref()
                    .unwrap_or("reason unavailable in retained sample"),
            )
            .expect("writing to a String cannot fail");
        } else {
            write!(reason, "; blocking sample {index} unavailable")
                .expect("writing to a String cannot fail");
        }
    }
    reason
}
