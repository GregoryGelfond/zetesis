use super::NumericRowScratch;
use crate::{
    AdmissionLimits, EvaluationError, EvaluationLimits, EvaluationWorkspace, ExtremaMeasure,
    ExtremumCut, ExtremumCutDescriptor, ExtremumCutRule, ExtremumRow, FormulaNodes, FormulaParts,
    FrozenReduct, Interpretation, Limits, Node, NodeView, NumericDomain, NumericRow, Theory,
    ThresholdDescriptor, TightCheckLimits, TightPlan, TightPlanLimits, TightVerdict,
    TightWorkspace, check, models_reduct, native_aggregate as native,
};
use zetesis_core::Value;
use zetesis_cpu::{Cancellation, Stop};

fn family(roots: Vec<usize>, reverse: bool) -> Theory {
    let mut nodes = FormulaNodes::default();
    let mut transaction = nodes.transaction();
    for atom in 0..2 {
        transaction.push(NodeView::Atom(atom), 10, 100).unwrap();
    }
    let ranks = if reverse { [4, 0, 3] } else { [0, 4, 3] };
    transaction
        .push_extremum_cut_family_range(
            ExtremumRow {
                conditions: &[0, 1, 0],
                ranks: &ranks,
                rank_count: 5,
            },
            &[
                ExtremumCutRule::Prefix(1),
                ExtremumCutRule::Suffix(4),
                ExtremumCutRule::Prefix(5),
            ],
            10,
            100,
        )
        .unwrap();
    transaction.commit();
    Theory::new(2, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap()
}

fn world(theory: &Theory, mask: usize) -> Interpretation {
    Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|atom| mask & (1 << atom) != 0),
    )
    .unwrap()
}

fn view(theory: &Theory, node: usize) -> ExtremumCut<'_> {
    let NodeView::ExtremumCut(cut) = theory.view().node(node).unwrap() else {
        panic!("cut fixture");
    };
    cut
}

fn raw(rows: usize, cuts: bool) -> Theory {
    let mut spans = vec![
        NumericRow {
            start: 0,
            length: 0,
            domain: NumericDomain::Weights
        };
        rows
    ];
    let mut nodes = vec![Node::atom(0)];
    let mut descriptors = vec![];
    if cuts {
        spans[rows - 1] = NumericRow {
            start: 0,
            length: 1,
            domain: NumericDomain::Ordinal { rank_count: 5 },
        };
        nodes.push(Node::extremum_cut(0));
        descriptors.push(ExtremumCutDescriptor {
            row: rows - 1,
            rule: ExtremumCutRule::Prefix(2),
        });
    }
    let (conditions, values) = if cuts {
        (vec![0], vec![1])
    } else {
        (vec![], vec![])
    };
    let parts = FormulaParts::from_numeric_raw(
        nodes,
        vec![],
        conditions,
        values,
        spans,
        vec![],
        descriptors,
    )
    .unwrap();
    Theory::new(1, parts, vec![], AdmissionLimits::default()).unwrap()
}

#[test]
fn shared_cuts_reduce_once_per_world() {
    let owner = family(vec![], false);
    let mut workspace = EvaluationWorkspace::default();
    for (ordinal, mask) in [3, 0, 1, 2, 3].into_iter().enumerate() {
        let candidate = world(&owner, mask);
        let attempt = workspace.evaluate(
            &candidate,
            EvaluationLimits::default(),
            &Cancellation::default(),
        );
        // Five nodes, one row of three occurrences, one validity reset;
        // only the first use initializes its one safe measure cell.
        assert_eq!(attempt.work, 9 + u64::from(ordinal == 0));
        let truth = attempt.result.unwrap();
        assert_eq!(truth.node_truth(2), Some(mask & 1 != 0));
        assert_eq!(truth.node_truth(3), Some(mask & 2 != 0));
        assert_eq!(truth.node_truth(4), Some(true));
    }
}

#[test]
fn equal_rows_keep_distinct_authoritative_slots() {
    let mut nodes = FormulaNodes::default();
    let mut transaction = nodes.transaction();
    transaction.push(NodeView::Atom(0), 3, 2).unwrap();
    for _ in 0..2 {
        transaction
            .push_extremum_cut_family_range(
                ExtremumRow {
                    conditions: &[0],
                    ranks: &[1],
                    rank_count: 3,
                },
                &[ExtremumCutRule::Prefix(2)],
                3,
                2,
            )
            .unwrap();
    }
    transaction.commit();
    let owner = Theory::new(1, nodes.into_parts(), vec![], AdmissionLimits::default()).unwrap();
    let mut workspace = EvaluationWorkspace::default();
    let candidate = world(&owner, 1);
    let attempt = workspace.evaluate(
        &candidate,
        EvaluationLimits::default(),
        &Cancellation::default(),
    );
    assert!(attempt.result.is_ok());
    // Three nodes, two row reads, two initializations, one reset word.
    assert_eq!(attempt.work, 8);
}

