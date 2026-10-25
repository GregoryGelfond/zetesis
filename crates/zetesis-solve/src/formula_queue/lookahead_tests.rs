//! A GPU enumeration's two-phase slates against a device double: the next slate
//! is proposed while the current slate's last submission is checked, with the
//! same family, slate sizes and failure semantics as the serial schedule.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use zetesis_cpu::Cancellation;
use zetesis_ferraris::{SlateId, SlateRef, Theory};
use zetesis_sat::{BatchVerdict, Limits, StableModels};

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

/// A device that decides each candidate exactly, admits at most `prefix`
/// candidates per submission, and records what the queue asked of it.
struct Double {
    prefix: usize,
    stop: Stop,
    control: Cancellation,
    in_flight: usize,
    peak_in_flight: usize,
    overlapped: usize,
    /// Each slate's candidate count, at its first submission.
    slates: Vec<usize>,
    /// Whether every submission's completion was observed.
    reusable: bool,
}

impl Double {
    fn new(prefix: usize, control: &Cancellation) -> Self {
        Self {
            prefix,
            stop: Stop::Never,
            control: control.clone(),
            in_flight: 0,
            peak_in_flight: 0,
            overlapped: 0,
            slates: Vec::new(),
            reusable: true,
        }
    }
}

struct Receipt {
    slate: SlateId,
    offset: usize,
    verdicts: Vec<BatchVerdict>,
}

struct Prepared<'d> {
    double: &'d mut Double,
    receipt: Receipt,
}

impl Prepared<'_> {
    fn submit(&mut self) {
        self.double.in_flight += 1;
        self.double.peak_in_flight = self.double.peak_in_flight.max(self.double.in_flight);
    }

    fn finish(self, stopped: bool) -> Result<Receipt, Failure> {
        self.double.in_flight -= 1;
        if stopped {
            return Err(Failure::Search(zetesis_sat::Incomplete::Cancelled));
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
        cancellation: &Cancellation,
    ) -> Result<Prepared<'d>, Failure> {
        cancellation
            .poll()
            .map_err(|stop| Failure::Search(stop.into()))?;
        assert_eq!(self.in_flight, 0, "a submission is prepared beside another");
        if offset == 0 {
            self.slates.push(slate.candidates().len());
        }
        let rest = &slate.candidates()[offset..];
        let admitted = &rest[..rest.len().min(self.prefix)];
        let verdicts = admitted
            .iter()
            .map(|candidate| {
                let check = zetesis_ferraris::check(
                    slate.theory(),
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
            })
            .collect();
        Ok(Prepared {
            double: self,
            receipt: Receipt {
                slate: slate.id(),
                offset,
                verdicts,
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
        Ok(Decoded {
            verdicts: receipt.verdicts,
            compute: None,
        })
    }
}

impl PreparedSubmission for Prepared<'_> {
    type Checked = Result<Receipt, Failure>;

    fn count(&self) -> usize {
        self.receipt.verdicts.len()
    }

    fn check(mut self, cancellation: &Cancellation) -> Self::Checked {
        self.submit();
        let stopped = cancellation.poll().is_err();
        self.finish(stopped)
    }

    fn check_overlapping<R>(
        mut self,
        cancellation: &Cancellation,
        during: impl FnOnce() -> R,
    ) -> (Self::Checked, Option<R>) {
        self.submit();
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
