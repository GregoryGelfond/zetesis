//! Every captured aggregate witness is checked against complete original answers.
//! Cached false-body hints deliberately carry no justification: the collector
//! must reconstruct enough evidence to hold outside the region that supplied it.

use super::{Program, assert_sound, explain, limits, partial};
use crate::regions::Work;
use crate::test_support::{SinglePermits, constructed_theory, push};
use crate::{ExtremumCut, ExtremumCutRule, ExtremumRow, FormulaTransaction, NodeView, Threshold};
use zetesis_cpu::Stop;

#[test]
fn ordinary_closure_reuses_native_aggregate_explanations() {
    use crate::{Knowledge, Narrowing, NarrowingScratch, OriginalSubject, Region, RegionLimits};
    use zetesis_cpu::Cancellation;

    for maximum in [false, true] {
        let (program, _) = program(|nodes| {
            if maximum {
                extremum(nodes, &[2, 3], &[2, 4], ExtremumCutRule::Suffix(2))
            } else {
                threshold(nodes, &[2, 3], &[2, 3], 1)
            }
        });
        let mut scratch = NarrowingScratch::default();
        let close = |region: &mut Region, known: &mut Knowledge, scratch: &mut NarrowingScratch| {
            program
                .index
                .narrow_known(
                    OriginalSubject::new(&program.owner, Some(&program.producers)),
                    region,
                    known,
                    scratch,
                    RegionLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap()
        };
        let mut first = Region::all_open(4);
        assert!(first.cut(2) && first.cut(3));
        let (result, captured) = close(&mut first, &mut program.index.knowledge(), &mut scratch);
        assert!(matches!(result, Narrowing::Fixed { .. }));
        assert_eq!(captured.support_explanations.certified, 1);
        assert_eq!(scratch.explanations.len(), 1);
        let passes = scratch.founded.passes;
        let mut sibling = Region::all_open(4);
        assert!(sibling.hold(0) && sibling.cut(2) && sibling.cut(3));
        let (result, reused) = close(&mut sibling, &mut program.index.knowledge(), &mut scratch);
        assert_eq!(result, Narrowing::Refuted);
        assert_eq!(reused.support_explanations.refuted, 1);
        assert_eq!(reused.support_explanations.attempts, 0);
        assert_eq!(scratch.founded.passes, passes);
    }
}

/// a <-> b; a <- body; choices x,y. Atoms a,b,x,y occupy 0,1,2,3.
fn program(build: impl FnOnce(&mut FormulaTransaction<'_>) -> usize) -> (Program, usize) {
    let mut body = 0;
    let owner = constructed_theory(4, |nodes| {
        let bottom = push(nodes, NodeView::False);
        let mut roots = Vec::new();
        for atom in 2..4 {
            let negative = push(nodes, NodeView::Implies(atom, bottom));
            roots.push(push(nodes, NodeView::Or(&[atom, negative])));
        }
        roots.push(push(nodes, NodeView::Implies(0, 1)));
        roots.push(push(nodes, NodeView::Implies(1, 0)));
        body = build(nodes);
        roots.push(push(nodes, NodeView::Implies(body, 0)));
        roots
    });
    (Program::new(owner), body)
}

fn threshold(
    nodes: &mut FormulaTransaction<'_>,
    conditions: &[usize],
    weights: &[u64],
    bound: i128,
) -> usize {
    push(
        nodes,
        NodeView::Threshold(Threshold {
            conditions,
            weights,
            bound,
        }),
    )
}

fn extremum(
    nodes: &mut FormulaTransaction<'_>,
    conditions: &[usize],
    ranks: &[u64],
    rule: ExtremumCutRule,
) -> usize {
    push(
        nodes,
        NodeView::ExtremumCut(ExtremumCut {
            row: ExtremumRow {
                conditions,
                ranks,
                rank_count: 6,
            },
            rule: &rule,
        }),
    )
}

/// Whole answer families are independent of the collector and include answers
/// outside the region whose false hint supplied a candidate unsupported set.
fn check_all_regions(program: &Program, body: usize) -> (usize, usize) {
    let answers = program.answers();
    let mut head_captures = 0;
    let mut conditional = 0;
    for code in 0..81 {
        let completed = program.complete(&partial(code, 4), &[body]);
        for start in 0..2 {
            if let Some(record) = explain(
                program,
                &completed,
                start,
                limits(),
                &mut Work::new(u64::MAX),
            )
            .unwrap()
            {
                assert_sound(program, &completed, &record, &answers);
                // A direct cut of a can still explain b's absence, even if
                // this producer is constant true. Count a's own captures here.
                head_captures += usize::from(start == 0);
                conditional += usize::from(!record.conditions.is_empty());
            }
        }
    }
    (head_captures, conditional)
}

#[test]
fn threshold_witnesses_hold_in_every_matching_answer() {
    let mut conditional = 0;
    for weights in [
        [0, 0, 0, 0],
        [1, 1, 1, 1],
        [1, 3, 2, 4],
        [u64::MAX, 1, u64::MAX, 0],
    ] {
        let total: i128 = weights.into_iter().map(i128::from).sum();
        for bound in [-1, 0, 1, 2, total, total + 1] {
            let (program, body) = program(|nodes| {
                let bottom = push(nodes, NodeView::False);
                let negative = push(nodes, NodeView::Implies(2, bottom));
                // Repeated x occurrences retain both weights; b may acquire
                // its justification from complete unsupported-set coverage.
                threshold(nodes, &[2, 2, 1, negative], &weights, bound)
            });
            let (captured, with_conditions) = check_all_regions(&program, body);
            conditional += with_conditions;
            if bound <= 0 {
                assert_eq!(captured, 0, "a constant-true producer must block capture");
            } else if bound > total {
                assert!(captured > 0, "an impossible threshold must be explained");
            }
        }
    }
    assert!(
        conditional > 0,
        "the native path must capture conditional reasons"
    );
}

#[test]
fn extremum_witnesses_hold_in_every_matching_answer() {
    let mut conditional = 0;
    for boundary in 0..=6 {
        for rule in [
            ExtremumCutRule::Prefix(boundary),
            ExtremumCutRule::Suffix(boundary),
        ] {
            let (program, body) = program(|nodes| {
                let bottom = push(nodes, NodeView::False);
                let negative = push(nodes, NodeView::Implies(2, bottom));
                extremum(nodes, &[2, 3, 1, negative, 2], &[0, 1, 3, 5, 4], rule)
            });
            let (captured, with_conditions) = check_all_regions(&program, body);
            conditional += with_conditions;
            if matches!(
                rule,
                ExtremumCutRule::Prefix(6) | ExtremumCutRule::Suffix(0)
            ) {
                assert_eq!(captured, 0, "true endpoints need no selected witness");
            }
        }
    }
    assert!(conditional > 0);
}

#[test]
fn unproved_contributions_remain_possible() {
    let (program, body) = program(|nodes| {
        let bottom = push(nodes, NodeView::False);
        let negative = push(nodes, NodeView::Implies(2, bottom));
        let opaque = push(nodes, NodeView::Implies(negative, bottom));
        threshold(nodes, &[1, opaque, 3], &[1, 7, 1], 7)
    });
    let completed = program.complete(&[(0, true), (2, false), (3, false)], &[body]);
    assert!(
        explain(&program, &completed, 0, limits(), &mut Work::new(u64::MAX))
            .unwrap()
            .is_none()
    );
    assert!(
        program.answers().contains(&0b0111),
        "x can support the cycle in another region"
    );
}

#[test]
fn failed_contribution_discards_its_partial_witness() {
    let (program, body) = program(|nodes| {
        let bottom = push(nodes, NodeView::False);
        let negative = push(nodes, NodeView::Implies(2, bottom));
        let opaque = push(nodes, NodeView::Implies(negative, bottom));
        let alternative = push(nodes, NodeView::Or(&[negative, opaque]));
        threshold(nodes, &[alternative, 3], &[1, 2], 2)
    });
    let completed = program.complete(&[(0, true), (2, true), (3, false)], &[body]);
    let record = explain(&program, &completed, 0, limits(), &mut Work::new(u64::MAX))
        .unwrap()
        .unwrap();
    assert_eq!(record.conditions, [(3, false)]);
    assert_sound(&program, &completed, &record, &program.answers());
}

#[test]
fn failed_middle_contribution_preserves_only_earlier_evidence() {
    let mut body = 0;
    let owner = constructed_theory(5, |nodes| {
        let bottom = push(nodes, NodeView::False);
        let mut roots = Vec::new();
        let mut negative = Vec::new();
        for atom in 2..5 {
            let absent = push(nodes, NodeView::Implies(atom, bottom));
            negative.push(absent);
            roots.push(push(nodes, NodeView::Or(&[atom, absent])));
        }
        roots.push(push(nodes, NodeView::Implies(0, 0)));
        roots.push(push(nodes, NodeView::Implies(1, 1)));
        let opaque = push(nodes, NodeView::Implies(negative[1], bottom));
        let alternative = push(nodes, NodeView::Or(&[1, negative[1], opaque]));
        body = threshold(
            nodes,
            &[negative[0], alternative, negative[2]],
            &[1, 1, 2],
            2,
        );
        roots.push(push(nodes, NodeView::Implies(body, 0)));
        roots
    });
    let program = Program::new(owner);
    let completed = program.complete(&[(2, true), (3, true), (4, true)], &[body]);
    let record = explain(&program, &completed, 0, limits(), &mut Work::new(u64::MAX))
        .unwrap()
        .unwrap();
    assert_eq!(record.conditions, [(2, true), (4, true)]);
    assert_eq!(record.unsupported, [0]);
    assert_sound(&program, &completed, &record, &program.answers());
}

#[test]
fn nonqualifying_extremum_conditions_are_not_premises() {
    let (program, body) =
        program(|nodes| extremum(nodes, &[2, 3], &[1, 4], ExtremumCutRule::Prefix(2)));
    let completed = program.complete(&[(0, true), (2, false)], &[body]);
    let record = explain(&program, &completed, 0, limits(), &mut Work::new(u64::MAX))
        .unwrap()
        .unwrap();
    assert_eq!(record.conditions, [(2, false)]);
    assert_sound(&program, &completed, &record, &program.answers());
}

#[test]
fn empty_numeric_witnesses_are_handled_as_constants() {
    for rule in [ExtremumCutRule::Prefix(2), ExtremumCutRule::Suffix(4)] {
        let (program, body) = program(|nodes| extremum(nodes, &[], &[], rule));
        let completed = program.complete(&[], &[body]);
        let record = explain(&program, &completed, 0, limits(), &mut Work::new(u64::MAX))
            .unwrap()
            .unwrap();
        assert!(record.conditions.is_empty());
        assert_sound(&program, &completed, &record, &program.answers());
    }
    for bound in [0, 1] {
        let (program, body) = program(|nodes| threshold(nodes, &[], &[], bound));
        let completed = program.complete(&[], &[body]);
        let record = explain(&program, &completed, 0, limits(), &mut Work::new(u64::MAX)).unwrap();
        assert_eq!(record.is_some(), bound > 0);
    }
}

#[test]
fn nonqualifying_rows_preserve_every_interruption() {
    for (ranks, rule) in [
        ([4, 5], ExtremumCutRule::Prefix(2)),
        ([0, 1], ExtremumCutRule::Suffix(4)),
    ] {
        let (program, body) = program(|nodes| extremum(nodes, &[2, 3], &ranks, rule));
        assert!(program.index.chain_of[body].is_none());
        let completed = program.complete(&[], &[body]);
        let mut measured = Work::new(u64::MAX);
        let record = explain(&program, &completed, 0, limits(), &mut measured)
            .unwrap()
            .unwrap();
        assert!(record.conditions.is_empty());
        assert_sound(&program, &completed, &record, &program.answers());
        for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
            for prefix in 0..measured.spent {
                let mut quota = SinglePermits::new(prefix, failure);
                let mut work = Work::reserved(&mut quota);
                assert!(
                    matches!(explain(&program, &completed, 0, limits(), &mut work), Err(actual) if actual == failure)
                );
                assert_eq!(work.spent, prefix);
            }
        }
    }
}

fn nested_program() -> (Program, usize) {
    program(|nodes| {
        let inner = threshold(nodes, &[2, 1], &[2, 1], 2);
        let cut = extremum(nodes, &[inner, 3], &[1, 4], ExtremumCutRule::Prefix(2));
        threshold(nodes, &[cut, 3], &[3, 1], 3)
    })
}

#[test]
fn nested_numeric_witnesses_preserve_every_interruption() {
    let (program, body) = nested_program();
    let completed = program.complete(&[(2, false), (3, false)], &[body]);
    let mut measured = Work::new(u64::MAX);
    let record = explain(&program, &completed, 0, limits(), &mut measured)
        .unwrap()
        .unwrap();
    assert_sound(&program, &completed, &record, &program.answers());
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for prefix in 0..measured.spent {
            let mut quota = SinglePermits::new(prefix, failure);
            let mut work = Work::reserved(&mut quota);
            assert!(
                matches!(explain(&program, &completed, 0, limits(), &mut work), Err(actual) if actual == failure)
            );
            assert_eq!(work.spent, prefix);
        }
    }
}

#[test]
fn local_numeric_capture_limits_publish_no_partial_reason() {
    let (program, body) = nested_program();
    let completed = program.complete(&[(2, false), (3, false)], &[body]);
    for limited in [
        super::Limits {
            atoms: 0,
            ..limits()
        },
        super::Limits {
            literals: 0,
            ..limits()
        },
        super::Limits {
            visits: 0,
            ..limits()
        },
        super::Limits {
            bytes: 0,
            ..limits()
        },
    ] {
        assert!(
            explain(&program, &completed, 0, limited, &mut Work::new(u64::MAX))
                .unwrap()
                .is_none()
        );
    }
}
