//! Native groups retain one graph across original, frozen and specialized reads.

use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{
    AdmissionLimits, EvaluationError, EvaluationLimits, EvaluationWorkspace, FormulaParts,
    FrozenReduct, Interpretation, Limits, Node, OperandSpan, PositivePlan, PositivePlanLimits,
    Theory, TightCheckLimits, TightPlan, TightPlanLimits, TightVerdict,
};

use super::support::worlds::eval;

fn shared(width: usize, root: usize) -> Theory {
    let mut operands: Vec<_> = (0..width).map(|position| position % 2).collect();
    operands.extend((0..width).map(|position| if position % 2 == 0 { 3 } else { 1 }));
    operands.extend([4, 6, 0]);
    Theory::new(
        2,
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::falsum(),
                Node::implies(0, 2),
                Node::and_span(OperandSpan {
                    start: 0,
                    length: width,
                }),
                Node::or_span(OperandSpan {
                    start: width,
                    length: width,
                }),
                Node::implies(4, 5),
                Node::or_span(OperandSpan {
                    start: 2 * width,
                    length: 3,
                }),
            ],
            operands,
        )
        .unwrap(),
        vec![root],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn world(theory: &Theory, mask: u8) -> Interpretation {
    Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|&atom| mask & (1 << atom) != 0),
    )
    .unwrap()
}

#[test]
fn native_shared_groups_preserve_arbitrary_frozen_interpretations() {
    let cancellation = Cancellation::default();
    for width in [3, 65, 129] {
        for root in [4, 5, 6, 7] {
            let theory = shared(width, root);
            // Width is retained in the one operand arena, not expanded to nodes.
            assert_eq!(theory.nodes().len(), 8);
            assert_eq!(theory.operands().len(), 2 * width + 3);
            assert_eq!(theory.parts().occurrences(), 2 * width + 7);
            for candidate_mask in 0..4 {
                let candidate = world(&theory, candidate_mask);
                let frozen =
                    FrozenReduct::new(&candidate, Limits::default(), &cancellation).unwrap();
                let mut workspace = EvaluationWorkspace::default();
                let attempt =
                    workspace.evaluate(&candidate, EvaluationLimits::default(), &cancellation);
                let original = attempt.result.unwrap();
                for index in 0..theory.view().len() {
                    assert_eq!(
                        original.node_truth(index),
                        Some(eval(theory.view(), index, candidate_mask, None))
                    );
                }
                for tested_mask in 0..4 {
                    let tested = world(&theory, tested_mask);
                    assert_eq!(
                        frozen
                            .is_satisfied_by(&tested, Limits::default(), &cancellation)
                            .unwrap(),
                        eval(theory.view(), root, tested_mask, Some(candidate_mask)),
                        "width {width}, root {root}, M {candidate_mask}, J {tested_mask}",
                    );
                }
            }
        }
    }
}

#[test]
fn every_partial_operand_scan_returns_its_work_without_truth() {
    let width = 65;
    let theory = Theory::new(
        1,
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::and_span(OperandSpan {
                    start: 0,
                    length: width,
                }),
            ],
            vec![0; width],
        )
        .unwrap(),
        vec![1],
        AdmissionLimits::default(),
    )
    .unwrap();
    let candidate = world(&theory, 0);
    let complete_work = u64::try_from(2 + width + 1).unwrap();
    let mut workspace = EvaluationWorkspace::default();
    for max_work in 0..=complete_work {
        let attempt = workspace.evaluate(
            &candidate,
            EvaluationLimits {
                max_work,
                ..EvaluationLimits::default()
            },
            &Cancellation::default(),
        );
        assert_eq!(attempt.work, max_work);
        if max_work == complete_work {
            assert!(!attempt.result.unwrap().is_model());
        } else {
            assert_eq!(
                attempt.result.unwrap_err(),
                EvaluationError::Stopped(Stop::WorkLimit)
            );
        }
    }
}