#[test]
fn workspace_reuse_cannot_cross_theory_authority() {
    let left = family(vec![], false);
    let right = family(vec![], true);
    let mut workspace = EvaluationWorkspace::default();
    for (owner, expected) in [(&left, true), (&right, false), (&left, true)] {
        let candidate = world(owner, 1);
        let attempt = workspace.evaluate(
            &candidate,
            EvaluationLimits::default(),
            &Cancellation::default(),
        );
        assert_eq!(attempt.result.unwrap().node_truth(2), Some(expected));
    }
}

#[test]
fn interrupted_passes_do_not_retain_truth() {
    let owner = family(vec![], false);
    for max_work in 0..10 {
        let mut workspace = EvaluationWorkspace::default();
        let candidate = world(&owner, 3);
        let attempt = workspace.evaluate(
            &candidate,
            EvaluationLimits {
                max_work,
                ..EvaluationLimits::default()
            },
            &Cancellation::default(),
        );
        assert_eq!(
            attempt.result.unwrap_err(),
            EvaluationError::Stopped(Stop::WorkLimit)
        );
        assert_eq!(attempt.work, max_work);
        let next = world(&owner, 0);
        let result = workspace
            .evaluate(&next, EvaluationLimits::default(), &Cancellation::default())
            .result
            .unwrap();
        assert_eq!(result.node_truth(2), Some(false));
        assert_eq!(result.node_truth(3), Some(false));
        assert_eq!(result.node_truth(4), Some(true));
    }
}

#[test]
fn failed_reductions_publish_no_valid_bit() {
    let owner = family(vec![], false);
    let mut scratch = NumericRowScratch::default();
    scratch.reserve(&owner).unwrap();
    scratch.reset(&owner, || Ok::<_, Stop>(())).unwrap();
    let mut reads = 0;
    let result = scratch.extremum(&owner, 2, view(&owner, 2), |_| {
        reads += 1;
        if reads == 2 {
            Err(Stop::Cancelled)
        } else {
            Ok(true)
        }
    });
    assert_eq!(result, Err(Stop::Cancelled));
    assert_eq!(scratch.valid, [0]);
    reads = 0;
    let measure = scratch
        .extremum(&owner, 2, view(&owner, 2), |_| {
            reads += 1;
            Ok::<_, Stop>(false)
        })
        .unwrap();
    assert_eq!(reads, 3);
    assert_eq!(
        measure,
        ExtremaMeasure {
            minimum: 4,
            maximum: 0
        }
    );
    assert_eq!(scratch.valid, [1]);
    let cached = scratch
        .extremum(&owner, 3, view(&owner, 3), |_| Err(Stop::Cancelled))
        .unwrap();
    assert_eq!(cached, measure);
}

#[test]
fn packed_resets_preserve_zero_tail_bits() {
    let owner = raw(130, true);
    let mut scratch = NumericRowScratch::default();
    scratch.reserve(&owner).unwrap();
    let mut ticks = 0;
    scratch
        .reset(&owner, || {
            ticks += 1;
            Ok::<_, Stop>(())
        })
        .unwrap();
    assert_eq!(ticks, 133);
    assert_eq!(scratch.valid, [0, 0, 0]);
    scratch
        .extremum(&owner, 1, view(&owner, 1), |_| Ok::<_, Stop>(true))
        .unwrap();
    assert_eq!(scratch.valid, [0, 0, 2]);
    ticks = 0;
    scratch
        .reset(&owner, || {
            ticks += 1;
            Ok::<_, Stop>(())
        })
        .unwrap();
    assert_eq!(ticks, 3);
    assert_eq!(scratch.valid, [0, 0, 0]);
    assert_eq!(scratch.measures.len(), 130);
    assert!(scratch.retained_bytes() >= 130 * 16 + 3 * 8);
}

