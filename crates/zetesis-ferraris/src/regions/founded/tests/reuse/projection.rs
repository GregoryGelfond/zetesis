//! False events outside the exact body projection preserve inherited closure.

use super::*;

/// a or b; a <- b; b <- a; {c}. The choice's not-c node is outside
/// every producer body, although holding c makes that original node false.
pub(super) fn independent_choice() -> Theory {
    theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(2, 3),
            Node::or_pair([2, 4]),
            Node::or_pair([0, 1]),
            Node::implies(0, 1),
            Node::implies(1, 0),
        ],
        vec![5, 6, 7, 8],
    )
}

fn node_never(knowledge: &Knowledge, node: usize) -> bool {
    let masks = match &knowledge.width {
        Width::Compact16(known) => known.masks.slices(),
        Width::Compact32(known) => known.masks.slices(),
        Width::Native(known) => known.masks.slices(),
    };
    bit(masks[1], node)
}

#[test]
fn an_irrelevant_false_node_keeps_completed_foundation() {
    let program = Program::new(independent_choice());
    let (parent, inherited, mut scratch) = program.closed();
    assert!(!scratch.founded.may_affect_body(4));
    let bytes = scratch.founded.retained_bytes();
    let mut child = parent.clone();
    child.hold(2);
    let mut knowledge = inherited.clone();
    let before = scratch.founded.passes;
    let (result, _) = program.close(&mut child, &mut knowledge, &mut scratch);
    assert_eq!(result, Narrowing::Fixed { changed: false });
    assert!(
        node_never(&knowledge, 4),
        "holding c falsifies its negative alternative"
    );
    assert_eq!(scratch.founded.passes, before, "no new relaxation input");
    assert!(completed(&knowledge));
    assert_eq!(scratch.founded.retained_bytes(), bytes);

    let mut reference = parent;
    reference.hold(2);
    let mut reference_known = inherited;
    invalidate(&mut reference_known);
    let (reference_result, _) = program.close(&mut reference, &mut reference_known, &mut scratch);
    assert_eq!(reference_result, result);
    assert_eq!(reference, child);
    assert_eq!(scratch.founded.passes, before + 1);
}

#[test]
fn false_output_atoms_keep_completed_foundation() {
    let program = Program::new(independent_choice());
    let (mut child, mut knowledge, mut scratch) = program.closed();
    assert!(!scratch.founded.may_affect_body(2));
    let before = scratch.founded.passes;
    child.cut(2);
    program.close(&mut child, &mut knowledge, &mut scratch);
    assert!(child.is_cut(2));
    assert_eq!(scratch.founded.passes, before);
    assert!(completed(&knowledge));
}

#[test]
fn blocked_body_nodes_remain_inputs_across_siblings() {
    let program = Program::new(negative_seed());
    let (parent, inherited, mut scratch) = program.closed();
    for _ in 0..2 {
        // Keep the completed projection, including its blocked cells, but
        // remove optional explanations so this test requires actual closure
        // invalidation rather than a retained row cutting the cycle first.
        scratch.explanations = crate::regions::founded::ledger::Ledger::default();
        let mut child = parent.clone();
        child.hold(2);
        let mut knowledge = inherited.clone();
        let before = scratch.founded.passes;
        let (_, counts) = program.close(&mut child, &mut knowledge, &mut scratch);
        assert!(child.is_cut(0) && child.is_cut(1));
        assert_eq!(counts.support_explanations.matched, 0);
        assert_eq!(counts.support_explanations.cut, 0);
        assert_eq!(scratch.founded.passes, before + 2);
        assert!(matches!(scratch.founded.states[4], State::Inactive));
        assert!(scratch.founded.may_affect_body(4));
        assert!(completed(&knowledge));
    }
}

