//! PR35 steering publication and compact-encoding resource boundaries.
use super::*;
use crate::test_support::{constructed_theory, push, theory};

#[test]
fn publication_admits_every_word_before_changing_the_shared_generation() {
    let shared = SharedCompletion::default();
    let old = vec![7; 20];
    assert_eq!(shared.publish(&old, &mut Meter::local(21)).unwrap(), 1);
    let next = vec![11; 20];
    for maximum in [0, 1, 4, 20] {
        let mut work = Meter::local(maximum);
        assert_eq!(
            shared.publish(&next, &mut work),
            Err(Failure::Stopped(Stop::WorkLimit))
        );
        assert_eq!(work.charged(), maximum);
        assert_eq!(
            shared.generation.load(std::sync::atomic::Ordering::Acquire),
            1
        );
        assert_eq!(*shared.model.lock().unwrap(), old);
    }
    let mut exact = Meter::local(21); // one publication header and twenty words
    assert_eq!(shared.publish(&next, &mut exact).unwrap(), 2);
    assert_eq!(exact.charged(), 21);
    assert_eq!(*shared.model.lock().unwrap(), next);
}

struct ControlAccount {
    cancellation: Cancellation,
    used: u64,
    cancel_after: u64,
    refunded: u64,
}
impl Quota for ControlAccount {
    type Error = Stop;
    fn reserve(&mut self, _: u64) -> Result<u64, Stop> {
        if self.used == self.cancel_after {
            self.cancellation.cancel();
        }
        self.cancellation.poll()?;
        self.used += 1;
        Ok(1)
    }
    fn refund(&mut self, unused: u64) {
        self.refunded += unused;
    }
}

fn guided(shared: &std::sync::Arc<SharedCompletion>) -> SmallScratch {
    let mut scratch = SmallScratch::default();
    scratch.set_policy(SmallPolicy::parse("learn=inf,steer=1"));
    scratch.share_completions(std::sync::Arc::clone(shared));
    scratch
}

#[test]
fn interrupted_adoption_returns_the_prefix_and_does_not_consume_the_generation() {
    let shared = std::sync::Arc::new(SharedCompletion::default());
    shared
        .publish(&vec![u64::MAX; 130], &mut Meter::local(u64::MAX))
        .unwrap();
    let region = Region::all_open(130 * 64);
    let cancellation = Cancellation::default();
    let mut scratch = guided(&shared);
    let refused = scratch.attempt(&cancellation, &mut Meter::local(7), |s, w| {
        s.guide(&region, w)
    });
    assert_eq!(refused.result, Err(Failure::Stopped(Stop::WorkLimit)));
    assert_eq!(refused.charged, 7);
    assert_eq!(scratch.adopted, 0);
    assert!(scratch.models.is_empty() && scratch.following.is_none());
    let retry = scratch.attempt(&cancellation, &mut Meter::local(u64::MAX), |s, w| {
        s.guide(&region, w)
    });
    assert_eq!(retry.result, Ok(Some(region.len() - 1)));
    assert_eq!(scratch.adopted, 1);
    assert_eq!(scratch.models.len(), 1);
    assert!(scratch.following.is_some());

    let cancellation = Cancellation::default();
    let mut account = ControlAccount {
        cancellation: cancellation.clone(),
        used: 0,
        cancel_after: 32,
        refunded: 0,
    };
    let mut scratch = guided(&shared);
    let cancelled = scratch.attempt(&cancellation, &mut Meter::reserved(&mut account), |s, w| {
        s.guide(&region, w)
    });
    assert_eq!(cancelled.result, Err(Failure::Refused(Stop::Cancelled)));
    assert_eq!(cancelled.charged, 32);
    assert_eq!(account.used - account.refunded, 32);
    assert_eq!(scratch.adopted, 0);
    assert!(scratch.models.is_empty() && scratch.following.is_none());
}

