//! A GPU enumeration's two-phase slates against a device double: the next slate
//! is proposed while the current slate's last submission is checked, with the
//! same family, slate sizes and failure semantics as the serial schedule.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::time::Duration;

use zetesis_backend::BatchComputeTime;

use zetesis_algebra::Cancellation;
use zetesis_ferraris::{Interpretation, SlateId, SlateRef, Theory};
use zetesis_stable::{
    BatchLimits, BatchVerdict, CompletionExecutor, Incomplete, Limits, Lookahead, SearchLimits,
    SlateProposal, StableModels,
};

use super::device::{Decoded, PreparedSubmission, SlateDevice};
use super::{BatchQueue, Demand};
use crate::batch_executor::DeviceVerdicts;
use crate::formula_execution::Failure;
use crate::phase_timing::Recorder;
use crate::{GpuBatching, GpuLookahead, GpuLookaheadDeclined, SolveConfig, SolveError};

/// When the double raises the control during a slate's last submission.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stop {
    Never,
    /// Inside the production run beside the submission.
    DuringProduction,
    /// After the production returned, while the finish waits.
    DuringFinish,
}

/// Which candidates the double decides on the device.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Decides {
    /// Every candidate, exactly: the tight route, which completes no
    /// residuals on the host.
    Wholly,
    /// None: every candidate is a residual for the host's exact completion,
    /// as the hybrid route's general propagation can leave it.
    Residuals,
    /// The first slate wholly and every later slate's candidates as
    /// residuals: a hybrid route whose pending slate the device decides.
    FirstSlateWholly,
}

/// The double's verdict on a candidate of a slate, the first or a later one.
fn decided(
    decides: Decides,
    first: bool,
    theory: &Theory,
    candidate: &Interpretation,
) -> BatchVerdict {
    if decides == Decides::Residuals || (decides == Decides::FirstSlateWholly && !first) {
        return BatchVerdict::Residual;
    }
    let check = zetesis_ferraris::check(
        theory,
        candidate,
        zetesis_ferraris::Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    if check.accepted() {
        BatchVerdict::NoProperSubset
    } else {
        BatchVerdict::Refuted
    }
}

/// A device that decides candidates as `decides` says, exactly where it
/// decides them, admits at most `prefix` candidates per submission, and
/// records what the queue asked of it.
struct Double {
    prefix: usize,
    decides: Decides,
    stop: Stop,
    control: Cancellation,
    in_flight: usize,
    peak_in_flight: usize,
    overlapped: usize,
    /// Each slate's candidate count, at its first submission.
    slates: Vec<usize>,
    /// Whether every submission's completion was observed.
    reusable: bool,
    /// Each submission's compute interval, in submission order. The double
    /// reports one with the next submission, as the device's timer reports a
    /// batch's interval one call late, and none past the script.
    intervals: Vec<Option<BatchComputeTime>>,
    /// Each checked submission's finish wait, in submission order, when it
    /// was checked beside the host's production.
    waits: Vec<Option<Duration>>,
}

impl Double {
    fn new(prefix: usize, control: &Cancellation) -> Self {
        Self {
            prefix,
            decides: Decides::Wholly,
            stop: Stop::Never,
            control: control.clone(),
            in_flight: 0,
            peak_in_flight: 0,
            overlapped: 0,
            slates: Vec::new(),
            reusable: true,
            intervals: Vec::new(),
            waits: Vec::new(),
        }
    }
}

struct Receipt {
    slate: SlateId,
    offset: usize,
    verdicts: Vec<BatchVerdict>,
    /// The submission's place among every submission the double checked.
    index: usize,
    finish_wait: Option<Duration>,
}

struct Prepared<'d> {
    double: &'d mut Double,
    receipt: Receipt,
}

impl Prepared<'_> {
    /// Submit, recording the submission's finish wait: a distinct duration
    /// for each submission checked beside the host's production.
    fn submit(&mut self, overlapped: bool) {
        self.double.in_flight += 1;
        self.double.peak_in_flight = self.double.peak_in_flight.max(self.double.in_flight);
        self.receipt.index = self.double.waits.len();
        let wait = overlapped.then(|| Duration::from_micros(7 * (self.receipt.index as u64 + 1)));
        self.receipt.finish_wait = wait;
        self.double.waits.push(wait);
    }

    fn finish(self, stopped: bool) -> Result<Receipt, Failure> {
        self.double.in_flight -= 1;
        if stopped {
            return Err(Failure::Search(zetesis_stable::Incomplete::Cancelled));
        }
        Ok(self.receipt)
    }
}