#[test]
fn interrupted_growth_only_retains_safe_cells() {
    let owner = raw(130, true);
    let mut scratch = NumericRowScratch::default();
    scratch.reserve(&owner).unwrap();
    let mut ticks = 0;
    let result = scratch.reset(&owner, || {
        if ticks == 8 {
            return Err(Stop::WorkLimit);
        }
        ticks += 1;
        Ok(())
    });
    assert_eq!(result, Err(Stop::WorkLimit));
    assert_eq!(scratch.measures.len(), 5);
    assert_eq!(scratch.valid, [0, 0, 0]);
    ticks = 0;
    scratch
        .reset(&owner, || {
            ticks += 1;
            Ok::<_, Stop>(())
        })
        .unwrap();
    assert_eq!(ticks, 128);
    assert_eq!(scratch.measures.len(), 130);
}

#[test]
fn unused_weight_rows_allocate_no_measure_columns() {
    let owner = raw(130, false);
    let mut scratch = NumericRowScratch::default();
    assert_eq!(scratch.required_bytes(&owner), 0);
    scratch.reserve(&owner).unwrap();
    let mut ticks = 0;
    scratch
        .reset(&owner, || {
            ticks += 1;
            Ok::<_, Stop>(())
        })
        .unwrap();
    assert_eq!(ticks, 0);
    assert_eq!(scratch.retained_bytes(), 0);
    let mut workspace = EvaluationWorkspace::default();
    let candidate = world(&owner, 0);
    let attempt = workspace.evaluate(
        &candidate,
        EvaluationLimits::default(),
        &Cancellation::default(),
    );
    assert!(attempt.result.is_ok());
    assert_eq!(attempt.work, 1);
}

#[test]
fn row_storage_is_admitted_before_any_work() {
    let owner = raw(130, true);
    let mut workspace = EvaluationWorkspace::default();
    let required = size_of::<EvaluationWorkspace>()
        + owner.nodes().len()
        + 130 * size_of::<ExtremaMeasure>()
        + 3 * size_of::<u64>();
    let candidate = world(&owner, 1);
    let attempt = workspace.evaluate(
        &candidate,
        EvaluationLimits {
            max_bytes: required - 1,
            ..EvaluationLimits::default()
        },
        &Cancellation::default(),
    );
    assert_eq!(
        attempt.result.unwrap_err(),
        EvaluationError::Storage {
            required: required as u128,
            limit: required - 1
        }
    );
    assert_eq!(attempt.work, 0);
    assert_eq!(
        workspace.retained_bytes(),
        size_of::<EvaluationWorkspace>() as u128
    );
    assert!(
        workspace
            .evaluate(
                &candidate,
                EvaluationLimits::default(),
                &Cancellation::default()
            )
            .result
            .is_ok()
    );
    let retained = usize::try_from(workspace.retained_bytes()).unwrap();
    assert!(
        workspace
            .evaluate(
                &candidate,
                EvaluationLimits {
                    max_bytes: retained,
                    ..EvaluationLimits::default()
                },
                &Cancellation::default()
            )
            .result
            .is_ok()
    );
    let smaller = raw(0, false);
    let smaller_world = world(&smaller, 0);
    let refused = workspace.evaluate(
        &smaller_world,
        EvaluationLimits {
            max_bytes: retained - 1,
            ..EvaluationLimits::default()
        },
        &Cancellation::default(),
    );
    assert!(matches!(
        refused.result,
        Err(EvaluationError::Storage { .. })
    ));
    assert_eq!(refused.work, 0);
}

