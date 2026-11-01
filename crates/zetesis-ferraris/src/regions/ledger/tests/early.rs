//! A successful original support cut can supply a reusable conditional absence.

use super::*;

fn capture_from_pruning(program: &Program) -> NarrowingScratch {
    let mut original = region(&[(2, false), (3, true)]);
    assert!(!original.is_held(0) && !original.is_held(1));
    let mut scratch = NarrowingScratch::default();
    let (result, counts) =
        program.close(&mut original, &mut program.index.knowledge(), &mut scratch);
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert!(original.is_cut(0) && original.is_cut(1));
    assert!(scratch.common.founded.passes > 0);
    assert_eq!(counts.support_explanations.attempts, 1);
    assert_eq!(counts.support_explanations.certified, 1);
    assert_eq!(counts.support_explanations.refuted, 0);
    assert_eq!(scratch.learning.explanations.len(), 1);
    assert_eq!(
        scratch.learning.explanations.rows[0]
            .row
            .conditions()
            .collect::<Vec<_>>(),
        [(2, false), (3, true)]
    );
    assert_eq!(scratch.learning.explanations.absent_outputs(0), [0, 1]);
    scratch
}

#[test]
fn successful_pruning_supplies_a_sibling_deduction() {
    let program = Program::new(false);
    let mut scratch = capture_from_pruning(&program);
    let mut sibling = region(&[(2, false), (3, true), (4, true)]);
    let (result, counts) =
        program.close(&mut sibling, &mut program.index.knowledge(), &mut scratch);
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert!(sibling.is_cut(0) && sibling.is_cut(1));
    assert_eq!(counts.support_explanations.cut, 2);
    assert_eq!(counts.support_explanations.matched, 1);
    assert_eq!(counts.support_explanations.attempts, 0);
}

#[test]
fn an_early_explanation_can_force_a_later_open_premise() {
    let program = Program::new(false);
    let mut scratch = capture_from_pruning(&program);
    let mut sibling = region(&[(0, true), (3, true)]);
    let (result, counts) =
        program.close(&mut sibling, &mut program.index.knowledge(), &mut scratch);
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert_eq!(sibling.decision(2), Some(true));
    assert_eq!(counts.support_explanations.forced, 1);
    assert_eq!(counts.support_explanations.matched, 1);
}

