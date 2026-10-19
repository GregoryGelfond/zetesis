//! A retained condition can force its only open premise away from a held output.

use super::super::Deduction;
use super::*;

/// Add one implication to the existing fixture, whose nodes all have inline
/// operands. It makes an ordinary event from the required premise observable.
fn with_consequence(body: usize, head: usize) -> Program {
    let base = Program::new(false);
    let mut nodes = base.owner.nodes().to_vec();
    let mut roots = base.owner.roots().to_vec();
    roots.push(nodes.len());
    nodes.push(Node::implies(body, head));
    let owner = theory(5, nodes, roots);
    let index = Narrower::new(&owner);
    let producers = crate::producers(&owner, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .producers
        .unwrap();
    Program {
        owner,
        index,
        producers,
    }
}

#[test]
fn an_open_absence_condition_uses_atom_events() {
    // The retained row is (not x and y) -> absence of a,b. Holding a,y
    // therefore requires x. The added x -> z must receive the atom event.
    let program = with_consequence(2, 4);
    let initial = region(&[(0, true), (3, true)]);
    let mut without_row = initial.clone();
    let (result, _) = program.close(
        &mut without_row,
        &mut program.index.knowledge(),
        &mut NarrowingScratch::default(),
    );
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert_eq!(without_row.decision(2), None);
    assert_eq!(without_row.decision(4), None);

    let (_, mut scratch) = program.captured();
    let mut narrowed = initial;
    let (result, counts) =
        program.close(&mut narrowed, &mut program.index.knowledge(), &mut scratch);
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert_eq!(narrowed.decision(2), Some(true));
    assert_eq!(narrowed.decision(4), Some(true));
    assert_eq!(counts.support_explanations.forced, 1);
    assert_eq!(counts.support_explanations.matched, 1);
    assert_eq!(counts.support_explanations.cut, 0);
    assert_eq!(counts.support_explanations.refuted, 0);
}

#[test]
fn an_open_presence_condition_uses_atom_events() {
    // Holding a while x is absent requires y absent. The added z -> y
    // must then exclude z through ordinary implication propagation.
    let program = with_consequence(4, 3);
    let initial = region(&[(0, true), (2, false)]);
    let mut without_row = initial.clone();
    let (result, _) = program.close(
        &mut without_row,
        &mut program.index.knowledge(),
        &mut NarrowingScratch::default(),
    );
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert_eq!(without_row.decision(3), None);
    assert_eq!(without_row.decision(4), None);

    let (_, mut scratch) = program.captured();
    let mut narrowed = initial;
    let (result, counts) =
        program.close(&mut narrowed, &mut program.index.knowledge(), &mut scratch);
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert_eq!(narrowed.decision(3), Some(false));
    assert_eq!(narrowed.decision(4), Some(false));
    assert_eq!(counts.support_explanations.forced, 1);
    assert_eq!(counts.support_explanations.matched, 1);
    assert_eq!(counts.support_explanations.cut, 0);
    assert_eq!(counts.support_explanations.refuted, 0);
}

#[test]
fn a_later_opposite_condition_prevents_premise_forcing() {
    let program = Program::new(false);
    let (_, mut scratch) = program.captured();
    // The first stored condition, x absent, is open. The later y-held
    // condition is contradicted; no inference may be published early.
    let mut narrowed = region(&[(0, true), (3, false)]);
    let (result, counts) =
        program.close(&mut narrowed, &mut program.index.knowledge(), &mut scratch);
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert_eq!(narrowed.decision(2), None);
    assert_eq!(counts.support_explanations.forced, 0);
    assert_eq!(counts.support_explanations.matched, 0);
}

#[test]
fn two_open_conditions_force_neither_premise() {
    let program = Program::new(false);
    let (_, mut scratch) = program.captured();
    let mut narrowed = region(&[(0, true)]);
    let (result, counts) =
        program.close(&mut narrowed, &mut program.index.knowledge(), &mut scratch);
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert_eq!(narrowed.decision(2), None);
    assert_eq!(narrowed.decision(3), None);
    assert_eq!(counts.support_explanations.forced, 0);
    assert_eq!(counts.support_explanations.matched, 0);
}

#[test]
fn an_open_condition_needs_a_held_unsupported_output() {
    let program = Program::new(false);
    let (_, mut scratch) = program.captured();
    for (decisions, open) in [([(3, true)], 2), ([(2, false)], 3)] {
        let mut narrowed = region(&decisions);
        let (result, counts) =
            program.close(&mut narrowed, &mut program.index.knowledge(), &mut scratch);
        assert!(matches!(result, Narrowing::Fixed { .. }));
        assert!(!narrowed.is_held(0) && !narrowed.is_held(1));
        assert_eq!(narrowed.decision(open), None);
        assert_eq!(counts.support_explanations.forced, 0);
        assert_eq!(counts.support_explanations.matched, 0);
    }
}

#[test]
fn a_foreign_owner_cannot_force_a_partial_premise() {
    let original = Program::new(false);
    let foreign = Program::new(true);
    let (_, mut scratch) = original.captured();
    let mut narrowed = region(&[(0, true), (3, true)]);
    let (result, counts) =
        foreign.close(&mut narrowed, &mut foreign.index.knowledge(), &mut scratch);
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert_eq!(narrowed.decision(2), None);
    assert_eq!(counts.support_explanations.forced, 0);
    assert_eq!(scratch.explanations.len(), 0);
}

#[test]
fn an_inventory_free_visit_cannot_force_a_partial_premise() {
    let program = Program::new(false);
    let (_, mut scratch) = program.captured();
    let initial = region(&[(0, true), (3, true)]);
    let mut narrowed = initial.clone();
    let (result, counts) = program
        .index
        .narrow_known(
            OriginalSubject::new(&program.owner, None),
            &mut narrowed,
            &mut program.index.knowledge(),
            &mut scratch,
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert_eq!(narrowed.decision(2), None);
    assert_eq!(counts.support_explanations.work, 0);
    assert_eq!(counts.support_explanations.forced, 0);
    assert_eq!(scratch.explanations.len(), 1);

    let mut original = initial;
    let (result, counts) =
        program.close(&mut original, &mut program.index.knowledge(), &mut scratch);
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert_eq!(counts.support_explanations.forced, 1);
    assert_eq!(original.decision(2), Some(true));
}

#[test]
fn a_frozen_visit_cannot_force_a_partial_premise() {
    let program = Program::new(false);
    let (_, mut scratch) = program.captured();
    // Keep the entire initial region below the frozen candidate. Compare with
    // a fresh reduct query rather than assuming the reduct leaves the premise
    // open: only the original-theory explanation must be ignored.
    let candidate = world(&program.owner, 0b11111);
    let mut workspace = EvaluationWorkspace::default();
    let evaluated = workspace
        .evaluate(
            &candidate,
            EvaluationLimits::default(),
            &Cancellation::default(),
        )
        .result
        .unwrap();
    assert!(evaluated.is_model());
    let initial = region(&[(0, true), (3, true)]);
    let mut with_row = initial.clone();
    let mut without_row = initial;
    let subject = FrozenSubject::new(&program.owner, evaluated.truth());
    let (expected, _) = program
        .index
        .narrow_frozen_known(
            subject,
            &mut without_row,
            &mut program.index.knowledge(),
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    let (actual, counts) = program
        .index
        .narrow_frozen_known(
            subject,
            &mut with_row,
            &mut program.index.knowledge(),
            &mut scratch,
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(actual, expected);
    for atom in 0..5 {
        assert_eq!(with_row.decision(atom), without_row.decision(atom));
    }
    assert_eq!(counts.support_explanations.work, 0);
    assert_eq!(counts.support_explanations.forced, 0);
    assert_eq!(scratch.explanations.len(), 1);
}

#[test]
fn every_interrupted_partial_match_preserves_its_typed_failure() {
    let program = Program::new(false);
    let (_, scratch) = program.captured();
    let held = [(1 << 0) | (1 << 3)];
    let cut = [0];
    let expected = Deduction::Require {
        atom: 2,
        value: true,
    };
    let mut measured = Work::new(u64::MAX);
    assert_eq!(
        scratch
            .explanations
            .deduction(0, &held, &cut, &mut measured)
            .unwrap(),
        expected
    );
    assert!(measured.spent > 0);
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for prefix in 0..measured.spent {
            let mut permits = SinglePermits::new(prefix, failure);
            let mut work = Work::reserved(&mut permits);
            assert_eq!(
                scratch.explanations.deduction(0, &held, &cut, &mut work),
                Err(failure)
            );
            assert_eq!(work.spent, prefix);
            assert_eq!(scratch.explanations.len(), 1);
            assert_eq!(
                scratch
                    .explanations
                    .deduction(0, &held, &cut, &mut Work::new(u64::MAX))
                    .unwrap(),
                expected
            );
        }
    }
}