#[test]
fn frozen_queries_reset_shared_measures_for_every_world() {
    let owner = family(vec![2, 3], false);
    for outer in 0..4 {
        let candidate = world(&owner, outer);
        let reduct =
            FrozenReduct::new(&candidate, Limits::default(), &Cancellation::default()).unwrap();
        for inner in 0..4 {
            let tested = world(&owner, inner);
            let expected = outer == 3 && inner == 3;
            assert_eq!(
                reduct
                    .is_satisfied_by(&tested, Limits::default(), &Cancellation::default())
                    .unwrap(),
                expected
            );
            assert_eq!(
                models_reduct(
                    &owner,
                    &candidate,
                    &tested,
                    Limits::default(),
                    &Cancellation::default()
                )
                .unwrap(),
                expected
            );
        }
        let checked = check(
            &owner,
            &candidate,
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(checked.accepted(), outer == 3);
        if outer == 3 {
            assert_eq!(checked.statistics().subsets, 3);
        }
    }
}

#[test]
fn tight_checks_reuse_only_initialized_storage() {
    let owner = family(vec![], false);
    let mut nodes = FormulaNodes::new(owner.parts().try_clone().unwrap());
    let mut transaction = nodes.transaction();
    let root = transaction.push(NodeView::Implies(2, 1), 6, 20).unwrap();
    transaction.commit();
    let owner = Theory::new(
        2,
        nodes.into_parts(),
        vec![0, root],
        AdmissionLimits::default(),
    )
    .unwrap();
    let plan =
        TightPlan::compile(&owner, TightPlanLimits::default(), &Cancellation::default()).unwrap();
    let mut workspace = TightWorkspace::default();
    let full = world(&owner, 3);
    let first = workspace
        .check(
            &plan,
            &full,
            TightCheckLimits::default(),
            &Cancellation::default(),
        )
        .result
        .unwrap();
    let second = workspace
        .check(
            &plan,
            &full,
            TightCheckLimits::default(),
            &Cancellation::default(),
        )
        .result
        .unwrap();
    assert_eq!(first.verdict, TightVerdict::Stable);
    assert_eq!(first.work, second.work + 1);
    assert_eq!(first.logical_bytes, second.logical_bytes);
    assert!(workspace.retained_bytes() >= 6 + 4 + 16 + 8);
    let empty = world(&owner, 0);
    assert_ne!(
        workspace
            .check(
                &plan,
                &empty,
                TightCheckLimits::default(),
                &Cancellation::default()
            )
            .result
            .unwrap()
            .verdict,
        TightVerdict::Stable
    );
}

#[test]
fn eligibility_accounts_one_cache_for_both_phases() {
    let owner = family(vec![], false);
    let group = native::Group::new(
        &owner,
        native::Function::Count,
        (2..5)
            .map(|condition| native::Tuple {
                key: vec![Value::Number(i32::try_from(condition).unwrap())],
                condition,
            })
            .collect(),
        vec![],
        native::AdmissionLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let candidate = world(&owner, 3);
    let tested = world(&owner, 0);
    let eligibility = group
        .eligibility(
            &candidate,
            Some(&tested),
            native::EligibilityLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(eligibility.original(), &[true, true, true]);
    assert_eq!(eligibility.frozen(), Some(&[false, false, true][..]));
    let statistics = eligibility.statistics();
    assert_eq!(statistics.work, 25);
    assert_eq!(statistics.peak_bytes, 40);
    assert_eq!(statistics.resident_bytes, 6);
    assert!(statistics.storage_peak_bytes >= statistics.peak_bytes);
    assert!(statistics.storage_bytes >= statistics.resident_bytes);
    for max_work in 0..25 {
        let refused = group
            .eligibility(
                &candidate,
                Some(&tested),
                native::EligibilityLimits {
                    max_work,
                    ..native::EligibilityLimits::default()
                },
                &Cancellation::default(),
            )
            .unwrap_err();
        assert_eq!(
            refused.kind(),
            native::ErrorKind::Limit(native::Resource::Work)
        );
        assert_eq!(refused.statistics().work, max_work);
        assert!(refused.statistics().storage_peak_bytes >= 40);
    }
    let refused = group
        .eligibility(
            &candidate,
            Some(&tested),
            native::EligibilityLimits {
                max_bytes: 39,
                ..native::EligibilityLimits::default()
            },
            &Cancellation::default(),
        )
        .unwrap_err();
    assert_eq!(
        refused.kind(),
        native::ErrorKind::Limit(native::Resource::Bytes)
    );
    assert_eq!(refused.statistics().work, 0);
    assert_eq!(refused.statistics().storage_peak_bytes, 0);
}

#[test]
fn observation_refusal_precedes_the_second_column() {
    let owner = raw(130, true);
    let mut scratch = NumericRowScratch::default();
    let mut observations = Vec::new();
    let result = scratch.reserve_observed(&owner, |actual| {
        observations.push(actual);
        Err(Stop::Cancelled)
    });
    assert_eq!(result, Err(Stop::Cancelled));
    assert_eq!(observations.len(), 1);
    assert!(observations[0] >= 130 * size_of::<ExtremaMeasure>() as u128);
    assert_eq!(scratch.valid.capacity(), 0);
    assert!(scratch.measures.is_empty());
    assert_eq!(scratch.retained_bytes(), observations[0]);
}

#[test]
fn interleaved_cuts_use_their_original_row_slot() {
    // Row 1 depends on row 0's first cut. The later row-0 cut must use its own
    // completed measure, never the most recently reduced row's measure.
    let parts = FormulaParts::from_numeric_raw(
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::extremum_cut(0),
            Node::extremum_cut(1),
            Node::extremum_cut(2),
        ],
        vec![],
        vec![0, 1, 2],
        vec![0, 4, 1],
        vec![
            NumericRow {
                start: 0,
                length: 2,
                domain: NumericDomain::Ordinal { rank_count: 5 },
            },
            NumericRow {
                start: 2,
                length: 1,
                domain: NumericDomain::Ordinal { rank_count: 5 },
            },
        ],
        vec![],
        vec![
            ExtremumCutDescriptor {
                row: 0,
                rule: ExtremumCutRule::Prefix(1),
            },
            ExtremumCutDescriptor {
                row: 1,
                rule: ExtremumCutRule::Suffix(1),
            },
            ExtremumCutDescriptor {
                row: 0,
                rule: ExtremumCutRule::Suffix(4),
            },
        ],
    )
    .unwrap();
    let owner = Theory::new(2, parts, vec![], AdmissionLimits::default()).unwrap();
    let mut workspace = EvaluationWorkspace::default();
    for mask in 0..4 {
        let candidate = world(&owner, mask);
        let attempt = workspace.evaluate(
            &candidate,
            EvaluationLimits::default(),
            &Cancellation::default(),
        );
        let truth = attempt.result.unwrap();
        assert_eq!(truth.node_truth(2), Some(mask & 1 != 0));
        assert_eq!(truth.node_truth(3), Some(mask & 1 != 0));
        assert_eq!(truth.node_truth(4), Some(mask & 2 != 0));
    }
}

#[test]
fn evaluation_bound_covers_fresh_and_reused_rows() {
    for rows in [1_usize, 2, 64, 65, 130] {
        let owner = raw(rows, true);
        let candidate = world(&owner, 1);
        let bound = u64::try_from(EvaluationWorkspace::work_bound(&owner)).unwrap();
        // Two nodes, one occurrence, all dense row cells and validity words.
        assert_eq!(bound, u64::try_from(3 + rows + rows.div_ceil(64)).unwrap());
        let mut fresh = EvaluationWorkspace::default();
        let refused = fresh.evaluate(
            &candidate,
            EvaluationLimits {
                max_work: bound - 1,
                ..EvaluationLimits::default()
            },
            &Cancellation::default(),
        );
        assert_eq!(refused.work, bound - 1);
        assert_eq!(
            refused.result.unwrap_err(),
            EvaluationError::Stopped(Stop::WorkLimit)
        );
        let mut workspace = EvaluationWorkspace::default();
        for expected in [bound, bound - u64::try_from(rows).unwrap()] {
            let attempt = workspace.evaluate(
                &candidate,
                EvaluationLimits {
                    max_work: bound,
                    ..EvaluationLimits::default()
                },
                &Cancellation::default(),
            );
            assert!(attempt.result.unwrap().is_model());
            assert_eq!(attempt.work, expected);
        }
        // Retained larger cache columns add no work for an owner without numeric nodes.
        let plain = raw(200, false);
        assert_eq!(EvaluationWorkspace::work_bound(&plain), 1);
        let plain_candidate = world(&plain, 0);
        let attempt = workspace.evaluate(
            &plain_candidate,
            EvaluationLimits {
                max_work: 1,
                ..EvaluationLimits::default()
            },
            &Cancellation::default(),
        );
        assert!(attempt.result.unwrap().is_model());
        assert_eq!(attempt.work, 1);
    }
}

#[test]
fn unused_numeric_descriptors_do_not_request_evaluation_slots() {
    let mut rows = vec![
        NumericRow {
            start: 0,
            length: 0,
            domain: NumericDomain::Ordinal { rank_count: 3 },
        };
        65
    ];
    rows[0].domain = NumericDomain::Weights;
    let parts = FormulaParts::from_numeric_raw(
        vec![Node::atom(0)],
        vec![],
        vec![],
        vec![],
        rows,
        vec![ThresholdDescriptor { row: 0, bound: 1 }],
        vec![ExtremumCutDescriptor {
            row: 64,
            rule: ExtremumCutRule::Prefix(2),
        }],
    )
    .unwrap();
    let owner = Theory::new(1, parts, vec![0], AdmissionLimits::default()).unwrap();
    let candidate = world(&owner, 1);
    let bound = u64::try_from(EvaluationWorkspace::work_bound(&owner)).unwrap();
    assert_eq!(bound, 2);
    assert_eq!(NumericRowScratch::requested_bytes(&owner), 0);
    assert_eq!(NumericRowScratch::reset_work_bound(&owner), 0);
    let mut workspace = EvaluationWorkspace::default();
    let attempt = workspace.evaluate(
        &candidate,
        EvaluationLimits {
            max_work: bound,
            ..EvaluationLimits::default()
        },
        &Cancellation::default(),
    );
    assert!(attempt.result.unwrap().is_model());
    assert_eq!(attempt.work, bound);
}
