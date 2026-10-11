use super::*;
use crate::FormulaNodes;

fn owner() -> FormulaNodes {
    FormulaNodes::new(FormulaParts::new(vec![Node::atom(0), Node::falsum()], vec![]).unwrap())
}

fn raw(rows: Vec<ContributionRow>, thresholds: Vec<ThresholdDescriptor>) -> FormulaParts {
    FormulaParts::from_raw(
        vec![Node::atom(0), Node::threshold(0)],
        vec![],
        vec![0],
        vec![7],
        rows,
        thresholds,
    )
    .unwrap()
}

fn limits(nodes: usize, operands: usize) -> AdmissionLimits {
    AdmissionLimits {
        max_nodes: nodes,
        max_operands: operands,
        ..AdmissionLimits::default()
    }
}

#[test]
fn threshold_storage_keeps_the_node_size() {
    assert_eq!(
        std::mem::size_of::<Node>(),
        3 * std::mem::size_of::<usize>()
    );
}

#[test]
fn family_shares_one_row_without_coalescing_positions() {
    let mut nodes = owner();
    let roots = nodes
        .push_threshold_family(&[0, 0, 1], &[2, 3, 0], &[-1, 4, 6], 5, 9)
        .unwrap();
    assert_eq!(roots, [2, 3, 4]);
    assert_eq!(nodes.parts().conditions(), [0, 0, 1]);
    assert_eq!(nodes.parts().numeric_values(), [2, 3, 0]);
    assert_eq!(
        nodes.parts().rows(),
        [NumericRow {
            start: 0,
            length: 3,
            domain: NumericDomain::Weights,
        }]
    );
    assert_eq!(nodes.parts().occurrences(), 9);
    for (&root, &bound) in roots.iter().zip(&[-1, 4, 6]) {
        assert_eq!(
            nodes.view().node(root).unwrap(),
            NodeView::Threshold(Threshold {
                conditions: &[0, 0, 1],
                weights: &[2, 3, 0],
                bound,
            })
        );
    }
}

#[test]
fn empty_rows_preserve_all_widened_bounds() {
    let mut nodes = FormulaNodes::default();
    let bounds = [i128::MIN, 0, 1, i128::MAX];
    let roots = nodes
        .push_threshold_family(&[], &[], &bounds, 4, 0)
        .unwrap();
    let theory = nodes
        .prepare_admission(0, roots, limits(4, 0))
        .admit()
        .unwrap();
    for (index, &bound) in bounds.iter().enumerate() {
        assert_eq!(
            theory.view().node(index).unwrap(),
            NodeView::Threshold(Threshold {
                conditions: &[],
                weights: &[],
                bound,
            })
        );
    }
}

#[test]
fn single_threshold_push_copies_the_complete_row() {
    let mut nodes = owner();
    let mut transaction = nodes.transaction();
    transaction
        .push(
            NodeView::Threshold(Threshold {
                conditions: &[0, 1],
                weights: &[u64::MAX, 0],
                bound: i128::from(u64::MAX),
            }),
            3,
            2,
        )
        .unwrap();
    transaction.commit();
    assert_eq!(nodes.parts().numeric_values(), &[u64::MAX, 0]);
    assert_eq!(nodes.parts().thresholds().len(), 1);
}

#[test]
fn zero_weight_children_still_require_predecessors() {
    let mut nodes = owner();
    for bounds in [&[][..], &[i128::MIN][..], &[i128::MAX][..]] {
        assert_eq!(
            nodes
                .push_threshold_family(&[2], &[0], bounds, 3, 1)
                .unwrap_err(),
            AdmissionError::Edge
        );
        assert_eq!(nodes.view().len(), 2);
        assert!(nodes.parts().rows().is_empty());
    }
}

#[test]
fn empty_families_still_validate_numeric_shape() {
    let mut nodes = owner();
    assert_eq!(
        nodes
            .push_threshold_family(&[0], &[], &[], 2, 0)
            .unwrap_err(),
        AdmissionError::ThresholdShape
    );
    assert!(nodes.parts().conditions().is_empty());
}

#[test]
fn empty_families_publish_no_unused_row() {
    let mut nodes = owner();
    assert!(
        nodes
            .push_threshold_family(&[0], &[1], &[], 2, 0)
            .unwrap()
            .is_empty()
    );
    assert!(nodes.parts().rows().is_empty());
    assert!(nodes.parts().conditions().is_empty());
}

