#![allow(
    clippy::disallowed_methods,
    reason = "tests start their own threads and pools"
)]
use super::*;
use crate::formula_support::Accounting;
use crate::grounding_observer::Work;
use crate::test_support::location;
use crate::{FormulaFailure, FormulaLimits, FormulaResource};
use rayon::ThreadPoolBuilder;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Condvar, Mutex};
use zetesis_algebra::Cancellation;

fn workers(count: usize) -> NonZeroUsize {
    NonZeroUsize::new(count).unwrap()
}

fn assert_complete_grounding(source: &str, staged: bool) {
    use crate::formula_support::testing;
    use crate::test_support::answer_sets;

    let serial = crate::formula_ground::ground(testing::prepare(source), None, None).unwrap();
    let expected = answer_sets(&serial.theory, &serial.atoms);
    let mut task_count = None;
    for count in [1, 4, 14] {
        let (parallel, tasks) = with_source_workers(workers(count), || {
            crate::formula_ground::ground(testing::prepare(source), None, None)
        });
        let parallel = parallel.unwrap();
        if staged {
            assert!(tasks > 0, "ordinary source bypassed workers: {source}");
        } else {
            assert_eq!(
                tasks, 0,
                "single-generator source left its serial consumer: {source}"
            );
        }
        if let Some(prior) = task_count {
            assert_eq!(
                tasks, prior,
                "worker count changed the logical task geometry"
            );
        }
        task_count = Some(tasks);
        assert_eq!(parallel.atoms.atoms(), serial.atoms.atoms());
        assert_eq!(parallel.theory.nodes(), serial.theory.nodes());
        assert_eq!(parallel.theory.operands(), serial.theory.operands());
        assert_eq!(parallel.theory.roots(), serial.theory.roots());
        assert_eq!(parallel.origins, serial.origins);
        assert_eq!(answer_sets(&parallel.theory, &parallel.atoms), expected);
    }
}

#[test]
fn ordinary_grounding_preserves_the_complete_reduct_family() {
    for source in [
        "p(1;2). q(1;2). r(X,Y) :- p(X), q(Y).",
        "p(1;2). q(1;2). r(X) :- p(X), q(Y).",
        "p(1). q(1). r(X,Y) :- p(X), q(Y). s(X,Y) :- r(X,Y), p(X).",
    ] {
        assert_complete_grounding(source, true);
    }
}

#[test]
fn single_generator_grounding_retains_its_serial_complete_family() {
    assert_complete_grounding("p(1). q(1). r(X) :- p(X), q(X). s(X) :- r(X), p(X).", false);
}

#[test]
fn physical_worker_count_preserves_complete_source_receipts() {
    use crate::formula_support::testing;
    let source = "p(1..8). q(1..5). r(X,Y) :- p(X), q(Y).";
    let capture = |count| {
        with_source_workers(workers(count), || {
            crate::formula_ground::ground_retained(testing::prepare(source), None, false).unwrap()
        })
    };
    let (single, single_tasks) = capture(1);
    let (parallel, parallel_tasks) = capture(14);
    assert!(single_tasks > 0);
    assert_eq!(single_tasks, parallel_tasks);
    assert_eq!(single.accounting.work, parallel.accounting.work);
    assert_eq!(
        single.accounting.substitutions,
        parallel.accounting.substitutions
    );
    assert_eq!(single.compiled.expansion, parallel.compiled.expansion);
    assert_eq!(
        single.compiled.atoms.atoms(),
        parallel.compiled.atoms.atoms()
    );
    assert_eq!(
        single.compiled.theory.nodes(),
        parallel.compiled.theory.nodes()
    );
}