#[test]
fn observation_and_refutation_notice_do_not_turn_a_refusal_into_deferral() {
    let region = Region::all_open(192);
    let cancellation = Cancellation::default();
    let mut scratch = SmallScratch::default();
    scratch.following = Some(vec![u64::MAX; 3]);
    let refused = scratch.attempt(&cancellation, &mut Meter::local(2), |s, w| {
        s.defers(&region, w)
    });
    assert_eq!(refused.result, Err(Failure::Stopped(Stop::WorkLimit)));
    assert_eq!(refused.charged, 2);
    assert!(scratch.following.is_none());
    assert_eq!(scratch.statistics.deferrals, 0);
    assert_eq!(scratch.statistics.steer_failures, 0);

    scratch.set_policy(SmallPolicy::parse("adapt=1"));
    scratch.following = Some(vec![u64::MAX; 3]);
    let refused = scratch.attempt(&cancellation, &mut Meter::local(2), |s, w| {
        s.note_refuted(&region, w)
    });
    assert_eq!(refused.result, Err(Failure::Stopped(Stop::WorkLimit)));
    assert_eq!(refused.charged, 2); // observed refutation + first adaptive word
    assert_eq!(scratch.statistics.steer_failures, 1);
    assert!(scratch.following.is_none());
}

fn compact_subject() -> Theory {
    constructed_theory(3, |transaction| {
        let falsehood = push(transaction, NodeView::False);
        let negative = push(transaction, NodeView::Implies(0, falsehood));
        let threshold = push(
            transaction,
            NodeView::Threshold(crate::Threshold {
                conditions: &[negative, negative, 1],
                weights: &[1, 2, 1],
                bound: 3,
            }),
        );
        let premise = push(transaction, NodeView::And(&[1, 2]));
        vec![push(transaction, NodeView::Implies(premise, threshold))]
    })
}

#[test]
fn compact_encoding_reads_real_nodes_and_preserves_every_atom_completion() {
    let owner = compact_subject();
    let shape = Shape::read(&owner, None, &mut Meter::local(u64::MAX))
        .unwrap()
        .unwrap();
    let cancellation = Cancellation::default();
    for compact in [false, true] {
        let policy = SmallPolicy {
            refinements: Refinements::MEASURED.with(Refinements::COMPACT, compact),
            ..SmallPolicy::default()
        };
        let mut work = Meter::local(u64::MAX);
        let mut solver = Solver::encode(&shape, policy, &mut work).unwrap();
        let exact = work.charged();
        assert!(exact > 1);
        let mut limited = Meter::local(exact - 1);
        assert!(matches!(
            Solver::encode(&shape, policy, &mut limited),
            Err(Failure::Stopped(Stop::WorkLimit))
        ));
        assert_eq!(limited.charged(), exact - 1);
        for mask in 0..8 {
            let assumed: Vec<_> = (0..3)
                .map(|atom| lit(atom, mask & (1 << atom) == 0))
                .collect();
            let outcome = solver
                .solve(
                    &assumed,
                    u64::MAX,
                    &cancellation,
                    &mut Meter::local(u64::MAX),
                    None,
                )
                .unwrap();
            // The weighted negative row is exactly !x0. Its implication from
            // x1 AND x2 therefore excludes exactly the all-held assignment.
            assert_eq!(matches!(outcome, Outcome::Satisfiable), mask != 7);
        }
    }
}

#[test]
fn root_probe_refusal_during_model_extraction_cannot_publish_a_prefix() {
    let owner = theory(
        512,
        (0..512).map(crate::Node::atom).collect(),
        (0..512).collect(),
    );
    let cancellation = Cancellation::default();
    let prepare = |shared: &std::sync::Arc<SharedCompletion>| {
        let mut scratch = guided(shared);
        scratch
            .shape(&owner, None, &mut Meter::local(u64::MAX))
            .unwrap();
        scratch.guard(&[], &mut Meter::local(u64::MAX)).unwrap();
        scratch
    };
    let shared = std::sync::Arc::new(SharedCompletion::default());
    let mut reference = prepare(&shared);
    let mut search = Meter::local(u64::MAX);
    assert!(matches!(
        reference
            .solver
            .as_mut()
            .unwrap()
            .solve(&[], u64::MAX, &cancellation, &mut search, None)
            .unwrap(),
        Outcome::Satisfiable
    ));
    // The same prepared search is followed by eight initialized model words,
    // then one charged visit per atom; stop strictly inside that extraction.
    let limit = search.charged() + 8 + 17;
    let mut scratch = prepare(&shared);
    let refused = scratch.attempt(&cancellation, &mut Meter::local(limit), |s, w| {
        s.probe_root(u64::MAX, &cancellation, w)
    });
    assert_eq!(refused.result, Err(Failure::Stopped(Stop::WorkLimit)));
    assert_eq!(refused.charged, limit);
    assert_eq!(
        shared.generation.load(std::sync::atomic::Ordering::Acquire),
        0
    );
    assert!(shared.model.lock().unwrap().is_empty());
    assert!(scratch.solver.is_none() && scratch.models.is_empty());
    let retry = scratch.attempt(&cancellation, &mut Meter::local(u64::MAX), |s, w| {
        s.guard(&[], w)?;
        s.probe_root(u64::MAX, &cancellation, w)
    });
    assert_eq!(retry.result, Ok(()));
    assert_eq!(
        shared.generation.load(std::sync::atomic::Ordering::Acquire),
        1
    );
    assert_eq!(*shared.model.lock().unwrap(), vec![u64::MAX; 8]);
}

