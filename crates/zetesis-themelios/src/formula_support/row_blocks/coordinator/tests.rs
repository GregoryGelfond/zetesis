use super::*;
use crate::ExpansionResource;
use crate::formula_support::testing::Fixture;
use crate::test_support::{given_site, location};
use std::num::NonZeroUsize;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use zetesis_algebra::Cancellation;

#[derive(Clone, Copy)]
enum Mode {
    Idle,
    Failure,
    Refusal,
    Grant,
    Cancellation,
    Panic,
}

struct Value {
    rows: Range<usize>,
    live: Arc<AtomicUsize>,
}
impl Drop for Value {
    fn drop(&mut self) {
        self.live.fetch_sub(1, Ordering::Relaxed);
    }
}

struct Kernel {
    mode: Mode,
    started: AtomicUsize,
    live: Arc<AtomicUsize>,
    barrier: Barrier,
    cancellation: Cancellation,
}
impl Kernel {
    fn execute(
        &self,
        rows: Range<usize>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<Value, FormulaFailure> {
        self.started.fetch_or(1 << rows.start, Ordering::Relaxed);
        if !matches!(self.mode, Mode::Idle) {
            counters.charge_work((rows.start + 1) as u128, limits, location())?;
            counters.substitution(limits, location())?;
            for resource in [
                ExpansionResource::TermWork,
                ExpansionResource::Templates,
                ExpansionResource::Values,
                ExpansionResource::ScalarBytes,
                ExpansionResource::Origins,
            ] {
                budget.charge(resource, (rows.start + 1) as u128, location())?;
            }
        }
        match self.mode {
            Mode::Failure if rows.start == 0 => {
                return Err(FormulaFailure::UnadmittedTerm {
                    location: given_site(),
                });
            }
            Mode::Failure if rows.start == 2 => {
                return Err(FormulaFailure::SourceActivity {
                    location: location(),
                });
            }
            Mode::Refusal if rows.start == 0 => {
                counters.charge_work(u128::from(limits.max_work), limits, given_site())?;
                unreachable!("requested grant plus one already-performed unit");
            }
            // More than a third of the remainder, far less than all of it.
            Mode::Grant if rows.start == 0 => {
                let share = FormulaLimits::default().max_work / 2;
                counters.charge_work(u128::from(share), limits, given_site())?;
            }
            Mode::Cancellation => {
                self.barrier.wait();
                if rows.start == 0 {
                    self.cancellation.cancel();
                }
                self.barrier.wait();
            }
            Mode::Panic => {
                self.barrier.wait();
                assert_ne!(rows.start, 1, "coordinator worker fixture");
            }
            Mode::Idle | Mode::Failure | Mode::Refusal | Mode::Grant => {}
        }
        self.live.fetch_add(1, Ordering::Relaxed);
        Ok(Value {
            rows,
            live: Arc::clone(&self.live),
        })
    }
}

struct Report {
    rows: Vec<Range<usize>>,
    failure: Option<FormulaFailure>,
    panicked: bool,
    work: u64,
    substitutions: u64,
    expansion: ExpansionUsage,
    started: usize,
    alive: usize,
    observed: crate::GroundingWork,
}

fn capture(mode: Mode, workers: usize, width: usize) -> Report {
    let kernel = Kernel {
        mode,
        started: AtomicUsize::new(0),
        live: Arc::new(AtomicUsize::new(0)),
        barrier: Barrier::new(width),
        cancellation: Cancellation::default(),
    };
    let executor = Executor::new(NonZeroUsize::new(workers).unwrap()).unwrap();
    let mut fixture = Fixture::default();
    fixture.with(location(), |_, computation, counters| {
        counters.observed = Work::worker(true);
        let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
        if matches!(mode, Mode::Cancellation) {
            counters.attach_test_control(&kernel.cancellation);
            budget.set_cancellation(Some(kernel.cancellation.clone()));
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            run(
                &executor,
                width,
                &mut budget,
                Context::new(
                    &*computation,
                    &FormulaLimits::default(),
                    counters,
                    location(),
                ),
                |rows, limits, budget, counters| kernel.execute(rows, limits, budget, counters),
            )
        }));
        let (rows, failure, panicked) = match result {
            Ok(Ok(completed)) => (
                completed
                    .results()
                    .map(|value| value.rows.clone())
                    .collect(),
                None,
                false,
            ),
            Ok(Err(error)) => (Vec::new(), Some(error), false),
            Err(_) => (Vec::new(), None, true),
        };
        Report {
            rows,
            failure,
            panicked,
            work: counters.accounting.work,
            substitutions: counters.accounting.substitutions,
            expansion: budget.usage(),
            started: kernel.started.load(Ordering::Relaxed),
            alive: kernel.live.load(Ordering::Relaxed),
            observed: counters.observed.snapshot(),
        }
    })
}

