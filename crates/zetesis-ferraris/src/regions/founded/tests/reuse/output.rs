//! Cutting an unused output preserves coverage of the remaining uncut atoms.

use super::{projection::independent_choice, *};

/// The second occurrence of c is a body input; the first is only a choice head.
fn positive_alias() -> Theory {
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
            Node::atom(2),
            Node::implies(9, 0),
        ],
        vec![5, 6, 7, 8, 10],
    )
}

/// {c}; a <- b; b <- a; a <- not not c. Cutting c falsifies the
/// opaque body, although no positive Atom occurrence of c is projected.
fn opaque_seed() -> Theory {
    theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(2, 3),
            Node::implies(4, 3),
            Node::or_pair([2, 4]),
            Node::implies(1, 0),
            Node::implies(0, 1),
            Node::implies(5, 0),
        ],
        vec![6, 7, 8, 9],
    )
}

#[test]
fn any_included_occurrence_invalidates_a_false_atom() {
    let program = Program::new(positive_alias());
    let (mut child, mut knowledge, mut scratch) = program.closed();
    assert!(!scratch.founded.may_affect_body(2));
    assert!(scratch.founded.may_affect_body(9));
    let before = scratch.founded.passes;
    child.cut(2);
    program.close(&mut child, &mut knowledge, &mut scratch);
    assert_eq!(scratch.founded.passes, before + 1);
    assert!(child.is_open(0) && child.is_open(1));
    assert!(completed(&knowledge));
}

#[test]
fn a_cut_output_can_invalidate_an_opaque_body() {
    let program = Program::new(opaque_seed());
    let (mut child, mut knowledge, mut scratch) = program.closed();
    assert!(!scratch.founded.may_affect_body(2));
    assert!(scratch.founded.may_affect_body(5));
    let before = scratch.founded.passes;
    child.cut(2);
    program.close(&mut child, &mut knowledge, &mut scratch);
    assert!(child.is_cut(0) && child.is_cut(1));
    assert_eq!(scratch.founded.passes, before + 2);
    assert!(completed(&knowledge));
}

#[test]
fn a_new_true_negative_body_preserves_completion() {
    let program = Program::new(negative_seed());
    let (mut child, mut knowledge, mut scratch) = program.closed();
    assert!(!scratch.founded.may_affect_body(2));
    let before = scratch.founded.passes;
    child.cut(2);
    program.close(&mut child, &mut knowledge, &mut scratch);
    assert!(child.is_held(0) && child.is_held(1));
    assert_eq!(scratch.founded.passes, before);
    assert!(completed(&knowledge));
}

#[test]
fn unknown_projection_invalidates_output_atoms() {
    let program = Program::new(independent_choice());
    let (parent, inherited, _) = program.closed();
    for foreign in [false, true] {
        let mut scratch = if foreign {
            Program::new(positive_alias()).closed().2
        } else {
            NarrowingScratch::default()
        };
        let before = scratch.founded.passes;
        let mut child = parent.clone();
        let mut knowledge = inherited.clone();
        child.cut(2);
        program.close(&mut child, &mut knowledge, &mut scratch);
        assert_eq!(scratch.founded.passes, before + 1);
        assert!(completed(&knowledge));
        assert!(
            scratch
                .founded
                .owner
                .as_ref()
                .unwrap()
                .same_instance(&program.owner)
        );
    }
}

#[test]
fn output_reuse_preserves_complete_candidate_families() {
    for owner in [
        independent_choice(),
        positive_alias(),
        negative_seed(),
        opaque_seed(),
    ] {
        let program = Program::new(owner);
        let (actual, reused) = program.candidates(false);
        let (reference, repeated) = program.candidates(true);
        assert_eq!(actual, reference);
        assert!(reused <= repeated);
        let expected: BTreeSet<_> = (0..8)
            .filter(|&mask| accepted(&program.owner, mask))
            .collect();
        let answers: BTreeSet<_> = actual
            .into_iter()
            .filter(|&mask| accepted(&program.owner, mask))
            .collect();
        assert_eq!(answers, expected);
    }
}

#[test]
fn stopped_output_events_retry_from_the_parent() {
    let program = Program::new(independent_choice());
    let (parent, inherited, mut scratch) = program.closed();
    let mut expected = parent.clone();
    expected.cut(2);
    let mut expected_known = inherited.clone();
    let (expected_result, statistics) =
        program.close(&mut expected, &mut expected_known, &mut scratch);
    assert_eq!(scratch.founded.passes, 1);
    assert!(statistics.work > 0);
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for ceiling in 0..statistics.work {
            let (_, _, mut scratch) = program.closed();
            let mut child = parent.clone();
            child.cut(2);
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
            let mut retry = parent.clone();
            retry.cut(2);
            partial.clone_from(&inherited);
            let (actual, _) = program.close(&mut retry, &mut partial, &mut scratch);
            assert_eq!(actual, expected_result);
            assert_eq!(retry, expected);
            assert_eq!(scratch.founded.passes, 1);
            assert!(completed(&partial));
        }
    }
}