impl SlateDevice for Double {
    type Prepared<'d> = Prepared<'d>;
    type Checked = Result<Receipt, Failure>;

    fn prepare<'d>(
        &'d mut self,
        slate: SlateRef<'_>,
        offset: usize,
        _max_bytes: u64,
        cancellation: &Cancellation,
    ) -> Result<Prepared<'d>, Failure> {
        cancellation
            .poll()
            .map_err(|stop| Failure::Search(stop.into()))?;
        assert_eq!(self.in_flight, 0, "a submission is prepared beside another");
        if offset == 0 {
            self.slates.push(slate.candidates().len());
        }
        let first = self.slates.len() == 1;
        let rest = &slate.candidates()[offset..];
        let admitted = &rest[..rest.len().min(self.prefix)];
        let verdicts = admitted
            .iter()
            .map(|candidate| decided(self.decides, first, slate.theory(), candidate))
            .collect();
        Ok(Prepared {
            double: self,
            receipt: Receipt {
                slate: slate.id(),
                offset,
                verdicts,
                index: 0,
                finish_wait: None,
            },
        })
    }

    fn decode(
        &mut self,
        checked: Self::Checked,
        _theory: &Theory,
        slate: SlateId,
        offset: usize,
        offered: usize,
        rows: &mut super::device::CheckedRows,
    ) -> Result<Decoded, Failure> {
        let receipt = checked?;
        // As a device receipt does, refuse another slate's or another offset's.
        if receipt.slate != slate
            || receipt.offset != offset
            || receipt.verdicts.is_empty()
            || receipt.verdicts.len() > offered
        {
            return Err(Failure::Run(SolveError::FormulaBatchShape {
                expected: offered,
                actual: receipt.verdicts.len(),
            }));
        }
        let compute = receipt
            .index
            .checked_sub(1)
            .and_then(|previous| self.intervals.get(previous).copied().flatten());
        for verdict in receipt.verdicts {
            rows.classified(verdict)?;
        }
        Ok(Decoded {
            compute,
            finish_wait: receipt.finish_wait,
        })
    }
}

impl PreparedSubmission for Prepared<'_> {
    type Checked = Result<Receipt, Failure>;

    fn count(&self) -> usize {
        self.receipt.verdicts.len()
    }

    fn check(mut self, cancellation: &Cancellation) -> Self::Checked {
        self.submit(false);
        let stopped = cancellation.poll().is_err();
        self.finish(stopped)
    }

    fn check_overlapping<R>(
        mut self,
        cancellation: &Cancellation,
        during: impl FnOnce() -> R,
    ) -> (Self::Checked, Option<R>) {
        self.submit(true);
        self.double.overlapped += 1;
        if self.double.stop == Stop::DuringProduction {
            self.double.control.cancel();
        }
        let result = during();
        if self.double.stop == Stop::DuringFinish {
            self.double.control.cancel();
            // The finish polls and returns the stop, completion unobserved.
            self.double.reusable = false;
            return (self.finish(true), Some(result));
        }
        // The finish observes completion without polling the control and
        // returns the decoded checks, which the queue drops on its own poll.
        let _ = cancellation;
        (self.finish(false), Some(result))
    }
}

fn options(batch: usize) -> SolveConfig {
    SolveConfig {
        gpu_batching: GpuBatching::Adaptive,
        batch_size: NonZeroUsize::new(batch).unwrap(),
        ..Default::default()
    }
}

fn stream(
    source: &str,
    cancellation: &Cancellation,
) -> (zetesis_themelios::AdmittedFormula, StableModels) {
    let admitted = zetesis_reference_support::formula(source);
    let models =
        StableModels::new(admitted.theory(), Limits::default(), cancellation.clone()).unwrap();
    (admitted, models)
}