#[test]
fn fixed_blocks_cover_the_original_stream_once_in_order() {
    for width in [0, 1, 3, 13, 14, 15, 29] {
        let single = capture(Mode::Idle, 1, width);
        assert!(single.failure.is_none());
        assert!(!single.panicked);
        assert_eq!(
            single.rows.iter().cloned().flatten().collect::<Vec<_>>(),
            (0..width).collect::<Vec<_>>()
        );
        assert_eq!(single.rows.len(), width.min(TASKS));
        assert!(single.rows.iter().all(|rows| !rows.is_empty()));
        assert_eq!(single.alive, 0);
        for workers in [4, 14] {
            let parallel = capture(Mode::Idle, workers, width);
            assert!(parallel.failure.is_none());
            assert!(!parallel.panicked);
            assert_eq!(parallel.rows, single.rows);
            assert_eq!(parallel.work, single.work);
            assert_eq!(parallel.substitutions, single.substitutions);
            assert_eq!(parallel.expansion, single.expansion);
            assert_eq!(parallel.alive, 0);
        }
    }
}

fn performed(report: &Report, baseline: &Report) {
    assert_eq!(report.started, 0b111);
    assert_eq!(report.work, baseline.work + 6);
    assert_eq!(report.substitutions, baseline.substitutions + 3);
    assert_eq!(
        report.expansion,
        ExpansionUsage {
            term_work: 6,
            templates: 6,
            values: 6,
            scalar_bytes: 6,
            origin_locations: 6,
        }
    );
    assert_eq!(report.alive, 0);
    assert!(
        report.rows.is_empty(),
        "an unsuccessful batch published a result"
    );
    assert_eq!(report.observed.support_peak_bytes, None);
}

#[test]
fn the_first_logical_failure_retains_all_completed_receipts() {
    for workers in [1, 3, 14] {
        let baseline = capture(Mode::Idle, workers, 3);
        let report = capture(Mode::Failure, workers, 3);
        performed(&report, &baseline);
        assert!(!report.panicked);
        assert!(matches!(report.failure,
            Some(FormulaFailure::UnadmittedTerm { location: site }) if site == given_site()));
    }
}

#[test]
fn worker_refusal_retains_actual_work_without_spending_its_grant() {
    let baseline = capture(Mode::Idle, 3, 3);
    let report = capture(Mode::Refusal, 3, 3);
    assert!(!report.panicked);
    // Every attempt's actual work settles; the block its grant refused is then
    // repeated once on the source's own accounts, which report the ceiling.
    assert_eq!(report.started, 0b111);
    assert_eq!(report.work, baseline.work + 6 + 1);
    assert_eq!(report.substitutions, baseline.substitutions + 3 + 1);
    assert_eq!(report.expansion.term_work, 6 + 1);
    assert_eq!(report.alive, 0);
    assert!(report.rows.is_empty());
    let ceiling = u128::from(FormulaLimits::default().max_work);
    assert!(matches!(report.failure, Some(FormulaFailure::Limit {
        resource: FormulaResource::Work, limit, observed, location: site,
    }) if limit == ceiling
        && observed == u128::from(report.work) + ceiling
        && site == given_site()));
}