#[test]
fn parallel_support_retains_join_and_head_work_attribution() {
    #[derive(Default)]
    struct Observer(Cell<crate::GroundingWork>);
    impl crate::GroundingObserver for Observer {
        fn enter(&self) {}
        fn exit(&self) {}
        fn details_enabled(&self) -> bool {
            true
        }
        fn phase_exit(
            &self,
            phase: crate::GroundingPhase,
            _: Option<crate::ProgramSite>,
            _: crate::GroundingOutcome,
            work: crate::GroundingWork,
        ) {
            if phase == crate::GroundingPhase::SupportCompletion {
                self.0.set(self.0.get().checked_sum(work));
            }
        }
    }
    let observer = Observer::default();
    let (result, tasks) = with_source_workers(workers(14), || {
        crate::formula_ground::ground(
            crate::formula_support::testing::prepare("p(1;2). q(1;2). r(X,Y) :- p(X), q(Y)."),
            Some(&observer),
            None,
        )
    });
    result.unwrap();
    assert!(tasks > 0);
    let work = observer.0.get();
    assert!(work.support_join_work.unwrap() > 0);
    assert!(work.support_head_work.unwrap() > 0);
    assert!(
        work.support_production_work.unwrap()
            >= work.support_join_work.unwrap() + work.support_head_work.unwrap()
    );
    assert_eq!(work.support_peak_bytes, None);
}

#[test]
fn in_place_scope_retains_non_send_state_on_the_coordinator() {
    let executor = Executor::new(workers(1)).unwrap();
    let caller = std::thread::current().id();
    let observed = Rc::new(Cell::new(None));
    let mut worker = None;
    executor.in_place(|scope| {
        observed.set(Some(std::thread::current().id()));
        scope.spawn(|_| worker = Some(std::thread::current().id()));
    });
    assert_eq!(observed.get(), Some(caller));
    assert_ne!(worker.unwrap(), caller);
}

#[test]
fn one_pool_is_reused_across_independent_batches() {
    let factories = Cell::new(0);
    let executor = Executor::with_factory(workers(1), |count| {
        factories.set(factories.get() + 1);
        ThreadPoolBuilder::new().num_threads(count.get()).build()
    })
    .unwrap();
    let mut first = [None, None];
    let mut second = [None, None];
    executor.run(&[(), ()], &mut first, |()| std::thread::current().id());
    executor.run(&[(), ()], &mut second, |()| std::thread::current().id());
    assert_eq!(factories.get(), 1);
    assert_eq!(first, second);
    // The coordinator runs the first task; the pool's one worker the other.
    assert_eq!(first[0].unwrap(), std::thread::current().id());
    assert_ne!(first[1].unwrap(), std::thread::current().id());
}

#[test]
fn a_single_task_runs_on_the_coordinator() {
    let executor = Executor::new(workers(4)).unwrap();
    let mut slot = [None];
    executor.run(&[()], &mut slot, |()| std::thread::current().id());
    assert_eq!(slot[0], Some(std::thread::current().id()));
}

#[test]
fn slots_preserve_logical_order_when_tasks_finish_in_reverse() {
    let executor = Executor::new(workers(3)).unwrap();
    let state = (Mutex::new((2_usize, Vec::new())), Condvar::new());
    let mut slots: [Option<usize>; 3] = [None; 3];
    executor.run(&[0, 1, 2], &mut slots, |&index| {
        let (lock, ready) = &state;
        let mut state = lock.lock().unwrap();
        while state.0 != index {
            state = ready.wait(state).unwrap();
        }
        state.1.push(index);
        state.0 = state.0.saturating_sub(1);
        ready.notify_all();
        index * 10
    });
    assert_eq!(state.0.lock().unwrap().1, [2, 1, 0]);
    assert_eq!(slots, [Some(0), Some(10), Some(20)]);
}

#[derive(Clone, Copy)]
enum Task {
    Success,
    Refusal,
    Panic,
    Cancellation,
}

type Attempt = Capture<usize, FormulaFailure, u64>;

fn attempt(task: Task) -> Attempt {
    let mut account = Accounting::default();
    let cancellation = Cancellation::default();
    let limits = FormulaLimits {
        max_work: 3,
        ..FormulaLimits::default()
    };
    Capture::run(
        &mut account,
        |account| {
            account.controlled(
                Some(&cancellation),
                Work::default(),
                location(),
                |local| match task {
                    Task::Success => {
                        local.charge_work(1, &limits, location())?;
                        Ok(7)
                    }
                    Task::Refusal => {
                        local.charge_work(2, &limits, location())?;
                        local.charge_work(2, &limits, location())?;
                        unreachable!("the exact ceiling refuses the second charge")
                    }
                    Task::Panic => {
                        local.charge_work(3, &limits, location())?;
                        panic!("worker failed after three units");
                    }
                    Task::Cancellation => {
                        local.charge_work(2, &limits, location())?;
                        cancellation.cancel();
                        Ok(11)
                    }
                },
            )
        },
        |account| account.work,
    )
}