#[test]
fn compact_and_full_encodings_keep_original_support_and_restriction_guards() {
    let owner = theory(
        2,
        vec![
            crate::Node::atom(0),
            crate::Node::atom(1),
            crate::Node::or_pair([0, 1]),
        ],
        vec![2],
    );
    let restriction = theory(
        2,
        vec![
            crate::Node::atom(1),
            crate::Node::falsum(),
            crate::Node::implies(0, 1),
        ],
        vec![2],
    );
    let cancellation = Cancellation::default();
    let extracted =
        crate::producers(&owner, crate::RegionLimits::default(), &cancellation).unwrap();
    assert!(extracted.producers.is_some());
    let shape = Shape::read(
        &owner,
        extracted.producers.as_ref(),
        &mut Meter::local(u64::MAX),
    )
    .unwrap()
    .unwrap();
    for compact in [false, true] {
        let policy = SmallPolicy {
            refinements: Refinements::MEASURED.with(Refinements::COMPACT, compact),
            ..SmallPolicy::default()
        };
        let mut solver = Solver::encode(&shape, policy, &mut Meter::local(u64::MAX)).unwrap();
        let guard = solver
            .guard_of(&restriction, &mut Meter::local(u64::MAX))
            .unwrap();
        for restricted in [false, true] {
            for mask in 0..4 {
                let mut assumed: Vec<_> = (0..2)
                    .map(|atom| lit(atom, mask & (1 << atom) == 0))
                    .collect();
                assumed.push(lit(guard, !restricted));
                let outcome = solver
                    .solve(
                        &assumed,
                        u64::MAX,
                        &cancellation,
                        &mut Meter::local(u64::MAX),
                        None,
                    )
                    .unwrap();
                // Ordinary disjunctive support allows exactly one head. The
                // active restriction removes b; its inactive guard does not.
                let expected = mask == 1 || (mask == 2 && !restricted);
                assert_eq!(matches!(outcome, Outcome::Satisfiable), expected);
            }
        }
    }
}

// Append inside small/metering_tests.rs, which already imports super::*.

fn reason_chain_solver() -> Solver {
    let mut work = Meter::local(u64::MAX);
    let policy = SmallPolicy {
        refinements: Refinements::MEASURED,
        ..SmallPolicy::default()
    };
    let mut solver = Solver::new(9, policy, &mut work).unwrap();
    // x0 is an assumption; each following variable has the actual binary
    // implication !x(i-1) | xi as its reason.
    solver.trail_lim.push(0);
    solver
        .assign(lit(0, false), Reason::None, &mut work)
        .unwrap();
    for atom in 1..9 {
        solver
            .attach(&[lit(atom - 1, true), lit(atom, false)], false, &mut work)
            .unwrap();
        solver
            .assign(
                lit(atom, false),
                Reason::Binary(lit(atom - 1, true)),
                &mut work,
            )
            .unwrap();
    }
    // Those are the two literals of the proposed learned clause being tested.
    solver.seen.set(0, true);
    solver.seen.set(8, true);
    solver
}