#[test]
fn unprepared_workers_invalidate_even_outside_nodes() {
    let program = Program::new(independent_choice());
    let (mut child, mut knowledge, _) = program.closed();
    let mut scratch = NarrowingScratch::default();
    assert!(scratch.founded.may_affect_body(4));
    child.hold(2);
    program.close(&mut child, &mut knowledge, &mut scratch);
    assert_eq!(scratch.founded.passes, 1, "unknown projection recomputed");
    assert!(!scratch.founded.may_affect_body(4));
    assert!(completed(&knowledge));
}

#[test]
fn foreign_outside_nodes_cannot_hide_changed_bodies() {
    let foreign = Program::new(independent_choice());
    let (_, _, mut scratch) = foreign.closed();
    assert!(!scratch.founded.may_affect_body(4));
    let program = Program::new(negative_seed());
    let (mut child, mut knowledge, _) = program.closed();
    child.hold(2);
    let before = scratch.founded.passes;
    program.close(&mut child, &mut knowledge, &mut scratch);
    assert!(child.is_cut(0) && child.is_cut(1));
    assert_eq!(scratch.founded.passes, before + 2);
    assert!(completed(&knowledge));
}

#[test]
fn interrupted_projection_has_no_outside_authority() {
    let foreign = Program::new(independent_choice());
    let (_, _, mut scratch) = foreign.closed();
    let program = Program::new(negative_seed());
    let (mut child, mut knowledge, _) = program.closed();
    assert_eq!(
        scratch.founded.prepare_bodies(
            &program.owner,
            &program.index,
            &program.producers,
            &mut Work::new(6),
        ),
        Err(Stop::WorkLimit)
    );
    assert!(matches!(scratch.founded.states[4], State::Outside));
    assert!(scratch.founded.owner.is_none());
    assert!(scratch.founded.may_affect_body(4));
    child.hold(2);
    program.close(&mut child, &mut knowledge, &mut scratch);
    assert!(child.is_cut(0) && child.is_cut(1));
    assert!(completed(&knowledge));
}

#[test]
fn projected_reuse_preserves_the_complete_candidate_family() {
    let program = Program::new(independent_choice());
    let (actual, reused) = program.candidates(false);
    let (reference, repeated) = program.candidates(true);
    assert_eq!(actual, reference);
    assert!(reused < repeated, "family traversals actually omit passes");
    let expected: BTreeSet<_> = (0..8)
        .filter(|&mask| accepted(&program.owner, mask))
        .collect();
    assert_eq!(expected, BTreeSet::from([3, 7]));
    let answers: BTreeSet<_> = actual
        .into_iter()
        .filter(|&mask| accepted(&program.owner, mask))
        .collect();
    assert_eq!(answers, expected);
}

#[test]
fn stopped_irrelevant_events_keep_retry_authority_local() {
    let program = Program::new(independent_choice());
    let (parent, inherited, mut scratch) = program.closed();
    let mut expected = parent.clone();
    expected.hold(2);
    let mut expected_known = inherited.clone();
    let (expected_result, statistics) =
        program.close(&mut expected, &mut expected_known, &mut scratch);
    assert_eq!(scratch.founded.passes, 1);
    assert!(statistics.work > 0);
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for ceiling in 0..statistics.work {
            let (_, _, mut scratch) = program.closed();
            let mut child = parent.clone();
            child.hold(2);
            let mut partial = inherited.clone();
            let attempt = program.attempt(
                &mut child,
                &mut partial,
                &mut scratch,
                &mut RefuseAfter {
                    remaining: ceiling,
                    failure,
                },
            );
            assert_eq!(attempt.result, Err(failure));
            assert_eq!(attempt.statistics.work, ceiling);
            assert_eq!(scratch.founded.passes, 1);
            // Discard failed knowledge, preserving only the successful parent.
            let mut retry = parent.clone();
            retry.hold(2);
            partial.clone_from(&inherited);
            let (actual, _) = program.close(&mut retry, &mut partial, &mut scratch);
            assert_eq!(actual, expected_result);
            assert_eq!(retry, expected);
            assert_eq!(scratch.founded.passes, 1);
            assert!(completed(&partial));
        }
    }
}