#[test]
fn family_occurrence_refusal_preserves_all_arenas() {
    let mut nodes = owner();
    let mut transaction = nodes.transaction();
    transaction.push(NodeView::And(&[0, 1, 0]), 6, 6).unwrap();
    assert_eq!(
        transaction
            .push_threshold_family(&[0, 1], &[1, 2], &[1, 2], 6, 6)
            .unwrap_err(),
        AdmissionError::Limit
    );
    assert_eq!(transaction.parts().occurrences(), 3);
    assert_eq!(transaction.parts().operands(), &[0, 1, 0]);
    assert!(transaction.parts().rows().is_empty());
    assert!(transaction.parts().numeric_values().is_empty());
    transaction.commit();
}

#[test]
fn nested_rollback_removes_only_new_numeric_rows() {
    let mut nodes = owner();
    let mut outer = nodes.transaction();
    outer
        .push_threshold_family(&[0], &[5], &[1, 4], 6, 8)
        .unwrap();
    {
        let mut inner = outer.transaction();
        inner
            .push_threshold_family(&[2, 3], &[1, 1], &[1], 6, 8)
            .unwrap();
    }
    assert_eq!(outer.parts().rows().len(), 1);
    assert_eq!(outer.parts().thresholds().len(), 2);
    assert_eq!(outer.parts().conditions(), &[0]);
    assert_eq!(outer.parts().occurrences(), 2);
    outer.commit();
}

#[test]
fn detachment_rebases_numeric_rows_without_changing_children() {
    let mut nodes = owner();
    nodes.push_threshold_family(&[0], &[8], &[4], 6, 8).unwrap();
    let mut append = nodes.transaction();
    append
        .push_threshold_family(&[2, 0], &[3, 5], &[4, 8], 6, 8)
        .unwrap();
    let expected_bytes = append.appended_payload_bytes();
    assert_eq!(append.appended_cells(), 9);
    let suffix = append.detach().unwrap();
    assert_eq!(suffix.first(), 3);
    assert_eq!(suffix.parts().payload_bytes(), expected_bytes);
    assert_eq!(
        suffix.parts().rows(),
        &[NumericRow {
            start: 0,
            length: 2,
            domain: NumericDomain::Weights,
        }]
    );
    assert_eq!(
        suffix.parts().thresholds(),
        &[
            ThresholdDescriptor { row: 0, bound: 4 },
            ThresholdDescriptor { row: 0, bound: 8 },
        ]
    );
    assert_eq!(
        suffix.view().node(1).unwrap(),
        NodeView::Threshold(Threshold {
            conditions: &[2, 0],
            weights: &[3, 5],
            bound: 8,
        })
    );
    assert_eq!(nodes.parts().conditions(), &[0]);
    assert_eq!(nodes.parts().thresholds().len(), 1);
}