#[test]
fn successful_refused_cancelled_and_panicking_work_all_settle_before_outcomes() {
    let executor = Executor::new(workers(3)).unwrap();
    let tasks = [
        Task::Success,
        Task::Refusal,
        Task::Panic,
        Task::Cancellation,
    ];
    let mut slots: [Option<Attempt>; 4] = std::array::from_fn(|_| None);
    executor.run(&tasks, &mut slots, |&task| attempt(task));
    let receipts = Rc::new(RefCell::new(Vec::new()));
    settle(&mut slots, |work| {
        receipts.borrow_mut().push(work);
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(*receipts.borrow(), [1, 2, 3, 2]);
    let [success, refusal, panic, cancelled] = slots.map(|slot| slot.unwrap().into_outcome());
    assert!(matches!(success, Outcome::Returned(Ok(7))));
    assert!(matches!(
        refusal,
        Outcome::Returned(Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            observed: 4,
            limit: 3,
            ..
        }))
    ));
    let Outcome::Panicked(payload) = panic else {
        panic!("worker panic must remain a panic after settlement");
    };
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"worker failed after three units")
    );
    assert!(matches!(
        cancelled,
        Outcome::Returned(Err(FormulaFailure::Interrupted { .. }))
    ));
}

#[test]
fn settlement_failure_does_not_discard_later_completed_receipts() {
    let executor = Executor::new(workers(1)).unwrap();
    let mut slots: [Option<Attempt>; 3] = std::array::from_fn(|_| None);
    executor.run(&[Task::Success; 3], &mut slots, |&task| attempt(task));
    let mut observed = Vec::new();
    let result = settle(&mut slots, |work| {
        let index = observed.len();
        observed.push(work);
        match index {
            0 => Err("first"),
            2 => Err("later"),
            _ => Ok(()),
        }
    });
    assert_eq!(result, Err("first"));
    assert_eq!(observed, [1, 1, 1]);
    for slot in slots {
        assert!(matches!(
            slot.unwrap().into_outcome(),
            Outcome::Returned(Ok(7))
        ));
    }
}

#[test]
fn failed_pool_factory_never_claims_a_cursor_or_submits_work() {
    let factories = Cell::new(0);
    let cursor_claims = Cell::new(0);
    let pool = Executor::with_factory(workers(14), |count| {
        factories.set(factories.get() + 1);
        ThreadPoolBuilder::new()
            .num_threads(count.get())
            .spawn_handler(|_| Err(std::io::Error::other("injected thread creation refusal")))
            .build()
    });
    match pool {
        Ok(executor) => {
            cursor_claims.set(cursor_claims.get() + 1);
            let mut slots = [None];
            executor.run(&[()], &mut slots, |()| ());
            panic!("the injected factory must fail before a job exists");
        }
        Err(error) => assert!(
            error
                .to_string()
                .contains("injected thread creation refusal")
        ),
    }
    assert_eq!(factories.get(), 1);
    assert_eq!(cursor_claims.get(), 0);
}

#[test]
fn empty_batches_create_no_worker_operation() {
    let executor = Executor::new(workers(1)).unwrap();
    let mut slots: [Option<()>; 0] = [];
    executor.run(&[] as &[()], &mut slots, |()| {
        panic!("empty batch ran a worker")
    });
    assert!(slots.is_empty());
}

#[test]
fn the_pool_with_its_caller_stays_within_the_requested_threads() {
    // --threads N caps grounding at N threads: N - 1 pool threads plus the
    // coordinating caller, which runs tasks too (it was a fixed 14 before).
    assert_eq!(pool_threads(Some(workers(1))), None);
    for count in [2, 3, 14] {
        let pool = pool_threads(Some(workers(count))).unwrap();
        assert_eq!(pool.get(), count - 1);
        let executor = Executor::new(pool).unwrap();
        assert_eq!(
            executor.pool.as_ref().unwrap().current_num_threads(),
            count - 1
        );
    }
    let available = std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    assert_eq!(
        pool_threads(None).map_or(1, |pool| pool.get() + 1),
        available.get()
    );
}