/// Every answer of a device enumeration under `demand`, with the queue.
fn enumerate(
    source: &str,
    options: &SolveConfig,
    demand: Demand,
    double: &mut Double,
    cancellation: &Cancellation,
) -> (BTreeSet<Vec<usize>>, BatchQueue) {
    let (_admitted, mut models) = stream(source, cancellation);
    let mut queue = BatchQueue::for_joint_oracle(options).unwrap();
    queue.set_demand(demand);
    let phases = Recorder::new(false);
    let mut family = BTreeSet::new();
    while let Some(next) = queue.next_device(&mut models, options, cancellation, &phases, double) {
        let Ok(model) = next else {
            panic!("the enumeration must complete");
        };
        assert_eq!(
            double.in_flight, 0,
            "a submission is in flight between calls"
        );
        assert!(family.insert(model.atoms().collect()));
    }
    assert!(models.exhausted());
    (family, queue)
}

/// The serial schedule's family and slate sizes, by the closure form.
fn serial(source: &str, options: &SolveConfig) -> (BTreeSet<Vec<usize>>, Vec<usize>) {
    let cancellation = Cancellation::default();
    let (_admitted, mut models) = stream(source, &cancellation);
    let mut queue = BatchQueue::for_joint_oracle(options).unwrap();
    let mut family = BTreeSet::new();
    let mut slates = Vec::new();
    while let Some(next) = queue.next(&mut models, options, &cancellation, |batch| {
        slates.push(batch.candidates().len());
        let mut verdicts =
            DeviceVerdicts::new(batch.candidates().len()).map_err(Failure::Search)?;
        for candidate in batch.candidates() {
            let check = zetesis_ferraris::check(
                batch.theory(),
                candidate,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            verdicts.values.push(if check.accepted() {
                BatchVerdict::NoProperSubset
            } else {
                BatchVerdict::Refuted
            });
        }
        verdicts.finish(batch).map_err(Failure::from)
    }) {
        let Ok(model) = next else {
            panic!("the serial enumeration must complete");
        };
        family.insert(model.atoms().collect());
    }
    (family, slates)
}

const UNBOUNDED: Demand = Demand {
    enumerates: true,
    owed: None,
};

const CHOICES: &str = "{a;b;c;d;e}.";
const CONSTRAINED: &str = "{a;b;c;d;e}. :- a, b. :- c, not d.";

#[test]
fn an_enumeration_proposes_slates_ahead() {
    let cancellation = Cancellation::default();
    let mut double = Double::new(usize::MAX, &cancellation);
    let (family, queue) = enumerate(CHOICES, &options(2), UNBOUNDED, &mut double, &cancellation);
    assert_eq!(family.len(), 32);
    assert!(queue.lookahead().slates > 0);
    assert!(double.overlapped > 0);
    assert_eq!(queue.lookahead().decision, Some(GpuLookahead::Engaged));
}

#[test]
fn an_optimizing_demand_without_candidate_bounds_never_proposes_ahead() {
    // Pruning disabled declares no bounds, so only the demand refuses.
    let cancellation = Cancellation::default();
    let mut double = Double::new(usize::MAX, &cancellation);
    let demand = Demand {
        enumerates: false,
        owed: None,
    };
    let (family, queue) = enumerate(CHOICES, &options(2), demand, &mut double, &cancellation);
    assert_eq!(family.len(), 32);
    assert_eq!(queue.lookahead().slates, 0);
    assert_eq!(double.overlapped, 0);
    assert_eq!(
        queue.lookahead().decision,
        Some(GpuLookahead::Declined(GpuLookaheadDeclined::Optimizing))
    );
}

#[test]
fn families_equal_the_serial_queues() {
    for source in [CHOICES, CONSTRAINED] {
        let cancellation = Cancellation::default();
        let mut double = Double::new(usize::MAX, &cancellation);
        let (family, _) = enumerate(source, &options(2), UNBOUNDED, &mut double, &cancellation);
        assert_eq!(family, serial(source, &options(2)).0, "{source}");
    }
}

#[test]
fn two_owed_answers_beside_a_slate_of_64_propose_nothing_ahead() {
    let cancellation = Cancellation::default();
    let (_admitted, mut models) = stream(CHOICES, &cancellation);
    let options = options(64);
    let mut queue = BatchQueue::for_joint_oracle(&options).unwrap();
    queue.set_demand(Demand {
        enumerates: true,
        owed: Some(2),
    });
    let mut double = Double::new(usize::MAX, &cancellation);
    let phases = Recorder::new(false);
    assert!(matches!(
        queue.next_device(&mut models, &options, &cancellation, &phases, &mut double),
        Some(Ok(_))
    ));
    assert_eq!(double.overlapped, 0);
    assert_eq!(queue.lookahead().slates, 0);
    assert_eq!(
        queue.lookahead().decision,
        Some(GpuLookahead::Declined(
            GpuLookaheadDeclined::RequestedAnswers
        ))
    );
}

#[test]
fn one_submission_is_in_flight_at_most_and_none_when_next_returns() {
    // Submissions of two candidates split every larger slate.
    let cancellation = Cancellation::default();
    let mut double = Double::new(2, &cancellation);
    let (family, _) = enumerate(CHOICES, &options(3), UNBOUNDED, &mut double, &cancellation);
    assert_eq!(family.len(), 32);
    assert_eq!(double.peak_in_flight, 1);
    assert_eq!(double.in_flight, 0);
}

#[test]
fn a_receipt_is_bound_to_its_place_among_equal_count_submissions() {
    // Each slate of four splits into two submissions of two, which the
    // double accepts only at the offset the queue assembled.
    let cancellation = Cancellation::default();
    let mut double = Double::new(2, &cancellation);
    let options = SolveConfig {
        gpu_batching: GpuBatching::Fixed,
        ..options(4)
    };
    let (family, _) = enumerate(CHOICES, &options, UNBOUNDED, &mut double, &cancellation);
    assert_eq!(family, serial(CHOICES, &options).0);
}

#[test]
fn a_stop_during_the_lookahead_leaves_the_slate_pending_and_the_device_reusable() {
    let cancellation = Cancellation::default();
    let (_admitted, mut models) = stream(CHOICES, &cancellation);
    let options = options(4);
    let mut queue = BatchQueue::for_joint_oracle(&options).unwrap();
    queue.set_demand(UNBOUNDED);
    let mut double = Double::new(usize::MAX, &cancellation);
    double.stop = Stop::DuringProduction;
    let phases = Recorder::new(false);
    let next = queue.next_device(&mut models, &options, &cancellation, &phases, &mut double);
    assert!(matches!(next, Some(Err(Failure::Search(_)))));
    // The queue returns the stop without attempting the commit.
    assert_eq!(models.batch_statistics().completion_calls, 0);
    assert_eq!(models.batch_statistics().committed, 0);
    assert!(models.batch_statistics().pending >= 4);
    assert_eq!(queue.len(), 0);
    assert!(!models.exhausted());
    assert!(double.reusable);
    assert_eq!(double.in_flight, 0);
}

#[test]
fn a_stop_during_the_finish_leaves_both_slates_pending() {
    let cancellation = Cancellation::default();
    let (_admitted, mut models) = stream(CHOICES, &cancellation);
    let options = options(4);
    let mut queue = BatchQueue::for_joint_oracle(&options).unwrap();
    queue.set_demand(UNBOUNDED);
    let mut double = Double::new(usize::MAX, &cancellation);
    double.stop = Stop::DuringFinish;
    let phases = Recorder::new(false);
    let next = queue.next_device(&mut models, &options, &cancellation, &phases, &mut double);
    assert!(matches!(next, Some(Err(Failure::Search(_)))));
    assert_eq!(models.batch_statistics().committed, 0);
    // The slate under check, four candidates, and the slate proposed ahead
    // of it stay pending.
    assert!(models.batch_statistics().pending > 4);
    assert!(!models.exhausted());
}

#[test]
fn adaptive_slate_sizes_equal_the_serial_queues() {
    for prefix in [usize::MAX, 6] {
        let cancellation = Cancellation::default();
        let mut double = Double::new(prefix, &cancellation);
        let (_, _) = enumerate(CHOICES, &options(2), UNBOUNDED, &mut double, &cancellation);
        let mut checked = Double::new(prefix, &cancellation);
        let serial_options = options(2);
        let (_, _) = enumerate(
            CHOICES,
            &serial_options,
            Demand {
                enumerates: true,
                owed: Some(1),
            },
            &mut checked,
            &cancellation,
        );
        assert_eq!(double.slates, checked.slates, "prefix {prefix}");
        assert!(double.overlapped > 0);
        assert_eq!(checked.overlapped, 0);
    }
}

/// Fixed slates of `batch` candidates with scalar completion, so that a
/// fixture's cumulative search work is the same in every run.
fn fixed(batch: usize) -> SolveConfig {
    SolveConfig {
        gpu_batching: GpuBatching::Fixed,
        completion_workers: NonZeroUsize::MIN,
        ..options(batch)
    }
}

/// A stream whose cumulative search work is capped at `max_work`.
fn limited(
    source: &str,
    cancellation: &Cancellation,
    max_work: u64,
) -> (zetesis_themelios::AdmittedFormula, StableModels) {
    let admitted = zetesis_reference_support::formula(source);
    let limits = Limits {
        search: SearchLimits {
            max_work,
            ..SearchLimits::default()
        },
        ..Limits::default()
    };
    let models = StableModels::new(admitted.theory(), limits, cancellation.clone()).unwrap();
    (admitted, models)
}

/// The stream's cumulative search work at the points where the limit tests
/// place their ceilings: the two-phase calls the queue makes, in its order and
/// under its limits for fixed slates, without a work limit.
struct Work {
    /// After the first slate's production.
    produced: u64,
    /// After the slate proposed ahead of the first slate's commit.
    ahead: u64,
    /// After the first slate's commit, under the verdicts the double gives it.
    committed: u64,
    /// After the third slate's production, proposed ahead beside the second.
    third: u64,
}

fn work(source: &str, options: &SolveConfig, decides: Decides) -> Work {
    let cancellation = Cancellation::default();
    let (_admitted, mut models) = stream(source, &cancellation);
    let limits = BatchLimits {
        max_candidates: options.batch_size,
        max_pending_bytes: options.max_batch_bytes,
    };
    let mut completion = CompletionExecutor::with_scratch_limit(
        options.completion_workers,
        options.max_completion_scratch_bytes,
    )
    .unwrap();
    let SlateProposal::New(first) = models.propose_slate(limits).unwrap() else {
        panic!("expected the first slate");
    };
    let produced = models.statistics().search.work;
    assert!(matches!(
        models.propose_lookahead(limits).unwrap(),
        Lookahead::Proposed(_)
    ));
    let ahead = models.statistics().search.work;
    let verdicts = {
        let slate = models.slate(&first).unwrap();
        slate
            .candidates()
            .iter()
            .map(|candidate| decided(decides, true, slate.theory(), candidate))
            .collect()
    };
    models
        .commit_slate(first, verdicts, &mut completion)
        .unwrap();
    let committed = models.statistics().search.work;
    let SlateProposal::Ahead(_second) = models.propose_slate(limits).unwrap() else {
        panic!("expected the slate proposed ahead");
    };
    assert!(matches!(
        models.propose_lookahead(limits).unwrap(),
        Lookahead::Proposed(_)
    ));
    let third = models.statistics().search.work;
    Work {
        produced,
        ahead,
        committed,
        third,
    }
}

/// A ceiling strictly between two calibrated points and away from both, so
/// that no reservation at either boundary decides the case.
fn between(low: u64, high: u64) -> u64 {
    assert!(
        high >= low + 2,
        "no room for a ceiling between {low} and {high}"
    );
    low + (high - low) / 2
}

/// What a device enumeration of `CHOICES` under a work ceiling delivered
/// before its first failure, and what the stream and the double record then.
struct Limited {
    answers: usize,
    work_limit: bool,
    statistics: zetesis_stable::BatchStatistics,
    exhausted: bool,
    double: Double,
}

fn limited_run(max_work: u64, options: &SolveConfig, decides: Decides) -> Limited {
    let cancellation = Cancellation::default();
    let (_admitted, mut models) = limited(CHOICES, &cancellation, max_work);
    let mut queue = BatchQueue::for_joint_oracle(options).unwrap();
    queue.set_demand(UNBOUNDED);
    let mut double = Double::new(usize::MAX, &cancellation);
    double.decides = decides;
    let phases = Recorder::new(false);
    let mut answers = 0;
    let work_limit = loop {
        match queue.next_device(&mut models, options, &cancellation, &phases, &mut double) {
            Some(Ok(_)) => answers += 1,
            Some(Err(Failure::Search(Incomplete::WorkLimit))) => break true,
            Some(Err(_)) => panic!("the only failure is the work limit"),
            None => break false,
        }
    };
    Limited {
        answers,
        work_limit,
        statistics: models.batch_statistics(),
        exhausted: models.exhausted(),
        double,
    }
}

/// On the hybrid route a slate holding residuals completes them on the host
/// from the stream's cumulative budget, which the slate proposed ahead of its
/// commit has already drawn on (design §3.8, Goal 2's second difference). The
/// commit is refused: the limit is reported, the slate stays pending with its
/// candidates counted, nothing is delivered and no exhaustion is claimed.
fn assert_residual_commit_refused(max_work: u64, options: &SolveConfig) {
    let run = limited_run(max_work, options, Decides::Residuals);
    assert!(run.work_limit);
    assert_eq!(run.answers, 0);
    assert_eq!(run.statistics.committed, 0);
    assert!(run.statistics.pending >= options.batch_size.get());
    assert_eq!(run.double.slates, [options.batch_size.get()]);
    assert!(!run.exhausted);
}

#[test]
fn a_limit_reached_during_the_lookahead_refuses_a_residual_slates_commit() {
    let options = fixed(4);
    let work = work(CHOICES, &options, Decides::Residuals);
    assert_residual_commit_refused(between(work.produced, work.ahead), &options);
}

/// The ceiling lies above what the serial schedule's first slate draws, its
/// production and its completion, and below what it draws after the slate
/// proposed ahead: the lookahead's production brings the limit within the
/// pending slate's reach.
#[test]
fn a_limit_the_lookahead_brings_within_reach_refuses_a_residual_slates_commit() {
    let options = fixed(4);
    let work = work(CHOICES, &options, Decides::Residuals);
    let serial = work.produced + (work.committed - work.ahead);
    assert_residual_commit_refused(between(serial.max(work.ahead), work.committed), &options);
}

/// The answers a limited run delivers when every slate the device decides
/// wholly commits before the limit is reported: all of `CHOICES`'s candidates
/// are answers, and a later slate holding residuals cannot commit past it.
fn wholly_decided(run: &Limited, decides: Decides) -> usize {
    match decides {
        Decides::Wholly => run.double.slates.iter().sum(),
        Decides::FirstSlateWholly => run.double.slates[0],
        Decides::Residuals => 0,
    }
}

/// A slate the device decides wholly draws nothing at its commit, so a limit
/// the lookahead's production reaches lets it commit and is reported after
/// it, and after the partial slate proposed ahead if there is one. The
/// hybrid route's wholly decided slate and the tight route agree.
#[test]
fn a_wholly_decided_slate_commits_before_a_limit_reached_during_the_lookahead() {
    for decides in [Decides::FirstSlateWholly, Decides::Wholly] {
        let options = fixed(4);
        let work = work(CHOICES, &options, decides);
        let run = limited_run(between(work.produced, work.ahead), &options, decides);
        assert!(run.work_limit, "{decides:?}");
        assert_eq!(run.double.slates[0], 4, "{decides:?}");
        // The lookahead met the limit: the slate it proposed, if any, is partial.
        assert!(
            run.double.slates.get(1).is_none_or(|&count| count < 4),
            "{decides:?}"
        );
        assert_eq!(run.answers, wholly_decided(&run, decides), "{decides:?}");
        assert!(!run.exhausted, "{decides:?}");
    }
}

/// A limit the lookahead's production stops short of is not met by a wholly
/// decided slate's commit, which draws nothing: the slate commits, its
/// answers are delivered with no stop at that commit, the slate proposed ahead
/// is full, and the limit is met later, by whatever next draws on the budget.
/// The hybrid route's wholly decided slate and the tight route agree.
#[test]
fn a_wholly_decided_slate_commits_with_no_stop_when_the_lookahead_stops_short() {
    for decides in [Decides::FirstSlateWholly, Decides::Wholly] {
        let options = fixed(4);
        let work = work(CHOICES, &options, decides);
        assert_eq!(work.committed, work.ahead, "{decides:?}");
        let run = limited_run(between(work.ahead, work.third), &options, decides);
        assert_eq!(&run.double.slates[..2], [4, 4], "{decides:?}");
        assert!(run.work_limit, "{decides:?}");
        assert_eq!(run.answers, wholly_decided(&run, decides), "{decides:?}");
        assert!(!run.exhausted, "{decides:?}");
    }
}

/// Every submission measured, a distinct interval each, except `refused`,
/// whose interval the timer refused.
fn intervals(refused: Option<usize>) -> Vec<Option<BatchComputeTime>> {
    (0..64)
        .map(|index| {
            Some(if Some(index) == refused {
                BatchComputeTime::Unavailable
            } else {
                BatchComputeTime::Measured(Duration::from_micros(100 * (index as u64 + 1)))
            })
        })
        .collect()
}

/// The pairs the queue books for the submissions before `upto`: the measured
/// intervals and the waits of the overlapped ones whose interval was measured,
/// and the count of overlapped ones whose interval was not.
fn paired(double: &Double, upto: usize) -> (u64, u64, u64) {
    let nanos = |duration: Duration| u64::try_from(duration.as_nanos()).unwrap();
    let (mut device, mut waits, mut unmeasured) = (0, 0, 0);
    for (index, wait) in double.waits[..upto].iter().enumerate() {
        let Some(wait) = wait else { continue };
        match double.intervals[index] {
            Some(BatchComputeTime::Measured(interval)) => {
                device += nanos(interval);
                waits += nanos(*wait);
            }
            _ => unmeasured += 1,
        }
    }
    (device, waits, unmeasured)
}

/// The last overlapped submission's interval arrives only with the
/// settlement's flush, and is booked then with that submission's own wait:
/// until the flush neither sum includes the pair, and after it both do.
#[test]
fn the_last_overlapped_interval_is_booked_at_the_flush_with_its_own_wait() {
    let cancellation = Cancellation::default();
    let mut double = Double::new(usize::MAX, &cancellation);
    double.intervals = intervals(None);
    let (_, mut queue) = enumerate(CHOICES, &fixed(4), UNBOUNDED, &mut double, &cancellation);
    let last = double.waits.len() - 1;
    assert!(
        double.waits[last].is_some(),
        "the last slate is checked beside its empty lookahead"
    );
    let (device, waits, unmeasured) = paired(&double, last);
    assert_eq!(queue.lookahead().device_ns, device);
    assert_eq!(queue.lookahead().finish_wait_ns, waits);
    queue.flushed(double.intervals[last]);
    let (device, waits, _) = paired(&double, last + 1);
    assert_eq!(queue.lookahead().device_ns, device);
    assert_eq!(queue.lookahead().finish_wait_ns, waits);
    assert_eq!(queue.lookahead().unmeasured, unmeasured);
    assert_eq!(unmeasured, 0);
}

/// An overlapped submission whose interval the timer refused is counted as
/// unmeasured, and its wait is left out, so both sums cover the same
/// submissions.
#[test]
fn a_refused_interval_is_unmeasured_and_its_wait_left_out() {
    let cancellation = Cancellation::default();
    let mut double = Double::new(usize::MAX, &cancellation);
    double.intervals = intervals(Some(1));
    let (_, mut queue) = enumerate(CHOICES, &fixed(4), UNBOUNDED, &mut double, &cancellation);
    let last = double.waits.len() - 1;
    assert!(double.waits[1].is_some());
    queue.flushed(double.intervals[last]);
    let (device, waits, unmeasured) = paired(&double, last + 1);
    assert_eq!(unmeasured, 1);
    assert_eq!(queue.lookahead().unmeasured, 1);
    assert_eq!(queue.lookahead().device_ns, device);
    assert_eq!(queue.lookahead().finish_wait_ns, waits);
}