#[test]
fn raw_admission_rejects_incomplete_numeric_columns() {
    let parts = FormulaParts::from_raw(
        vec![Node::falsum()],
        vec![],
        vec![0],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    assert_eq!(
        Theory::new(0, parts, vec![], limits(1, 1)).unwrap_err(),
        AdmissionError::NumericShape
    );
}

#[test]
fn unused_raw_descriptors_require_existing_rows() {
    let parts = FormulaParts::from_raw(
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![ThresholdDescriptor { row: 0, bound: 0 }],
    )
    .unwrap();
    let nodes = FormulaNodes::new(parts);
    assert_eq!(
        nodes
            .prepare_admission(0, vec![], limits(1, 0))
            .admit()
            .unwrap_err(),
        AdmissionError::Span
    );
}

#[test]
fn unused_raw_rows_require_complete_ranges() {
    let parts = FormulaParts::from_raw(
        vec![],
        vec![],
        vec![],
        vec![],
        vec![ContributionRow {
            start: 0,
            length: 1,
        }],
        vec![],
    )
    .unwrap();
    assert_eq!(
        FormulaNodes::new(parts)
            .prepare_admission(0, vec![], limits(1, 1))
            .admit()
            .unwrap_err(),
        AdmissionError::Span
    );
}

#[test]
fn raw_thresholds_validate_complete_topology() {
    let graph = FormulaParts::from_raw(
        vec![Node::falsum(), Node::threshold(0)],
        vec![],
        vec![1],
        vec![0],
        vec![ContributionRow {
            start: 0,
            length: 1,
        }],
        vec![ThresholdDescriptor { row: 0, bound: -1 }],
    )
    .unwrap();
    assert_eq!(
        Theory::new(0, graph, vec![1], limits(2, 1)).unwrap_err(),
        AdmissionError::Edge
    );
}

#[test]
fn raw_thresholds_reject_missing_descriptor_references() {
    let graph = FormulaParts::from_raw(
        vec![Node::threshold(0)],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    assert_eq!(
        Theory::new(0, graph, vec![], limits(1, 0)).unwrap_err(),
        AdmissionError::Span
    );
}

#[test]
fn unused_numeric_cells_consume_the_column_limit() {
    for maximum in [0, 1] {
        let graph = FormulaParts::from_raw(
            vec![],
            vec![],
            vec![usize::MAX],
            vec![u64::MAX],
            vec![],
            vec![],
        )
        .unwrap();
        let result = Theory::new(0, graph, vec![], limits(0, maximum));
        if maximum == 0 {
            assert_eq!(result.unwrap_err(), AdmissionError::Limit);
        } else {
            assert!(result.is_ok());
        }
    }
}

#[test]
fn unused_descriptors_consume_the_node_limit() {
    let graph = FormulaParts::from_raw(
        vec![],
        vec![],
        vec![],
        vec![],
        vec![ContributionRow {
            start: 0,
            length: 0,
        }],
        vec![ThresholdDescriptor { row: 0, bound: 0 }; 2],
    )
    .unwrap();
    assert_eq!(
        Theory::new(0, graph, vec![], limits(1, 0)).unwrap_err(),
        AdmissionError::Limit
    );
}

#[test]
fn raw_shared_rows_count_every_node_occurrence() {
    let mut graph = raw(
        vec![ContributionRow {
            start: 0,
            length: 1,
        }],
        vec![ThresholdDescriptor { row: 0, bound: 5 }],
    );
    graph.nodes.push(Node::threshold(0));
    graph.occurrences = 0;
    assert_eq!(
        Theory::new(1, graph, vec![], limits(3, 1)).unwrap_err(),
        AdmissionError::Limit
    );
}

#[test]
fn complete_copy_preserves_numeric_sharing() {
    let mut nodes = owner();
    nodes
        .push_threshold_family(&[0, 0], &[3, 4], &[4, 7], 4, 4)
        .unwrap();
    let copy = nodes.parts().try_clone().unwrap();
    assert_eq!(copy.payload_bytes(), nodes.parts().payload_bytes());
    assert_eq!(copy.rows().len(), 1);
    assert_eq!(copy.thresholds().len(), 2);
    assert_eq!(copy.view().node(3).unwrap(), nodes.view().node(3).unwrap());
    assert_ne!(
        copy.conditions().as_ptr(),
        nodes.parts().conditions().as_ptr()
    );
}

#[test]
fn remapping_preserves_shared_rows_and_inline_edges() {
    let mut nodes = owner();
    let mut append = nodes.transaction();
    append
        .push_threshold_family(&[0, 1], &[3, 4], &[4, 7], 5, 6)
        .unwrap();
    append.push(NodeView::Implies(2, 3), 5, 6).unwrap();
    let suffix = append.detach().unwrap();
    let remapped = suffix.view().try_remap(&[8, 9, 10, 11]).unwrap();
    assert_eq!(remapped.conditions(), &[8, 9]);
    assert_eq!(remapped.rows().len(), 1);
    assert_eq!(remapped.view().node(2).unwrap(), NodeView::Implies(10, 11));
    assert_eq!(
        suffix.view().try_remap(&[8]).unwrap_err(),
        AdmissionError::Edge
    );
}

#[test]
fn retained_bytes_include_reserved_numeric_capacity() {
    let mut nodes = owner();
    let before = nodes.parts().retained_bytes();
    {
        let mut transaction = nodes.transaction();
        transaction
            .push_threshold_family(&[0, 1], &[1, 2], &[1, 2], 4, 4)
            .unwrap();
    }
    assert!(nodes.parts().retained_bytes() > before);
    assert_eq!(
        nodes.parts().payload_bytes(),
        2 * std::mem::size_of::<Node>() as u128
    );
}

#[test]
fn selection_copies_only_the_closed_subset() {
    let mut nodes = owner();
    nodes
        .push_threshold_family(&[0, 0], &[3, 4], &[4, 7], 4, 4)
        .unwrap();
    let selected = nodes
        .view()
        .try_select(&[Some(8), None, Some(9), Some(10)])
        .unwrap();
    assert_eq!(selected.nodes().len(), 3);
    assert_eq!(selected.conditions(), &[8, 8]);
    assert_eq!(selected.rows().len(), 1);
    assert_eq!(selected.thresholds().len(), 2);
    assert_eq!(
        nodes
            .view()
            .try_select(&[None, None, Some(0), Some(1)])
            .unwrap_err(),
        AdmissionError::Edge
    );
}

#[test]
fn selected_append_retains_shared_rows() {
    let mut source = owner();
    source
        .push_threshold_family(&[0], &[5], &[1, 5], 4, 2)
        .unwrap();
    let selected = source
        .view()
        .try_select(&[Some(2), None, Some(3), Some(4)])
        .unwrap();
    let mut target = owner();
    let mut append = target.transaction();
    assert_eq!(append.append(selected.view(), 5, 2).unwrap(), 2..5);
    append.commit();
    assert_eq!(target.parts().rows().len(), 1);
    assert_eq!(target.parts().thresholds().len(), 2);
    assert_eq!(target.parts().conditions(), &[2]);
    assert_eq!(
        target.view().node(4).unwrap(),
        NodeView::Threshold(Threshold {
            conditions: &[2],
            weights: &[5],
            bound: 5
        })
    );
}

#[test]
fn appended_storage_rolls_back_on_a_forward_child() {
    let source = FormulaParts::from_raw(
        vec![Node::threshold(0)],
        vec![],
        vec![2],
        vec![0],
        vec![ContributionRow {
            start: 0,
            length: 1,
        }],
        vec![ThresholdDescriptor { row: 0, bound: -1 }],
    )
    .unwrap();
    let mut target = owner();
    let mut append = target.transaction();
    assert_eq!(
        append.append(source.view(), 3, 1).unwrap_err(),
        AdmissionError::Edge
    );
    assert_eq!(append.parts().nodes().len(), 2);
    assert!(append.parts().rows().is_empty());
    assert!(append.parts().thresholds().is_empty());
    assert!(append.parts().conditions().is_empty());
}

#[test]
fn appended_shared_storage_detaches_without_prefix_rows() {
    let mut source = owner();
    source
        .push_threshold_family(&[0], &[5], &[1, 5], 4, 2)
        .unwrap();
    let mut target = owner();
    target
        .push_threshold_family(&[1], &[2], &[1], 3, 1)
        .unwrap();
    let mapped = source.view().try_remap(&[3, 4, 5, 6]).unwrap();
    let mut append = target.transaction();
    assert_eq!(append.append(mapped.view(), 7, 3).unwrap(), 3..7);
    let suffix = append.detach().unwrap();
    assert_eq!(
        suffix.parts().rows(),
        &[NumericRow {
            start: 0,
            length: 1,
            domain: NumericDomain::Weights,
        }]
    );
    assert_eq!(suffix.parts().conditions(), &[3]);
    assert_eq!(
        suffix
            .parts()
            .thresholds()
            .iter()
            .map(|item| item.row)
            .collect::<Vec<_>>(),
        [0, 0]
    );
    assert_eq!(target.parts().rows().len(), 1);
}

#[test]
fn boolean_raw_constructor_cannot_hide_threshold_references() {
    assert_eq!(
        FormulaParts::new(vec![Node::threshold(0)], vec![]).unwrap_err(),
        AdmissionError::Span
    );
}

#[test]
fn family_growth_cannot_repair_a_raw_prefix_descriptor() {
    let graph = FormulaParts::from_raw(
        vec![Node::falsum(), Node::threshold(0)],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let mut nodes = FormulaNodes::new(graph);
    assert_eq!(
        nodes
            .push_threshold_family(&[0], &[1], &[1], 3, 1)
            .unwrap_err(),
        AdmissionError::Span
    );
    assert_eq!(nodes.parts().occurrences(), 0);
    assert!(nodes.parts().thresholds().is_empty());
}

#[test]
fn family_growth_cannot_repair_an_unused_raw_row() {
    let graph = FormulaParts::from_raw(
        vec![Node::falsum()],
        vec![],
        vec![],
        vec![],
        vec![ContributionRow {
            start: 0,
            length: 1,
        }],
        vec![],
    )
    .unwrap();
    let mut nodes = FormulaNodes::new(graph);
    assert_eq!(
        nodes
            .push_threshold_family(&[0], &[1], &[1], 3, 1)
            .unwrap_err(),
        AdmissionError::Span
    );
    assert!(nodes.parts().conditions().is_empty());
}

#[test]
fn append_cannot_repair_a_raw_prefix_descriptor() {
    let graph = FormulaParts::from_raw(
        vec![Node::falsum(), Node::threshold(0)],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let mut target = FormulaNodes::new(graph);
    let mut source = owner();
    source
        .push_threshold_family(&[0], &[1], &[1], 3, 1)
        .unwrap();
    let mut transaction = target.transaction();
    assert_eq!(
        transaction.append(source.view(), 5, 1).unwrap_err(),
        AdmissionError::Span
    );
    assert!(transaction.parts().thresholds().is_empty());
}

#[test]
fn nested_rollback_restores_numeric_validation_cache() {
    let mut graph = raw(
        vec![ContributionRow {
            start: 0,
            length: 1,
        }],
        vec![ThresholdDescriptor { row: 0, bound: 5 }],
    );
    let mut validated = 2;
    assert!(!graph.numeric.is_checked());
    let mut outer = FormulaTransaction::new(&mut graph, &mut validated);
    {
        let mut inner = outer.transaction();
        inner.push_threshold_family(&[0], &[1], &[1], 3, 2).unwrap();
        assert!(inner.parts().numeric.is_checked());
    }
    assert!(!outer.parts().numeric.is_checked());
    assert_eq!(outer.validation_frontier(), 2);
    outer.commit();
}

#[test]
fn selection_peak_includes_internal_arena_maps() {
    let mut nodes = owner();
    nodes
        .push_threshold_family(&[0], &[1], &[1, 2], 4, 2)
        .unwrap();
    let (parts, peak) = nodes
        .view()
        .try_select_with_peak(&[Some(0), None, Some(1), Some(2)])
        .unwrap();
    let minimum_maps = 3 * std::mem::size_of::<Option<usize>>() as u128;
    assert!(peak >= parts.retained_bytes() + minimum_maps);
}

#[test]
fn boolean_selection_peak_equals_output_capacity() {
    let nodes = owner();
    let (parts, peak) = nodes.view().try_select_with_peak(&[Some(0), None]).unwrap();
    assert_eq!(peak, parts.retained_bytes());
}

#[test]
fn complete_copy_preserves_only_established_numeric_checks() {
    let mut nodes = owner();
    nodes.push_threshold_family(&[0], &[1], &[1], 3, 1).unwrap();
    assert!(nodes.parts().try_clone().unwrap().numeric.is_checked());
    let raw = FormulaParts::from_raw(vec![], vec![], vec![], vec![], vec![], vec![]).unwrap();
    assert!(!raw.try_clone().unwrap().numeric.is_checked());
}

#[test]
fn remapping_discards_numeric_validation_cache() {
    let mut nodes = owner();
    nodes.push_threshold_family(&[0], &[1], &[1], 3, 1).unwrap();
    let remapped = nodes.view().try_remap(&[0, 1, 2]).unwrap();
    assert!(!remapped.numeric.is_checked());
    let selected = nodes.view().try_select(&[Some(0), None, Some(1)]).unwrap();
    assert!(!selected.numeric.is_checked());
}

#[test]
fn copy_work_counts_physical_and_unused_cells() {
    let parts = FormulaParts::from_raw(
        vec![
            Node::atom(0),
            Node::threshold(0),
            Node::threshold(1),
            Node::threshold(0),
        ],
        vec![0; 3],
        vec![0; 5],
        vec![7; 5],
        vec![
            ContributionRow {
                start: 0,
                length: 2,
            },
            ContributionRow {
                start: 2,
                length: 1,
            },
        ],
        vec![
            ThresholdDescriptor { row: 0, bound: 2 },
            ThresholdDescriptor { row: 0, bound: 3 },
            ThresholdDescriptor { row: 1, bound: 7 },
        ],
    )
    .unwrap();
    assert_eq!(parts.occurrences(), 6);
    // Four copied/recounted nodes, then Boolean cells, both full numeric
    // columns, rows and descriptors. Sharing does not multiply copy work.
    assert_eq!(parts.copy_work(), 2 * 4 + 3 + 5 + 5 + 2 + 3);
    assert_eq!(parts.try_clone().unwrap().copy_work(), parts.copy_work());
}

#[test]
fn admitted_raw_copy_reuses_numeric_validation() {
    let parts = raw(
        vec![ContributionRow {
            start: 0,
            length: 1,
        }],
        vec![ThresholdDescriptor { row: 0, bound: 5 }],
    );
    assert!(parts.pending_numeric_validation_work() > 0);
    let theory = Theory::new(1, parts, vec![1], limits(2, 1)).unwrap();
    let copied = theory.parts().try_clone().unwrap();
    assert_eq!(copied.pending_numeric_validation_work(), 0);
    assert_eq!(copied.view().node(1), theory.view().node(1));
}