#[test]
fn minimization_reason_traversal_refuses_exactly_and_retries_from_fresh_state() {
    // Independent semantic check: the chain and x0 entail x8.
    for assignment in 0_u32..512 {
        let chain =
            (1..9).all(|atom| assignment & (1 << (atom - 1)) == 0 || assignment & (1 << atom) != 0);
        if chain && assignment & 1 != 0 {
            assert_ne!(assignment & (1 << 8), 0);
        }
    }
    let mut reference = reason_chain_solver();
    let mut work = Meter::local(u64::MAX);
    assert!(
        reference
            .redundant(8, 1 << 1, &mut vec![0, 8], &mut work)
            .unwrap()
    );
    let full = work.charged();
    assert!(
        full > 21,
        "the test must traverse several nontrivial reasons"
    );
    let cancellation = Cancellation::default();
    for maximum in [0, 1, 3, 7, 13, 21, full - 1] {
        let mut scratch = SmallScratch::default();
        scratch.solver = Some(reason_chain_solver());
        let refused = scratch.attempt(&cancellation, &mut Meter::local(maximum), |s, w| {
            s.solver
                .as_mut()
                .unwrap()
                .redundant(8, 1 << 1, &mut vec![0, 8], w)
        });
        assert_eq!(refused.result, Err(Failure::Stopped(Stop::WorkLimit)));
        assert_eq!(refused.charged, maximum);
        assert!(
            scratch.solver.is_none(),
            "partial seen marks must not survive"
        );
        scratch.solver = Some(reason_chain_solver());
        let retry = scratch.attempt(&cancellation, &mut Meter::local(u64::MAX), |s, w| {
            s.solver
                .as_mut()
                .unwrap()
                .redundant(8, 1 << 1, &mut vec![0, 8], w)
        });
        assert_eq!(retry.result, Ok(true));
        assert_eq!(retry.charged, full);
    }
    // A decision interrupts the implication explanation. It cannot be removed
    // merely because it happens to hold in the current completion.
    let mut interrupted = reason_chain_solver();
    interrupted.reason[4] = PackedReason::NONE;
    let mut clear = vec![0, 8];
    assert!(
        !interrupted
            .redundant(8, 1 << 1, &mut clear, &mut Meter::local(u64::MAX))
            .unwrap()
    );
    assert_eq!(clear, vec![0, 8]);
    assert!((1..8).all(|atom| !interrupted.seen.get(atom)));
}

fn glue_remap_solver() -> Solver {
    let mut work = Meter::local(u64::MAX);
    let mut solver = Solver::new(6, SmallPolicy::default(), &mut work).unwrap();
    solver.refinements = Refinements::MEASURED;
    // Each learned clause is a strict weakening of the original, so deleting
    // it preserves every assignment of this independent six-variable fixture.
    solver
        .attach(
            &[lit(0, false), lit(1, false), lit(2, false)],
            false,
            &mut work,
        )
        .unwrap();
    for extra in 3..6 {
        solver
            .attach(
                &[
                    lit(0, false),
                    lit(1, false),
                    lit(2, false),
                    lit(extra, false),
                ],
                true,
                &mut work,
            )
            .unwrap();
    }
    solver.glue = vec![0, 3, 2, 9];
    solver.trail_lim.push(0);
    for atom in [1, 2, 5] {
        solver
            .assign(lit(atom, true), Reason::None, &mut work)
            .unwrap();
    }
    solver
        .assign(lit(0, false), Reason::Clause(3), &mut work)
        .unwrap();
    solver
}

fn assert_glue_remap(solver: &Solver) {
    assert_eq!(solver.clauses.len(), 3);
    assert_eq!(solver.learned_count, 2);
    assert_eq!(solver.glue, vec![0, 2, 9]);
    assert_eq!(solver.reason[0].unpack(), Reason::Clause(2));
    assert_eq!(
        solver.clauses.get(0),
        &[lit(0, false), lit(1, false), lit(2, false)]
    );
    assert_eq!(
        solver.clauses.get(1),
        &[lit(0, false), lit(1, false), lit(2, false), lit(4, false)]
    );
    assert_eq!(
        solver.clauses.get(2),
        &[lit(0, false), lit(1, false), lit(2, false), lit(5, false)]
    );
    let watches: Vec<_> = solver
        .watches
        .iter()
        .flatten()
        .map(|&w| watch_clause(w))
        .collect();
    assert_eq!(watches.len(), 6);
    for index in 0..3 {
        assert_eq!(watches.iter().filter(|&&ci| ci == index).count(), 2);
    }
    // Check the full family of classical assignments, not only the stored
    // partial assignment or a copied expected remapping implementation.
    for assignment in 0_u32..64 {
        let admitted = (0..solver.clauses.len()).all(|ci| {
            solver
                .clauses
                .get(ci)
                .iter()
                .any(|&l| (assignment & (1 << var(l)) != 0) != (l & 1 != 0))
        });
        assert_eq!(admitted, assignment & 7 != 0);
    }
}

