use super::*;
use crate::{ExtremumCutRule, ExtremumRow, FormulaNodes};

fn owner() -> FormulaNodes {
    FormulaNodes::new(FormulaParts::new(vec![Node::atom(0), Node::atom(1)], vec![]).unwrap())
}

fn row<'a>(conditions: &'a [usize], ranks: &'a [u64]) -> ExtremumRow<'a> {
    ExtremumRow {
        conditions,
        ranks,
        rank_count: 4,
    }
}

fn raw(domain: NumericDomain, values: Vec<u64>, cuts: Vec<ExtremumCutDescriptor>) -> FormulaParts {
    let width = values.len();
    FormulaParts::from_numeric_raw(
        vec![Node::atom(0)],
        vec![],
        vec![0; width],
        values,
        vec![NumericRow {
            start: 0,
            length: width,
            domain,
        }],
        vec![],
        cuts,
    )
    .unwrap()
}

fn admit(parts: FormulaParts) -> Result<Theory, AdmissionError> {
    Theory::new(2, parts, vec![], AdmissionLimits::default())
}

#[test]
fn ordinal_cuts_keep_existing_hot_view_layouts() {
    assert_eq!(
        std::mem::size_of::<Node>(),
        3 * std::mem::size_of::<usize>()
    );
    assert!(std::mem::size_of::<ExtremumCut<'_>>() <= std::mem::size_of::<Threshold<'_>>());
    if std::mem::size_of::<usize>() == 8 {
        assert_eq!(std::mem::size_of::<NodeView<'_>>(), 64);
        assert_eq!(std::mem::size_of::<FormulaView<'_>>(), 40);
    }
}

#[test]
fn mixed_families_share_columns_and_keep_domains() {
    let mut nodes = owner();
    let mut transaction = nodes.transaction();
    transaction
        .push_threshold_family_range(&[0], &[7], &[3], 10, 20)
        .unwrap();
    let cuts = [
        ExtremumCutRule::Prefix(2),
        ExtremumCutRule::Suffix(1),
        ExtremumCutRule::Prefix(2),
    ];
    assert_eq!(
        transaction
            .push_extremum_cut_family_range(row(&[0, 0, 1], &[1, 3, 0]), &cuts, 10, 20)
            .unwrap(),
        3..6
    );
    transaction.commit();
    let parts = nodes.parts();
    assert_eq!(parts.numeric_values(), &[7, 1, 3, 0]);
    assert_eq!(parts.conditions(), &[0, 0, 0, 1]);
    assert_eq!(parts.rows().len(), 2);
    assert_eq!(parts.rows()[0].domain, NumericDomain::Weights);
    assert_eq!(
        parts.rows()[1].domain,
        NumericDomain::Ordinal { rank_count: 4 }
    );
    assert_eq!(
        parts
            .extremum_cuts()
            .iter()
            .map(|cut| cut.row)
            .collect::<Vec<_>>(),
        [1, 1, 1]
    );
    assert_eq!(parts.occurrences(), 10);
    assert_eq!(parts.view().node(3), parts.view().node(5));
}

#[test]
fn endpoint_and_empty_families_still_validate_every_child() {
    let mut nodes = owner();
    for cuts in [
        &[][..],
        &[ExtremumCutRule::Prefix(0)][..],
        &[ExtremumCutRule::Suffix(0)][..],
    ] {
        let mut transaction = nodes.transaction();
        assert_eq!(
            transaction
                .push_extremum_cut_family_range(row(&[2], &[0]), cuts, 3, 1)
                .unwrap_err(),
            AdmissionError::Edge
        );
        assert!(transaction.parts().rows().is_empty());
    }
}

#[test]
fn ordinal_append_refuses_each_invalid_shape() {
    let mut nodes = owner();
    let cases = [
        (
            row(&[0], &[]),
            ExtremumCutRule::Prefix(1),
            AdmissionError::NumericShape,
        ),
        (
            row(&[0], &[4]),
            ExtremumCutRule::Prefix(1),
            AdmissionError::OrdinalRank,
        ),
        (
            ExtremumRow {
                rank_count: 1,
                ..row(&[], &[])
            },
            ExtremumCutRule::Prefix(0),
            AdmissionError::OrdinalRank,
        ),
        (
            row(&[], &[]),
            ExtremumCutRule::Suffix(5),
            AdmissionError::OrdinalCut,
        ),
    ];
    for (input, cut, expected) in cases {
        let mut transaction = nodes.transaction();
        assert_eq!(
            transaction
                .push_extremum_cut_family_range(input, &[cut], 3, 2)
                .unwrap_err(),
            expected
        );
        assert_eq!(transaction.appended_cells(), 0);
    }
}

#[test]
fn maximum_carrier_and_boundary_need_no_successor() {
    let mut nodes = owner();
    let mut transaction = nodes.transaction();
    let input = ExtremumRow {
        conditions: &[0],
        ranks: &[u64::MAX - 1],
        rank_count: u64::MAX,
    };
    transaction
        .push_extremum_cut_family_range(input, &[ExtremumCutRule::Prefix(u64::MAX)], 3, 1)
        .unwrap();
    transaction.commit();
    assert!(admit(nodes.into_parts()).is_ok());
}

#[test]
fn cut_descriptors_require_ordinal_domains() {
    let parts = raw(
        NumericDomain::Weights,
        vec![0],
        vec![ExtremumCutDescriptor {
            row: 0,
            rule: ExtremumCutRule::Prefix(1),
        }],
    );
    assert_eq!(admit(parts).unwrap_err(), AdmissionError::NumericDomain);
}

#[test]
fn threshold_descriptors_require_weight_domains() {
    let mut parts = raw(NumericDomain::Ordinal { rank_count: 4 }, vec![1], vec![]);
    parts
        .numeric
        .thresholds
        .push(ThresholdDescriptor { row: 0, bound: 0 });
    assert_eq!(admit(parts).unwrap_err(), AdmissionError::NumericDomain);
}

#[test]
fn unused_ordinal_rows_and_cuts_are_validated() {
    for (rank_count, value) in [(0, 0), (1, 0), (4, 4)] {
        assert_eq!(
            admit(raw(
                NumericDomain::Ordinal { rank_count },
                vec![value],
                vec![]
            ))
            .unwrap_err(),
            AdmissionError::OrdinalRank
        );
    }
    let parts = raw(
        NumericDomain::Ordinal { rank_count: 4 },
        vec![],
        vec![ExtremumCutDescriptor {
            row: 0,
            rule: ExtremumCutRule::Suffix(5),
        }],
    );
    assert_eq!(admit(parts).unwrap_err(), AdmissionError::OrdinalCut);
}

#[test]
fn numeric_growth_cannot_repair_a_raw_cut_reference() {
    let parts = FormulaParts::from_numeric_raw(
        vec![Node::atom(0), Node::extremum_cut(0)],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let mut nodes = FormulaNodes::new(parts);
    let mut transaction = nodes.transaction();
    assert_eq!(
        transaction
            .push_extremum_cut_family_range(row(&[0], &[1]), &[ExtremumCutRule::Prefix(2)], 3, 1)
            .unwrap_err(),
        AdmissionError::Span
    );
    assert_eq!(transaction.appended_cells(), 0);
}

#[test]
fn threshold_growth_cannot_repair_a_raw_cut_reference() {
    let parts = FormulaParts::from_numeric_raw(
        vec![Node::atom(0), Node::extremum_cut(0)],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let mut nodes = FormulaNodes::new(parts);
    let mut transaction = nodes.transaction();
    assert_eq!(
        transaction
            .push_threshold_family_range(&[0], &[1], &[1], 3, 1)
            .unwrap_err(),
        AdmissionError::Span
    );
    assert_eq!(transaction.appended_cells(), 0);
}

#[test]
fn ordinal_occurrence_refusal_keeps_every_arena() {
    let mut nodes = owner();
    let mut transaction = nodes.transaction();
    transaction.push(NodeView::And(&[0, 1, 0]), 6, 6).unwrap();
    let before = transaction.appended_payload_bytes();
    assert_eq!(
        transaction
            .push_extremum_cut_family_range(
                row(&[0, 1], &[1, 2]),
                &[ExtremumCutRule::Prefix(2), ExtremumCutRule::Suffix(2)],
                6,
                6
            )
            .unwrap_err(),
        AdmissionError::Limit
    );
    assert_eq!(transaction.appended_payload_bytes(), before);
    assert!(transaction.parts().extremum_cuts().is_empty());
}

#[test]
fn nested_cut_detachment_owns_only_new_rows() {
    let mut nodes = owner();
    let mut outer = nodes.transaction();
    outer
        .push_extremum_cut_family_range(row(&[0], &[1]), &[ExtremumCutRule::Prefix(2)], 10, 20)
        .unwrap();
    let mut inner = outer.transaction();
    inner
        .push_extremum_cut_family_range(
            row(&[2, 1], &[2, 3]),
            &[ExtremumCutRule::Suffix(2), ExtremumCutRule::Prefix(3)],
            10,
            20,
        )
        .unwrap();
    let bytes = inner.appended_payload_bytes();
    assert_eq!(inner.appended_cells(), 9);
    let suffix = inner.detach().unwrap();
    assert_eq!(suffix.parts().payload_bytes(), bytes);
    assert_eq!(suffix.parts().conditions(), &[2, 1]);
    assert_eq!(suffix.parts().rows()[0].start, 0);
    assert_eq!(
        suffix
            .parts()
            .extremum_cuts()
            .iter()
            .map(|cut| cut.row)
            .collect::<Vec<_>>(),
        [0, 0]
    );
    assert_eq!(outer.parts().extremum_cuts().len(), 1);
    assert_eq!(outer.parts().conditions(), &[0]);
    outer.append(suffix.view(), 10, 20).unwrap();
    outer.commit();
    assert_eq!(nodes.parts().extremum_cuts().len(), 3);
    assert!(admit(nodes.into_parts()).is_ok());
}

#[test]
fn raw_ordinal_copy_preserves_all_seven_arenas() {
    let mut parts = raw(
        NumericDomain::Ordinal { rank_count: 4 },
        vec![1, 3],
        vec![
            ExtremumCutDescriptor {
                row: 0,
                rule: ExtremumCutRule::Prefix(2)
            };
            2
        ],
    );
    parts.nodes.push(Node::extremum_cut(0));
    parts.numeric.conditions.push(usize::MAX);
    parts.numeric.values.push(u64::MAX);
    parts.operands.push(0);
    let copied = parts.try_clone().unwrap();
    assert_eq!(copied.nodes(), parts.nodes());
    assert_eq!(copied.operands(), parts.operands());
    assert_eq!(copied.conditions(), parts.conditions());
    assert_eq!(copied.numeric_values(), parts.numeric_values());
    assert_eq!(copied.rows(), parts.rows());
    assert_eq!(copied.thresholds(), parts.thresholds());
    assert_eq!(copied.extremum_cuts(), parts.extremum_cuts());
    assert_eq!(copied.copy_work(), 4 + 1 + 3 + 3 + 1 + 2);
    assert_eq!(copied.payload_bytes(), parts.payload_bytes());
    assert!(copied.pending_numeric_validation_work() > 0);
}

#[test]
fn ordinal_remap_and_selection_preserve_shared_rows() {
    let mut nodes = owner();
    let mut transaction = nodes.transaction();
    transaction
        .push_extremum_cut_family_range(
            row(&[0, 0], &[1, 3]),
            &[ExtremumCutRule::Prefix(2), ExtremumCutRule::Suffix(2)],
            4,
            4,
        )
        .unwrap();
    transaction.commit();
    let remapped = nodes.view().try_remap(&[8, 9, 10, 11]).unwrap();
    assert_eq!(remapped.conditions(), &[8, 8]);
    assert_eq!(remapped.rows().len(), 1);
    assert!(!remapped.numeric.is_checked());
    let (selected, peak) = nodes
        .view()
        .try_select_with_peak(&[Some(2), None, Some(3), Some(4)])
        .unwrap();
    assert_eq!(selected.conditions(), &[2, 2]);
    assert_eq!(selected.rows().len(), 1);
    assert_eq!(selected.extremum_cuts().len(), 2);
    assert!(peak >= selected.retained_bytes() + 3 * std::mem::size_of::<Option<usize>>() as u128);
    let mut target = owner();
    let mut append = target.transaction();
    assert_eq!(append.append(selected.view(), 5, 4).unwrap(), 2..5);
    append.commit();
    assert!(admit(target.into_parts()).is_ok());
    assert_eq!(
        nodes
            .view()
            .try_select(&[None, None, Some(0), Some(1)])
            .unwrap_err(),
        AdmissionError::Edge
    );
}

#[test]
fn admitted_ordinal_copy_keeps_only_established_checks() {
    let mut parts = raw(
        NumericDomain::Ordinal { rank_count: 4 },
        vec![1],
        vec![ExtremumCutDescriptor {
            row: 0,
            rule: ExtremumCutRule::Prefix(2),
        }],
    );
    parts.nodes.push(Node::extremum_cut(0));
    let theory = admit(parts).unwrap();
    assert_eq!(
        theory
            .parts()
            .try_clone()
            .unwrap()
            .pending_numeric_validation_work(),
        0
    );
    let selected = theory.view().try_select(&[Some(0), Some(1)]).unwrap();
    assert!(selected.pending_numeric_validation_work() > 0);
}

#[test]
fn semantic_cut_equality_ignores_row_descriptor_identity() {
    let mut nodes = owner();
    let mut transaction = nodes.transaction();
    for _ in 0..2 {
        transaction
            .push_extremum_cut_family_range(row(&[0], &[1]), &[ExtremumCutRule::Prefix(2)], 4, 2)
            .unwrap();
    }
    transaction.commit();
    assert_ne!(nodes.parts().nodes()[2], nodes.parts().nodes()[3]);
    assert_eq!(nodes.view().node(2), nodes.view().node(3));
}

#[test]
fn boolean_raw_ingress_cannot_hide_cut_nodes() {
    assert_eq!(
        FormulaParts::new(vec![Node::extremum_cut(0)], vec![]).unwrap_err(),
        AdmissionError::Span
    );
}

#[test]
fn unreferenced_cut_descriptors_consume_the_node_limit() {
    let parts = raw(
        NumericDomain::Ordinal { rank_count: 4 },
        vec![],
        vec![
            ExtremumCutDescriptor {
                row: 0,
                rule: ExtremumCutRule::Prefix(0)
            };
            2
        ],
    );
    let limits = AdmissionLimits {
        max_nodes: 1,
        ..AdmissionLimits::default()
    };
    assert_eq!(
        Theory::new(1, parts, vec![], limits).unwrap_err(),
        AdmissionError::Limit
    );
}

#[test]
fn cut_rollback_restores_numeric_validation_evidence() {
    let mut parts = raw(NumericDomain::Ordinal { rank_count: 4 }, vec![1], vec![]);
    let mut frontier = 1;
    let pending = parts.pending_numeric_validation_work();
    {
        let mut transaction = FormulaTransaction::new(&mut parts, &mut frontier);
        transaction
            .push_extremum_cut_family_range(row(&[0], &[1]), &[ExtremumCutRule::Prefix(2)], 2, 2)
            .unwrap();
        assert_eq!(transaction.parts().pending_numeric_validation_work(), 0);
    }
    assert_eq!(parts.pending_numeric_validation_work(), pending);
    assert_eq!(parts.numeric.rows.len(), 1);
    assert!(parts.numeric.cuts.is_empty());
}

#[test]
fn full_ordinal_admission_work_counts_rows_and_descriptors() {
    let mut parts = raw(
        NumericDomain::Ordinal { rank_count: 4 },
        vec![1, 3],
        vec![
            ExtremumCutDescriptor {
                row: 0,
                rule: ExtremumCutRule::Prefix(2)
            };
            2
        ],
    );
    parts
        .nodes
        .extend([Node::extremum_cut(0), Node::extremum_cut(1)]);
    // Raw construction is intentionally repeated to establish its declared
    // occurrence count independently of this test's direct node mutation.
    let parts = parts.try_clone().unwrap();
    let admission =
        FormulaNodes::new(parts).prepare_admission(1, vec![2], AdmissionLimits::default());
    assert_eq!(admission.work(), 2 * 3 + 4 + 1 + (1 + 2) + 2);
    assert!(admission.admit().is_ok());
}

#[test]
fn single_cut_push_keeps_constant_nodes_and_full_rows() {
    let mut nodes = owner();
    let mut transaction = nodes.transaction();
    for expected in 2..4 {
        let cut = ExtremumCut {
            row: row(&[0, 1], &[0, 3]),
            rule: &ExtremumCutRule::Prefix(0),
        };
        assert_eq!(
            transaction.push(NodeView::ExtremumCut(cut), 4, 4).unwrap(),
            expected
        );
    }
    transaction.commit();
    assert_eq!(nodes.parts().rows().len(), 2);
    assert_eq!(nodes.parts().numeric_values(), &[0, 3, 0, 3]);
    assert_eq!(nodes.parts().occurrences(), 4);
}

#[test]
fn empty_cut_family_publishes_no_arenas() {
    let mut nodes = owner();
    let mut transaction = nodes.transaction();
    assert_eq!(
        transaction
            .push_extremum_cut_family_range(row(&[0], &[1]), &[], 2, 0)
            .unwrap(),
        2..2
    );
    assert_eq!(transaction.appended_cells(), 0);
}

#[test]
fn append_rejects_invalid_unused_source_cuts_atomically() {
    let source = raw(
        NumericDomain::Ordinal { rank_count: 4 },
        vec![],
        vec![ExtremumCutDescriptor {
            row: 0,
            rule: ExtremumCutRule::Prefix(5),
        }],
    );
    let mut nodes = owner();
    let mut transaction = nodes.transaction();
    assert_eq!(
        transaction.append(source.view(), 3, 0).unwrap_err(),
        AdmissionError::OrdinalCut
    );
    assert_eq!(transaction.appended_cells(), 0);
}