fn duplicate_conjunction(width: usize) -> Theory {
    Theory::new(
        3,
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::atom(2),
                Node::and_span(OperandSpan {
                    start: 0,
                    length: width,
                }),
                Node::implies(3, 2),
            ],
            (0..width).map(|position| position % 2).collect(),
        )
        .unwrap(),
        vec![0, 1, 4],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn native_duplicate_conjuncts_drive_positive_support() {
    let width = 129;
    let theory = duplicate_conjunction(width);
    let cancellation = Cancellation::default();
    let positive =
        PositivePlan::compile(&theory, PositivePlanLimits::default(), &cancellation).unwrap();
    assert_eq!(
        positive.least_consequences().atoms().collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(positive.statistics().dependencies, 3 + width + 1);
    assert_eq!(positive.statistics().propagated_dependencies, 3 + width + 1);
}

#[test]
fn native_ranked_support_retains_partial_operand_work() {
    let width = 129;
    let theory = duplicate_conjunction(width);
    let cancellation = Cancellation::default();
    let tight = TightPlan::compile(&theory, TightPlanLimits::default(), &cancellation).unwrap();
    let candidate = Interpretation::new(&theory, [0, 1, 2]).unwrap();
    let complete = tight.check_accounted(&candidate, TightCheckLimits::default(), &cancellation);
    assert_eq!(complete.result.unwrap().verdict, TightVerdict::Stable);
    for max_work in 3..u64::try_from(4 + width).unwrap() {
        let partial = tight.check_accounted(
            &candidate,
            TightCheckLimits {
                max_work,
                ..TightCheckLimits::default()
            },
            &cancellation,
        );
        assert_eq!(partial.work, max_work);
        assert!(partial.result.is_err());
    }
}

#[test]
fn native_frozen_narrowing_keeps_every_reduct_model_in_each_region() {
    use zetesis_ferraris::{
        FrozenSubject, Narrower, Narrowing, NarrowingScratch, Region, RegionLimits,
    };
    let cancellation = Cancellation::default();
    for width in [3, 65, 129] {
        for root in [4, 5, 6, 7] {
            let theory = shared(width, root);
            let narrower = Narrower::new(&theory);
            assert_eq!(
                narrower.work(),
                u64::try_from(theory.nodes().len() + theory.parts().occurrences()).unwrap()
            );
            for candidate in 0..4 {
                let interpretation = world(&theory, candidate);
                let mut workspace = EvaluationWorkspace::default();
                let attempt =
                    workspace.evaluate(&interpretation, EvaluationLimits::default(), &cancellation);
                let truth = attempt.result.unwrap();
                for code in 0..9 {
                    let mut region = Region::all_open(2);
                    let mut digits = code;
                    for atom in 0..2 {
                        match digits % 3 {
                            1 => {
                                assert!(region.hold(atom));
                            }
                            2 => {
                                assert!(region.cut(atom));
                            }
                            _ => {}
                        }
                        digits /= 3;
                    }
                    let before = region.clone();
                    let (result, _) = narrower
                        .narrow_frozen_known(
                            FrozenSubject::new(&theory, truth.truth()),
                            &mut region,
                            &mut narrower.knowledge(),
                            &mut NarrowingScratch::default(),
                            RegionLimits::default(),
                            &cancellation,
                        )
                        .unwrap();
                    for tested in 0..4 {
                        let inside = (0..2).all(|atom| {
                            let present = tested & (1 << atom) != 0;
                            (!before.is_held(atom) || present) && (!before.is_cut(atom) || !present)
                        });
                        if inside && eval(theory.view(), root, tested, Some(candidate)) {
                            assert!(matches!(result, Narrowing::Fixed { .. }));
                            for atom in 0..2 {
                                let present = tested & (1 << atom) != 0;
                                assert!(!region.is_held(atom) || present);
                                assert!(!region.is_cut(atom) || !present);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn frozen_negation_keeps_masked_atom_decisions_free() {
    use zetesis_ferraris::{
        FrozenSubject, Narrower, Narrowing, NarrowingScratch, Region, RegionLimits,
    };

    let cancellation = Cancellation::default();
    for duplicate in [false, true] {
        let mut nodes = vec![Node::atom(0), Node::falsum(), Node::implies(0, 1)];
        let root = if duplicate {
            nodes.extend([
                Node::atom(0),
                Node::atom(0),
                Node::or_pair([3, 4]),
                Node::or_pair([2, 5]),
            ]);
            6
        } else {
            2
        };
        let theory = Theory::new(
            1,
            FormulaParts::new(nodes, vec![]).unwrap(),
            vec![root],
            AdmissionLimits::default(),
        )
        .unwrap();
        let candidate = world(&theory, 0);
        let mut workspace = EvaluationWorkspace::default();
        let attempt = workspace.evaluate(&candidate, EvaluationLimits::default(), &cancellation);
        let truth = attempt.result.unwrap();
        let narrower = Narrower::new(&theory);
        // For M = empty, not a freezes to false -> false. Adding a in J
        // cannot change that reduct or revive any masked occurrence of a.
        for tested in 0..2 {
            assert!(eval(theory.view(), root, tested, Some(0)));
        }
        for decision in 0..3 {
            let mut region = Region::all_open(1);
            match decision {
                1 => assert!(region.hold(0)),
                2 => assert!(region.cut(0)),
                _ => {}
            }
            let (result, _) = narrower
                .narrow_frozen_known(
                    FrozenSubject::new(&theory, truth.truth()),
                    &mut region,
                    &mut narrower.knowledge(),
                    &mut NarrowingScratch::default(),
                    RegionLimits::default(),
                    &cancellation,
                )
                .unwrap();
            assert!(
                matches!(result, Narrowing::Fixed { .. }),
                "duplicate={duplicate}, decision={decision}"
            );
            assert_eq!(region.is_held(0), decision == 1);
            assert_eq!(region.is_cut(0), decision == 2);
        }
    }
}

#[test]
fn frozen_atom_root_stays_false_when_its_atom_is_held() {
    use zetesis_ferraris::{
        FrozenSubject, Narrower, Narrowing, NarrowingScratch, Region, RegionLimits,
    };

    let theory = Theory::new(
        1,
        FormulaParts::new(vec![Node::atom(0), Node::atom(0)], vec![]).unwrap(),
        vec![1],
        AdmissionLimits::default(),
    )
    .unwrap();
    let cancellation = Cancellation::default();
    let candidate = world(&theory, 0);
    let mut workspace = EvaluationWorkspace::default();
    let attempt = workspace.evaluate(&candidate, EvaluationLimits::default(), &cancellation);
    let truth = attempt.result.unwrap();
    let narrower = Narrower::new(&theory);
    let mut region = Region::all_open(1);
    assert!(region.hold(0));
    assert!(!eval(theory.view(), 1, 1, Some(0)));
    let (result, _) = narrower
        .narrow_frozen_known(
            FrozenSubject::new(&theory, truth.truth()),
            &mut region,
            &mut narrower.knowledge(),
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &cancellation,
        )
        .unwrap();
    assert!(matches!(result, Narrowing::Refuted));
}