#[test]
fn a_grant_refusal_within_the_ceiling_completes_on_any_pool() {
    for workers in [1, 3, 14] {
        let single = capture(Mode::Idle, 1, 3);
        let report = capture(Mode::Grant, workers, 3);
        assert!(report.failure.is_none(), "{workers}: {:?}", report.failure);
        assert_eq!(report.rows, single.rows);
        assert_eq!(report.alive, 0);
    }
}

#[test]
fn cancellation_settles_every_performed_dimension_before_return() {
    let baseline = capture(Mode::Idle, 3, 3);
    let report = capture(Mode::Cancellation, 3, 3);
    performed(&report, &baseline);
    assert!(!report.panicked);
    assert_eq!(
        report
            .failure
            .as_ref()
            .and_then(FormulaFailure::interruption),
        Some(zetesis_algebra::Stop::Cancelled),
    );
}

#[test]
fn a_worker_panic_settles_receipts_and_releases_all_outputs() {
    let baseline = capture(Mode::Idle, 3, 3);
    let report = capture(Mode::Panic, 3, 3);
    performed(&report, &baseline);
    assert!(report.panicked);
    assert!(report.failure.is_none());
}

#[test]
fn descriptor_storage_refusal_starts_no_worker() {
    let mut fixture = Fixture::default();
    let executor = Executor::new(NonZeroUsize::new(1).unwrap()).unwrap();
    let started = AtomicUsize::new(0);
    fixture.with(location(), |_, computation, counters| {
        let limits = FormulaLimits {
            max_support_bytes: 0,
            ..FormulaLimits::default()
        };
        let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
        let result = run(
            &executor,
            3,
            &mut budget,
            Context::new(&*computation, &limits, counters, location()),
            |_, _, _, _| {
                started.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        );
        assert!(matches!(
            result,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                limit: 0,
                ..
            })
        ));
        assert_eq!(started.load(Ordering::Relaxed), 0);
        assert_eq!(budget.usage(), ExpansionUsage::default());
    });
}

#[test]
fn worker_grants_partition_remaining_accounts_and_retained_bytes() {
    let mut fixture = Fixture::default();
    let executor = Executor::new(NonZeroUsize::new(3).unwrap()).unwrap();
    fixture.with(location(), |support, computation, counters| {
        let limits = FormulaLimits::default();
        let expansion = ExpansionLimits::default();
        let mut budget = Budget::new(expansion, usize::MAX);
        budget
            .charge(ExpansionResource::TermWork, 19, location())
            .unwrap();
        budget
            .charge(ExpansionResource::ScalarBytes, 23, location())
            .unwrap();
        counters.substitution(&limits, location()).unwrap();
        let completed = run(
            &executor,
            29,
            &mut budget,
            Context::new(&*computation, &limits, counters, location()),
            |_, limits, budget, _| Ok((*limits, budget.worker_limits(1, 0))),
        )
        .unwrap();
        assert_eq!(
            completed
                .results()
                .map(|(grant, _)| grant.max_work)
                .sum::<u64>()
                + counters.accounting.work,
            limits.max_work
        );
        assert_eq!(
            completed
                .results()
                .map(|(grant, _)| grant.max_substitutions)
                .sum::<u64>()
                + counters.accounting.substitutions,
            limits.max_substitutions
        );
        assert_eq!(
            completed
                .results()
                .map(|(_, grant)| grant.max_term_work)
                .sum::<usize>()
                + budget.usage().term_work,
            expansion.max_term_work
        );
        assert_eq!(
            completed
                .results()
                .map(|(_, grant)| grant.max_scalar_bytes)
                .sum::<usize>()
                + budget.usage().scalar_bytes,
            expansion.max_scalar_bytes
        );
        assert_eq!(
            completed
                .results()
                .map(|(grant, _)| grant.max_support_bytes - support.relations().current_bytes())
                .sum::<usize>(),
            completed.reserved
        );
        assert_eq!(
            completed.publication_limits(&limits).max_support_bytes + completed.reserved,
            limits.max_support_bytes
        );
        assert_eq!(completed.results().count(), TASKS);
    });
}
