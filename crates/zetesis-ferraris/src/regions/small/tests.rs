//! Resource boundaries inside wide, conflict-free leaf operations.

use super::*;
use crate::test_support::theory;

fn empty_solver(atoms: usize) -> Solver {
    let theory = theory(atoms, (0..atoms).map(crate::Node::atom).collect(), vec![]);
    let mut work = Meter::local(u64::MAX);
    let shape = Shape::read(&theory, None, &mut work).unwrap().unwrap();
    Solver::encode(&shape, SmallPolicy::default(), &mut work).unwrap()
}

fn wide_watch_solver() -> Solver {
    let mut solver = empty_solver(513);
    let mut work = Meter::local(u64::MAX);
    for atom in 1..513 {
        solver
            .attach(&[lit(0, true), lit(atom, false)], false, &mut work)
            .unwrap();
    }
    solver
        .assign(lit(0, false), Reason::None, &mut work)
        .unwrap();
    solver
}

fn wide_threshold_solver() -> Solver {
    let mut solver = empty_solver(513);
    let mut work = Meter::local(u64::MAX);
    solver
        .add_pb(
            0,
            Pb {
                out: 0,
                conditions: (1..513).collect(),
                weights: vec![1; 512],
                bound: 512,
                sure: 0,
                open: 0,
                heaviest: 0,
                family: NO_FAMILY,
            },
            &mut work,
        )
        .unwrap();
    solver
        .assign(lit(0, false), Reason::None, &mut work)
        .unwrap();
    solver
}

#[test]
fn wide_watch_and_threshold_rows_refuse_inside_conflict_free_propagation() {
    for make in [wide_watch_solver as fn() -> Solver, wide_threshold_solver] {
        for maximum in [1, 2, 3, 7, 32] {
            let mut solver = make();
            let mut work = Meter::local(maximum);
            assert_eq!(
                solver.propagate(&mut work),
                Err(Failure::Stopped(Stop::WorkLimit))
            );
            assert_eq!(work.charged(), maximum);
            assert_eq!(solver.counts.conflicts, 0);
        }
        let mut solver = make();
        assert_eq!(solver.propagate(&mut Meter::local(u64::MAX)).unwrap(), None);
        assert!(solver.assigns.iter().all(|&value| value == TRUE));
    }
}

/// A caller account owns control at refills, as `Meter::reserved` specifies.
struct CancellingAccount {
    cancellation: Cancellation,
    granted: u64,
    refunded: u64,
    cancel_after: u64,
}
impl Quota for CancellingAccount {
    type Error = Stop;
    fn reserve(&mut self, _: u64) -> Result<u64, Stop> {
        if self.granted == self.cancel_after {
            self.cancellation.cancel();
        }
        self.cancellation.poll()?;
        self.granted += 1;
        Ok(1)
    }
    fn refund(&mut self, unused: u64) {
        self.refunded += unused;
    }
}

#[test]
fn wide_conflict_free_rows_reach_the_callers_cancellation_boundary() {
    for make in [wide_watch_solver as fn() -> Solver, wide_threshold_solver] {
        let mut account = CancellingAccount {
            cancellation: Cancellation::default(),
            granted: 0,
            refunded: 0,
            cancel_after: 32,
        };
        let mut solver = make();
        let mut work = Meter::reserved(&mut account);
        assert_eq!(
            solver.propagate(&mut work),
            Err(Failure::Refused(Stop::Cancelled))
        );
        assert_eq!(work.charged(), 32);
        work.refund();
        assert_eq!(account.granted - account.refunded, 32);
        assert_eq!(solver.counts.conflicts, 0);
    }
}

#[test]
fn packed_threshold_terms_are_work_even_before_a_completion_finishes() {
    let values = vec![u64::MAX; 512];
    let weights = vec![1; 512];
    let mut work = Meter::local(17);
    assert_eq!(
        threshold_lanes(&values, &weights, 512, &mut work),
        Err(Failure::Stopped(Stop::WorkLimit))
    );
    assert_eq!(work.charged(), 17);
    assert_eq!(
        threshold_lanes(&values, &weights, 512, &mut Meter::local(u64::MAX)).unwrap(),
        u64::MAX
    );
}