#[test]
fn glue_reduction_preserves_locked_reasons_low_glue_and_complete_assignments() {
    let mut reference = glue_remap_solver();
    let mut work = Meter::local(u64::MAX);
    reference.reduce(&mut work).unwrap();
    assert_glue_remap(&reference);
    let full = work.charged();
    assert!(full > 7);
    let cancellation = Cancellation::default();
    for maximum in [0, 1, 7, full / 2, full - 1] {
        let mut scratch = SmallScratch::default();
        scratch.solver = Some(glue_remap_solver());
        let refused = scratch.attempt(&cancellation, &mut Meter::local(maximum), |s, w| {
            s.solver.as_mut().unwrap().reduce(w)
        });
        assert_eq!(refused.result, Err(Failure::Stopped(Stop::WorkLimit)));
        assert_eq!(refused.charged, maximum);
        assert!(
            scratch.solver.is_none(),
            "partial arena/remap state must be discarded"
        );
        scratch.solver = Some(glue_remap_solver());
        let retry = scratch.attempt(&cancellation, &mut Meter::local(u64::MAX), |s, w| {
            s.solver.as_mut().unwrap().reduce(w)
        });
        assert_eq!(retry.result, Ok(()));
        assert_eq!(retry.charged, full);
        assert_glue_remap(scratch.solver.as_ref().unwrap());
    }
}

fn family_group_fixture() -> (Solver, Vec<Pb>) {
    let mut solver = Solver::new(14, SmallPolicy::default(), &mut Meter::local(u64::MAX)).unwrap();
    solver.refinements = Refinements::MEASURED.with(Refinements::FAMILY, true);
    let mut pbs = Vec::new();
    for bound in [3, 0, 2, 1] {
        // Equal conditions with different weights, then reversed conditions.
        // The groups' first appearances are B, A, C, deliberately interleaved.
        for (conditions, weights) in [
            (vec![0, 1], vec![2, 1]),
            (vec![0, 1], vec![1, 2]),
            (vec![1, 0], vec![1, 2]),
        ] {
            pbs.push(Pb {
                out: idx(2 + pbs.len()),
                conditions,
                weights,
                bound,
                sure: 0,
                open: 0,
                heaviest: 0,
                family: NO_FAMILY,
            });
        }
    }
    (solver, pbs)
}

fn assert_family_groups(solver: &Solver, pbs: &[Pb]) {
    assert_eq!(solver.families.len(), 3);
    assert_eq!(
        pbs.iter().map(|pb| pb.family).collect::<Vec<_>>(),
        vec![0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2]
    );
    for (family, members) in [[3_u32, 9, 6, 0], [4, 10, 7, 1], [5, 11, 8, 2]]
        .into_iter()
        .enumerate()
    {
        let actual = &solver.families[family].members;
        assert_eq!(actual.len(), 4);
        assert_eq!(
            actual.iter().map(|item| item.0).collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
        assert_eq!(
            actual.iter().map(|item| item.2).collect::<Vec<_>>(),
            members
        );
        for &(_, out, index) in actual {
            assert_eq!(out, pbs[index as usize].out);
        }
    }
}

#[test]
fn family_grouping_preserves_whole_keys_member_order_and_typed_refusal() {
    let (mut reference, mut pbs) = family_group_fixture();
    let mut work = Meter::local(u64::MAX);
    assert_eq!(
        reference.add_families(0, &mut pbs, &mut work).unwrap(),
        vec![0, 1, 2]
    );
    assert_family_groups(&reference, &pbs);
    let full = work.charged();
    assert!(full > 8);
    let cancellation = Cancellation::default();
    for maximum in [0, 1, 8, full / 2, full - 1] {
        let (solver, mut pbs) = family_group_fixture();
        let mut scratch = SmallScratch::default();
        scratch.solver = Some(solver);
        let refused = scratch.attempt(&cancellation, &mut Meter::local(maximum), |s, w| {
            s.solver.as_mut().unwrap().add_families(0, &mut pbs, w)
        });
        assert_eq!(refused.result, Err(Failure::Stopped(Stop::WorkLimit)));
        assert_eq!(refused.charged, maximum);
        assert!(scratch.solver.is_none());
        let (solver, mut fresh_pbs) = family_group_fixture();
        scratch.solver = Some(solver);
        let retry = scratch.attempt(&cancellation, &mut Meter::local(u64::MAX), |s, w| {
            s.solver
                .as_mut()
                .unwrap()
                .add_families(0, &mut fresh_pbs, w)
        });
        assert_eq!(retry.result, Ok(vec![0, 1, 2]));
        assert_eq!(retry.charged, full);
        assert_family_groups(scratch.solver.as_ref().unwrap(), &fresh_pbs);
    }
}