#[test]
fn early_rows_preserve_every_answer_in_all_partial_regions() {
    let program = Program::new(false);
    let answers: Vec<_> = (0..32)
        .filter(|&mask| {
            crate::check(
                &program.theory,
                &world(&program.theory, mask),
                crate::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .accepted()
        })
        .collect();
    let mut scratch = capture_from_pruning(&program);
    let mut forced = 0;
    let mut excluded = 0;
    for mut code in 0..243 {
        let mut initial = Region::all_open(5);
        for atom in 0..5 {
            match code % 3 {
                1 => assert!(initial.cut(atom)),
                2 => assert!(initial.hold(atom)),
                _ => {}
            }
            code /= 3;
        }
        let mut narrowed = initial.clone();
        let (result, counts) =
            program.close(&mut narrowed, &mut program.index.knowledge(), &mut scratch);
        forced += counts.support_explanations.forced;
        excluded += counts.support_explanations.cut + counts.support_explanations.refuted;
        for &answer in &answers {
            if compatible(&initial, answer) {
                assert_ne!(result, Narrowing::Refuted);
                assert!(compatible(&narrowed, answer));
            }
        }
    }
    assert!(
        forced > 0,
        "partial matches must exercise premise propagation"
    );
    assert!(
        excluded > 0,
        "complete matches must exercise output exclusion"
    );
}

#[test]
fn early_rows_are_suspended_without_original_producer_coverage() {
    let program = Program::new(false);
    let mut scratch = capture_from_pruning(&program);
    let mut workspace = EvaluationWorkspace::default();
    let candidate = world(&program.theory, 0b11111);
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
    let mut frozen = initial.clone();
    let mut reference = initial.clone();
    let subject = FrozenSubject::new(&program.theory, evaluated.truth());
    let expected = program
        .index
        .narrow_frozen_known(
            subject,
            &mut reference,
            &mut program.index.frozen_knowledge(),
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .result
        .unwrap();
    let attempt = program.index.narrow_frozen_known(
        subject,
        &mut frozen,
        &mut program.index.frozen_knowledge(),
        &mut scratch,
        RegionLimits::default(),
        &Cancellation::default(),
    );
    let actual = attempt.result.unwrap();
    let counts = attempt.statistics;
    assert_eq!(actual, expected);
    for atom in 0..5 {
        assert_eq!(frozen.decision(atom), reference.decision(atom));
    }
    assert_eq!(counts.support_explanations.work, 0);
    assert_eq!(scratch.learning.explanations.len(), 1);
    let mut restriction = initial;
    let attempt = program.index.narrow_known(
        OriginalSubject::new(&program.theory, None),
        &mut restriction,
        &mut program.index.knowledge(),
        &mut scratch,
        RegionLimits::default(),
        &Cancellation::default(),
    );
    attempt.result.unwrap();
    let counts = attempt.statistics;
    assert_eq!(restriction.decision(2), None);
    assert_eq!(counts.support_explanations.work, 0);
    assert_eq!(scratch.learning.explanations.len(), 1);
}

#[test]
fn a_foreign_owner_discards_early_rows() {
    let original = Program::new(false);
    let foreign = Program::new(true);
    let mut scratch = capture_from_pruning(&original);
    let mut narrowed = region(&[(0, true), (3, true)]);
    let (result, counts) =
        foreign.close(&mut narrowed, &mut foreign.index.knowledge(), &mut scratch);
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert_eq!(narrowed.decision(2), None);
    assert_eq!(counts.support_explanations.forced, 0);
    assert_eq!(scratch.learning.explanations.len(), 0);
}

fn separate_cycles() -> Program {
    // Different mask words require priority to survive discovery of an
    // earlier open unsupported head. Other atoms have no producer.
    let mut nodes: Vec<_> = (0..66).map(Node::atom).collect();
    let mut roots = Vec::new();
    for (body, head) in [(0, 1), (1, 0), (64, 65), (65, 64)] {
        roots.push(nodes.len());
        nodes.push(Node::implies(body, head));
    }
    Program::of(theory(66, nodes, roots))
}

#[test]
fn a_later_held_head_has_capture_priority() {
    let program = separate_cycles();
    let mut original = Region::all_open(66);
    assert!(original.hold(64));
    let mut scratch = NarrowingScratch::default();
    let (result, counts) =
        program.close(&mut original, &mut program.index.knowledge(), &mut scratch);
    assert_eq!(result, Narrowing::Refuted);
    assert_eq!(counts.support_explanations.attempts, 1);
    assert_eq!(counts.support_explanations.certified, 1);
    assert_eq!(scratch.learning.explanations.len(), 1);
    assert_eq!(scratch.learning.explanations.absent_outputs(0), [64, 65]);
}

#[test]
fn one_completed_pass_captures_at_most_one_open_component() {
    let program = separate_cycles();
    let mut original = Region::all_open(66);
    let mut scratch = NarrowingScratch::default();
    let (result, counts) =
        program.close(&mut original, &mut program.index.knowledge(), &mut scratch);
    assert!(matches!(result, Narrowing::Fixed { .. }));
    for atom in [0, 1, 64, 65] {
        assert!(original.is_cut(atom));
    }
    assert_eq!(counts.support_explanations.attempts, 1);
    assert_eq!(scratch.learning.explanations.len(), 1);
    assert_eq!(scratch.learning.explanations.absent_outputs(0), [0, 1]);
}

#[test]
fn unfinished_optional_discovery_keeps_the_original_cuts() {
    let program = separate_cycles();
    let mut original = Region::all_open(66);
    let mut scratch = NarrowingScratch::default();
    scratch.learning.explanations.prepare(&program.owner);
    scratch.learning.explanations.remaining = 1;
    let (result, counts) =
        program.close(&mut original, &mut program.index.knowledge(), &mut scratch);
    assert!(matches!(result, Narrowing::Fixed { .. }));
    for atom in [0, 1, 64, 65] {
        assert!(original.is_cut(atom));
    }
    assert!(scratch.common.founded.passes > 0);
    assert_eq!(counts.support_explanations.attempts, 0);
    assert_eq!(scratch.learning.explanations.len(), 0);
}