#[test]
fn interruption_during_epoch_wrap_cannot_reuse_old_cone_marks() {
    let owner = theory(
        2,
        vec![
            crate::Node::atom(0),
            crate::Node::atom(1),
            crate::Node::or_pair([0, 1]),
        ],
        vec![2],
    );
    let region = Region::all_open(2);
    let mut scratch = SmallScratch::default();
    scratch.set_policy(SmallPolicy::parse("enumerate=2"));
    let cancellation = Cancellation::default();
    let first = scratch.narrow(
        &owner,
        None,
        &[],
        &region,
        &cancellation,
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(first.result, Ok(SmallNarrowing::Open(Variant::Enumerated)));
    scratch.stamp.fill(1);
    scratch.epoch = u32::MAX;
    // Entry + one region word + the first cleared stamp: the next stamp
    // refuses. Without invalidation, epoch 1 would skip an uncleared root.
    let interrupted = scratch.narrow(
        &owner,
        None,
        &[],
        &region,
        &cancellation,
        &mut Meter::local(3),
    );
    assert_eq!(interrupted.result, Err(Failure::Stopped(Stop::WorkLimit)));
    assert_eq!(interrupted.charged, 3);
    let retry = scratch.narrow(
        &owner,
        None,
        &[],
        &region,
        &cancellation,
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(retry.result, first.result);
}

#[test]
fn caller_storage_refusal_cannot_become_an_unsupported_shape() {
    struct StorageAccount;
    impl Quota for StorageAccount {
        type Error = Stop;
        fn reserve(&mut self, _: u64) -> Result<u64, Stop> {
            Err(Stop::StorageLimit)
        }
        fn refund(&mut self, _: u64) {}
    }
    let owner = theory(1, vec![crate::Node::atom(0)], vec![]);
    let mut account = StorageAccount;
    let mut work = Meter::reserved(&mut account);
    assert!(matches!(
        Shape::read(&owner, None, &mut work),
        Err(Failure::Refused(Stop::StorageLimit))
    ));
    assert_eq!(work.charged(), 0);
}

#[test]
fn fallback_pick_charges_each_assignment_cell_once() {
    let mut solver = empty_solver(3);
    solver.heap.clear();
    solver.heap_index.fill(-1);
    solver.assigns[0] = TRUE;
    solver.assigns[1] = TRUE;
    // One pick dispatch, one empty-heap query, then three assignment cells.
    let mut short = Meter::local(4);
    assert_eq!(
        solver.pick(&mut short),
        Err(Failure::Stopped(Stop::WorkLimit))
    );
    assert_eq!(short.charged(), 4);
    let mut exact = Meter::local(5);
    assert_eq!(solver.pick(&mut exact), Ok(Some(2)));
    assert_eq!(exact.charged(), 5);
}

#[test]
fn enumeration_reaches_a_later_word_and_refusal_cannot_publish_refuted() {
    // (x0 OR x6) AND (NOT x0 OR x6) holds exactly when x6 does.
    // Seven open atoms give 128 completions; every lane of word 0 fails,
    // and the first satisfying completion is lane 0 of word 1 (mask 64).
    let mut nodes: Vec<_> = (0..7).map(crate::Node::atom).collect();
    nodes.push(crate::Node::falsum());
    nodes.push(crate::Node::implies(0, 7));
    nodes.push(crate::Node::or_pair([0, 6]));
    nodes.push(crate::Node::or_pair([8, 6]));
    let owner = theory(7, nodes, vec![9, 10]);
    let region = Region::all_open(7);
    let cancellation = Cancellation::default();
    let mut evaluator = crate::EvaluationWorkspace::default();
    for mask in 0_usize..128 {
        let candidate =
            crate::Interpretation::new(&owner, (0..7).filter(|atom| mask & (1 << atom) != 0))
                .unwrap();
        let evaluation = evaluator
            .evaluate(
                &candidate,
                crate::EvaluationLimits::default(),
                &cancellation,
            )
            .result
            .unwrap();
        assert_eq!(evaluation.is_model(), mask >= 64, "mask {mask}");
    }
    let fresh = || {
        let mut scratch = SmallScratch::default();
        scratch.set_policy(SmallPolicy::parse("enumerate=7"));
        scratch
    };
    // No support is supplied: every root is read explicitly for every word,
    // so this kernel fixture does not depend on a general-closure shortcut.
    let completed = fresh().narrow(
        &owner,
        None,
        &[],
        &region,
        &cancellation,
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(
        completed.result,
        Ok(SmallNarrowing::Open(Variant::Enumerated))
    );
    assert!(completed.charged > 1);
    let mut interrupted = fresh();
    let limit = completed.charged - 1;
    let refused = interrupted.narrow(
        &owner,
        None,
        &[],
        &region,
        &cancellation,
        &mut Meter::local(limit),
    );
    assert_eq!(refused.result, Err(Failure::Stopped(Stop::WorkLimit)));
    assert_eq!(refused.charged, limit);
    assert_eq!(interrupted.statistics.enumerated_refuted, 0);
    let retry = interrupted.narrow(
        &owner,
        None,
        &[],
        &region,
        &cancellation,
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(retry.result, Ok(SmallNarrowing::Open(Variant::Enumerated)));
}
