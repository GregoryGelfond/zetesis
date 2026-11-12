//! Cache identity, immutable-prefix reuse and interrupted publication contracts.

use super::{Activity, Bounds, Context, Evaluation};
use crate::expansion::Budget;
use crate::formula_support::testing::Fixture;
use crate::{ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource, ProgramSite};
use zetesis_algebra::{Cancellation, Stop};
use zetesis_ferraris::{FormulaNodes, NodeView};

fn with_context(action: impl FnOnce(&mut Context<'_, '_, '_>)) {
    Fixture::default().with(ProgramSite::program(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
        action(&mut Context {
            computation,
            limits: &limits,
            budget: &mut budget,
            counters,
            location: ProgramSite::program(),
        });
    });
}

fn push(nodes: &mut FormulaNodes, node: NodeView<'_>) -> usize {
    let mut transaction = nodes.transaction();
    let id = transaction.push(node, 4096, 16384).unwrap();
    transaction.commit();
    id
}

fn atoms(count: usize) -> FormulaNodes {
    let mut nodes = FormulaNodes::default();
    for atom in 0..count {
        push(&mut nodes, NodeView::Atom(atom));
    }
    nodes
}

#[test]
fn thresholds_preserve_independent_contribution_bounds() {
    let mut nodes = atoms(3);
    let guards = [-1, 0, 1, 2, 3, 7, 9, 10, 11];
    let roots = nodes
        .push_threshold_family(&[0, 1, 1, 2], &[2, 3, 5, 0], &guards, 4096, 16384)
        .unwrap();
    let states = [Activity::Absent, Activity::Optional, Activity::Required];
    for first in states {
        for second in states {
            for third in states {
                with_context(|context| {
                    let truth = [first, second, third];
                    // The third atom has weight zero; the second owns two
                    // independent tuple positions whose weights sum to eight.
                    let lower = i128::from(first == Activity::Required) * 2
                        + i128::from(second == Activity::Required) * 8;
                    let upper = i128::from(first != Activity::Absent) * 2
                        + i128::from(second != Activity::Absent) * 8;
                    let mut evaluation = Evaluation::new(context).unwrap();
                    for (&root, &bound) in roots.iter().zip(&guards) {
                        let result = evaluation
                            .evaluate_with(nodes.parts(), root, context, |atom, _| Ok(truth[atom]))
                            .unwrap();
                        let expected = if bound <= lower {
                            Activity::Required
                        } else if bound > upper {
                            Activity::Absent
                        } else {
                            Activity::Optional
                        };
                        assert!(
                            result == expected,
                            "bound {bound}, interval {lower}..={upper}"
                        );
                    }
                });
            }
        }
    }
}

#[test]
fn empty_native_rows_keep_the_zero_sum() {
    let mut nodes = FormulaNodes::default();
    let roots = nodes
        .push_threshold_family(&[], &[], &[-1, 0, 1], 4096, 16384)
        .unwrap();
    with_context(|context| {
        let mut evaluation = Evaluation::new(context).unwrap();
        for (&root, expected) in
            roots
                .iter()
                .zip([Activity::Required, Activity::Required, Activity::Absent])
        {
            let result = evaluation
                .evaluate_with(nodes.parts(), root, context, |_, _| {
                    panic!("empty rows have no atom activity")
                })
                .unwrap();
            assert!(result == expected);
        }
    });
}

#[test]
fn native_rows_keep_distinct_conditions_and_weights() {
    let mut nodes = atoms(3);
    let first = nodes
        .push_threshold_family(&[0, 1], &[1, 1], &[2], 4096, 16384)
        .unwrap()[0];
    let second = nodes
        .push_threshold_family(&[0, 1], &[1, 4], &[3], 4096, 16384)
        .unwrap()[0];
    let third = nodes
        .push_threshold_family(&[1, 2], &[1, 4], &[3], 4096, 16384)
        .unwrap()[0];
    with_context(|context| {
        let mut evaluation = Evaluation::new(context).unwrap();
        let truth = [Activity::Required, Activity::Optional, Activity::Absent];
        for (root, expected) in [
            (first, Activity::Optional),
            (second, Activity::Optional),
            (third, Activity::Absent),
        ] {
            let result = evaluation
                .evaluate_with(nodes.parts(), root, context, |atom, _| Ok(truth[atom]))
                .unwrap();
            assert!(result == expected);
        }
    });
}

#[test]
fn later_guards_do_not_rescan_the_native_row() {
    let mut nodes = atoms(1);
    let roots = nodes
        .push_threshold_family(&[0; 1024], &[1; 1024], &[1, 2], 4096, 16384)
        .unwrap();
    with_context(|context| {
        let mut evaluation = Evaluation::new(context).unwrap();
        let result = evaluation
            .evaluate_with(nodes.parts(), roots[0], context, |_, _| {
                Ok(Activity::Optional)
            })
            .unwrap();
        assert!(result == Activity::Optional);
        let before = context.counters.accounting.work;
        let result = evaluation
            .evaluate_with(nodes.parts(), roots[1], context, |_, _| {
                panic!("completed atom prefix is immutable")
            })
            .unwrap();
        assert!(result == Activity::Optional);
        assert!(context.counters.accounting.work - before < 1024);
        let before = context.counters.accounting.work;
        let result = evaluation
            .evaluate_with(nodes.parts(), roots[0], context, |_, _| {
                panic!("an earlier root needs no atom reread")
            })
            .unwrap();
        assert!(result == Activity::Optional);
        assert_eq!(context.counters.accounting.work - before, 1);
    });
}

#[test]
fn appending_storage_preserves_cached_prefix_identity() {
    let mut nodes = atoms(1);
    let first = nodes
        .push_threshold_family(&[0], &[2], &[1], 4096, 16384)
        .unwrap()[0];
    with_context(|context| {
        let mut evaluation = Evaluation::new(context).unwrap();
        let first_result = evaluation
            .evaluate_with(nodes.parts(), first, context, |_, _| Ok(Activity::Required))
            .unwrap();
        assert!(first_result == Activity::Required);
        let first_capacity = nodes.parts().node_capacity();
        let mut last = first;
        for atom in 1..257 {
            last = push(&mut nodes, NodeView::Atom(atom));
        }
        assert!(nodes.parts().node_capacity() > first_capacity);
        let second = nodes
            .push_threshold_family(&[last], &[2], &[1], 4096, 16384)
            .unwrap()[0];
        let result = evaluation
            .evaluate_with(nodes.parts(), second, context, |atom, _| {
                assert_ne!(atom, 0, "old atom activity must not be recomputed");
                Ok(Activity::Absent)
            })
            .unwrap();
        assert!(result == Activity::Absent);
        let result = evaluation
            .evaluate_with(nodes.parts(), first, context, |_, _| {
                panic!("old formula prefix must remain cached")
            })
            .unwrap();
        assert!(result == Activity::Required);
    });
}

#[test]
fn correlated_optional_literals_remain_optional() {
    let mut nodes = atoms(1);
    let falsum = push(&mut nodes, NodeView::False);
    let negative = push(&mut nodes, NodeView::Implies(0, falsum));
    let conjunction = push(&mut nodes, NodeView::And(&[0, negative]));
    let disjunction = push(&mut nodes, NodeView::Or(&[0, negative]));
    with_context(|context| {
        let mut evaluation = Evaluation::new(context).unwrap();
        for root in [conjunction, disjunction] {
            let activity = evaluation
                .evaluate_with(nodes.parts(), root, context, |_, _| Ok(Activity::Optional))
                .unwrap();
            assert!(activity == Activity::Optional);
        }
    });
}

#[test]
fn refused_work_never_publishes_an_uncomputed_value() {
    let mut nodes = atoms(2);
    let root = nodes
        .push_threshold_family(&[0, 1], &[3, 5], &[5], 4096, 16384)
        .unwrap()[0];
    let mut refusals = 0;
    for allowance in 0..40 {
        with_context(|context| {
            let mut evaluation = Evaluation::new(context).unwrap();
            let limits = FormulaLimits {
                max_work: context.counters.accounting.work + allowance,
                ..*context.limits
            };
            let mut limited = Context {
                computation: context.computation,
                limits: &limits,
                budget: context.budget,
                counters: context.counters,
                location: context.location,
            };
            let result = evaluation.evaluate_with(nodes.parts(), root, &mut limited, |_, _| {
                Ok(Activity::Required)
            });
            if let Err(error) = result {
                assert!(matches!(error, FormulaFailure::Limit {
                    resource: FormulaResource::Work, observed, limit, ..
                } if observed > limit));
                refusals += 1;
            }
            let result = evaluation
                .evaluate_with(nodes.parts(), root, context, |_, _| Ok(Activity::Required))
                .unwrap();
            assert!(result == Activity::Required);
            assert_eq!(evaluation.completed, root + 1);
        });
    }
    assert!(refusals > 10, "the sweep must interrupt multiple stages");
}

#[test]
fn cached_activity_observes_cancellation() {
    let nodes = atoms(1);
    with_context(|context| {
        let cancellation = Cancellation::default();
        context.counters.attach_test_control(&cancellation);
        let mut evaluation = Evaluation::new(context).unwrap();
        let result = evaluation
            .evaluate_with(nodes.parts(), 0, context, |_, _| Ok(Activity::Required))
            .unwrap();
        assert!(result == Activity::Required);
        let before = context.counters.accounting.work;
        cancellation.cancel();
        let error = evaluation
            .evaluate_with(nodes.parts(), 0, context, |_, _| {
                panic!("cancelled cached lookup must not read atoms")
            })
            .err()
            .unwrap();
        assert_eq!(error.interruption(), Some(Stop::Cancelled));
        assert_eq!(evaluation.completed, 1);
        assert_eq!(context.counters.accounting.work, before);
    });
}

#[test]
fn cached_rows_do_not_bypass_cancellation() {
    let mut nodes = atoms(1);
    let roots = nodes
        .push_threshold_family(&[0], &[2], &[1, 2], 4096, 16384)
        .unwrap();
    with_context(|context| {
        let cancellation = Cancellation::default();
        context.counters.attach_test_control(&cancellation);
        let mut evaluation = Evaluation::new(context).unwrap();
        evaluation
            .evaluate_with(nodes.parts(), roots[0], context, |_, _| {
                Ok(Activity::Optional)
            })
            .unwrap();
        let before = context.counters.accounting.work;
        cancellation.cancel();
        let error = evaluation
            .evaluate_with(nodes.parts(), roots[1], context, |_, _| {
                panic!("cancellation precedes the cached-row lookup")
            })
            .err()
            .unwrap();
        assert_eq!(error.interruption(), Some(Stop::Cancelled));
        assert_eq!(evaluation.completed, roots[0] + 1);
        assert_eq!(context.counters.accounting.work, before);
    });
}

#[test]
fn cache_storage_is_leased_until_its_owner_drops() {
    let mut nodes = atoms(1);
    let root = nodes
        .push_threshold_family(&[0], &[1], &[1], 4096, 16384)
        .unwrap()[0];
    with_context(|context| {
        let before = context.counters.workspace_bytes();
        let mut evaluation = Evaluation::new(context).unwrap();
        assert_eq!(
            context.counters.workspace_bytes() - before,
            size_of::<Evaluation>()
        );
        evaluation
            .evaluate_with(nodes.parts(), root, context, |_, _| Ok(Activity::Required))
            .unwrap();
        assert_eq!(
            context.counters.workspace_bytes() - before,
            size_of::<Evaluation>()
                + evaluation.values.capacity() * size_of::<Activity>()
                + evaluation.rows.capacity() * size_of::<Option<Bounds>>()
        );
        drop(evaluation);
        assert_eq!(context.counters.workspace_bytes(), before);
    });
}

#[test]
fn refused_storage_keeps_the_completed_prefix() {
    let mut nodes = atoms(1);
    let root = nodes
        .push_threshold_family(&[0], &[1], &[1], 4096, 16384)
        .unwrap()[0];
    with_context(|context| {
        let mut evaluation = Evaluation::new(context).unwrap();
        evaluation
            .evaluate_with(nodes.parts(), 0, context, |_, _| Ok(Activity::Required))
            .unwrap();
        let limits = FormulaLimits {
            max_support_bytes: 1,
            ..*context.limits
        };
        let mut limited = Context {
            computation: context.computation,
            limits: &limits,
            budget: context.budget,
            counters: context.counters,
            location: context.location,
        };
        let result = evaluation.evaluate_with(nodes.parts(), root, &mut limited, |_, _| {
            panic!("storage refusal must precede atom reads")
        });
        assert!(matches!(
            result,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                ..
            })
        ));
        assert_eq!(evaluation.completed, 1);
        let result = evaluation
            .evaluate_with(nodes.parts(), root, context, |_, _| {
                panic!("completed atom prefix is retained after refusal")
            })
            .unwrap();
        assert!(result == Activity::Required);
    });
}
