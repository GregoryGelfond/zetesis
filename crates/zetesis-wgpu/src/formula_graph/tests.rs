mod shared_rows;

use super::*;
use zetesis_ferraris::{AdmissionLimits, FormulaParts, Node, OperandSpan};

fn theory() -> Theory {
    Theory::new(
        2,
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::and_span(OperandSpan {
                    start: 1,
                    length: 3,
                }),
                Node::or_span(OperandSpan {
                    start: 2,
                    length: 3,
                }),
            ],
            vec![usize::MAX, 0, 1, 0, 2, usize::MAX],
        )
        .unwrap(),
        vec![3],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn combined_word_addresses_are_checked_before_device_bytes() {
    assert_eq!(bytes(0, 0).unwrap(), 16);
    assert_eq!(bytes(u32::MAX / 4, 3).unwrap(), u64::from(u32::MAX) * 4);
    assert_eq!(
        bytes(u32::MAX / 4, 4).unwrap_err().kind(),
        GpuErrorKind::Capacity
    );
    assert_eq!(
        bytes(u32::MAX / 4 + 1, 0).unwrap_err().kind(),
        GpuErrorKind::Capacity
    );
}

#[test]
fn logical_rows_omit_unused_cells_and_preserve_overlap_occurrences() {
    let theory = theory();
    let shape = Shape::new(&theory, &wgpu::Limits::default()).unwrap();
    assert_eq!((shape.edges, shape.wide_words, shape.bytes), (6, 6, 88));
    let mut packed = Vec::with_capacity(usize::try_from(shape.bytes / 4).unwrap());
    pack(&theory, &shape.rows, &mut packed, &Cancellation::default()).unwrap();
    assert_eq!(&packed[8..16], &[5, 0, 3, 0, 6, 3, 3, 0]);
    assert_eq!(&packed[16..], &[0, 1, 0, 1, 0, 2]);
    assert_eq!(packed.len() as u64 * 4, shape.bytes);
}

#[test]
fn operand_tail_is_included_in_both_device_byte_limits() {
    for buffer in [true, false] {
        for (limit, admitted) in [(87, false), (88, true)] {
            let mut device = wgpu::Limits::default();
            if buffer {
                device.max_buffer_size = limit;
            } else {
                device.max_storage_buffer_binding_size = limit;
            }
            assert_eq!(Shape::new(&theory(), &device).is_ok(), admitted);
        }
    }
}

#[test]
fn cancelled_operand_packing_returns_a_typed_interruption() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert_eq!(
        pack(
            &theory(),
            &Shape::new(&theory(), &wgpu::Limits::default())
                .unwrap()
                .rows,
            &mut Vec::new(),
            &cancellation
        )
        .unwrap_err()
        .interruption(),
        Some(zetesis_cpu::Stop::Cancelled)
    );
}

fn native_theory(bound: i128) -> Theory {
    use zetesis_ferraris::{FormulaNodes, NodeView};
    let mut nodes = FormulaNodes::new(
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::falsum(),
                Node::implies(0, 2),
                Node::implies(3, 1),
            ],
            vec![],
        )
        .unwrap(),
    );
    let mut transaction = nodes.transaction();
    let guards = transaction
        .push_threshold_family(
            &[0, 0, 1, 3, 4],
            &[1, 2, 0, 2, 1],
            &[bound, bound + 1],
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    let root = transaction
        .push(
            NodeView::Implies(guards[0], guards[1]),
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    transaction.commit();
    Theory::new(
        2,
        nodes.into_parts(),
        vec![root],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn physical_truth(words: &[u32], count: u32, atoms: usize, frozen: Option<&[bool]>) -> Vec<bool> {
    let mut values = Vec::new();
    for (index, node) in words.chunks_exact(4).take(count as usize).enumerate() {
        let value = match node[0] {
            0 => false,
            1 => atoms & (1 << node[1]) != 0,
            2 => values[node[1] as usize] && values[node[2] as usize],
            3 => values[node[1] as usize] || values[node[2] as usize],
            4 => !values[node[1] as usize] || values[node[2] as usize],
            5 | 6 => {
                let begin = count as usize * 4 + node[1] as usize;
                let row = &words[begin..begin + node[2] as usize];
                if node[0] == 5 {
                    row.iter().all(|&child| values[child as usize])
                } else {
                    row.iter().any(|&child| values[child as usize])
                }
            }
            7..=9 => numeric_truth(words, count, node, &values),
            10 => true,
            _ => panic!("unknown physical operation"),
        };
        values.push(value && frozen.is_none_or(|mask| mask[index]));
    }
    values
}

#[test]
fn native_transport_preserves_original_and_arbitrary_frozen_truth() {
    use zetesis_ferraris::{EvaluationLimits, EvaluationWorkspace, Interpretation, Limits};
    let cancellation = Cancellation::default();
    for bound in -1..=8 {
        let theory = native_theory(bound);
        let shape = Shape::new(&theory, &wgpu::Limits::default()).unwrap();
        let mut words = Vec::with_capacity(usize::try_from(shape.bytes).unwrap() / 4);
        pack(&theory, &shape.rows, &mut words, &cancellation).unwrap();
        assert_eq!(words.len() as u64 * 4, shape.bytes);
        let interpretation = |mask| {
            Interpretation::new(&theory, (0..2).filter(|atom| mask & (1 << atom) != 0)).unwrap()
        };
        for candidate in 0..4 {
            let original = physical_truth(&words, shape.nodes, candidate, None);
            let candidate = interpretation(candidate);
            let mut workspace = EvaluationWorkspace::default();
            let truth = workspace
                .evaluate(&candidate, EvaluationLimits::default(), &cancellation)
                .result
                .unwrap();
            assert_eq!(original.len(), theory.view().len());
            for (index, &value) in original.iter().enumerate() {
                assert_eq!(value, truth.node_truth(index).unwrap());
            }
            for tested in 0..4 {
                // Every interpretation is tested, including those outside M.
                let reduct = physical_truth(&words, shape.nodes, tested, Some(&original));
                let actual = theory.roots().iter().all(|&root| reduct[root]);
                let expected = zetesis_ferraris::models_reduct(
                    &theory,
                    &candidate,
                    &interpretation(tested),
                    Limits::default(),
                    &cancellation,
                )
                .unwrap();
                assert_eq!(actual, expected, "bound {bound}, tested {tested}");
            }
        }
    }
}

#[test]
fn native_physical_shape_refuses_device_storage_before_packing() {
    let theory = native_theory(3);
    let shape = Shape::new(&theory, &wgpu::Limits::default()).unwrap();
    assert_eq!(shape.nodes as usize, theory.view().len());
    let device = wgpu::Limits {
        max_storage_buffer_binding_size: shape.bytes - 1,
        ..wgpu::Limits::default()
    };
    assert_eq!(
        Shape::new(&theory, &device).err().unwrap().kind(),
        GpuErrorKind::Capacity
    );
}

#[test]
fn native_shape_scan_preserves_cancellation() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert_eq!(
        Shape::with_control(
            &native_theory(2),
            &wgpu::Limits::default(),
            u64::MAX,
            &cancellation
        )
        .err()
        .unwrap()
        .interruption(),
        Some(zetesis_cpu::Stop::Cancelled)
    );
}

fn ordinal_theory() -> Theory {
    use zetesis_ferraris::{ExtremumCutRule, ExtremumRow, FormulaNodes, NodeView};
    let mut nodes = FormulaNodes::new(
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::falsum(),
                Node::implies(0, 2),
                Node::implies(3, 1),
            ],
            vec![],
        )
        .unwrap(),
    );
    let rules: Vec<_> = (0..=5)
        .flat_map(|bound| {
            [
                ExtremumCutRule::Prefix(bound),
                ExtremumCutRule::Suffix(bound),
            ]
        })
        .collect();
    let mut transaction = nodes.transaction();
    let cuts = transaction
        .push_extremum_cut_family_range(
            ExtremumRow {
                conditions: &[0, 0, 1, 3, 4],
                ranks: &[0, 4, 2, 3, 1],
                rank_count: 5,
            },
            &rules,
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    let root = transaction
        .push(
            NodeView::Implies(cuts.start + 4, cuts.start + 5),
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    // Exercise a wide tail referencing native cuts as well as ordinary nodes.
    transaction
        .push(
            NodeView::Or(&[root, cuts.start, cuts.end - 1]),
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    transaction.commit();
    Theory::new(
        2,
        nodes.into_parts(),
        vec![root],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn ordinal_transport_preserves_original_and_arbitrary_frozen_truth() {
    use zetesis_ferraris::{EvaluationLimits, EvaluationWorkspace, Interpretation, Limits};
    let cancellation = Cancellation::default();
    let theory = ordinal_theory();
    let shape = Shape::new(&theory, &wgpu::Limits::default()).unwrap();
    assert_eq!(shape.nodes as usize, theory.view().len());
    let mut words = Vec::with_capacity(usize::try_from(shape.bytes).unwrap() / 4);
    pack(&theory, &shape.rows, &mut words, &cancellation).unwrap();
    assert_eq!(words.len() as u64 * 4, shape.bytes);
    let interpretation =
        |mask| Interpretation::new(&theory, (0..2).filter(|atom| mask & (1 << atom) != 0)).unwrap();
    for candidate_mask in 0..4 {
        let original = physical_truth(&words, shape.nodes, candidate_mask, None);
        let candidate = interpretation(candidate_mask);
        let mut workspace = EvaluationWorkspace::default();
        let truth = workspace
            .evaluate(&candidate, EvaluationLimits::default(), &cancellation)
            .result
            .unwrap();
        assert_eq!(original.len(), theory.view().len());
        for (index, &value) in original.iter().enumerate() {
            assert_eq!(value, truth.node_truth(index).unwrap());
        }
        for tested in 0..4 {
            let reduct = physical_truth(&words, shape.nodes, tested, Some(&original));
            for index in 0..theory.view().len() {
                if let NodeView::ExtremumCut(cut) = theory.view().node(index).unwrap() {
                    let measure = cut
                        .row
                        .reduce(|child| Ok::<_, zetesis_cpu::Stop>(reduct[child]))
                        .unwrap();
                    assert_eq!(reduct[index], original[index] && cut.rule.holds(measure));
                }
            }
            let actual = theory.roots().iter().all(|&root| reduct[root]);
            let expected = zetesis_ferraris::models_reduct(
                &theory,
                &candidate,
                &interpretation(tested),
                Limits::default(),
                &cancellation,
            )
            .unwrap();
            assert_eq!(actual, expected, "M={candidate_mask}, J={tested}");
        }
    }
}

#[test]
fn ordinal_shape_refuses_device_bytes_before_packing() {
    let theory = ordinal_theory();
    let shape = Shape::new(&theory, &wgpu::Limits::default()).unwrap();
    for buffer in [true, false] {
        let mut device = wgpu::Limits::default();
        if buffer {
            device.max_buffer_size = shape.bytes - 1;
        } else {
            device.max_storage_buffer_binding_size = shape.bytes - 1;
        }
        assert_eq!(
            Shape::new(&theory, &device).err().unwrap().kind(),
            GpuErrorKind::Capacity
        );
    }
}

#[test]
fn ordinal_packing_rejects_unadmitted_output_capacity() {
    let theory = ordinal_theory();
    let shape = Shape::new(&theory, &wgpu::Limits::default()).unwrap();
    let mut words = Vec::new();
    assert_eq!(
        pack(&theory, &shape.rows, &mut words, &Cancellation::default())
            .unwrap_err()
            .kind(),
        GpuErrorKind::Capacity
    );
    assert!(words.is_empty());
}

#[test]
fn ordinal_shape_scan_and_packing_preserve_cancellation() {
    let theory = ordinal_theory();
    let shape = Shape::new(&theory, &wgpu::Limits::default()).unwrap();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert_eq!(
        Shape::with_control(&theory, &wgpu::Limits::default(), u64::MAX, &cancellation)
            .err()
            .unwrap()
            .interruption(),
        Some(zetesis_cpu::Stop::Cancelled)
    );
    let mut words = Vec::with_capacity(usize::try_from(shape.bytes).unwrap() / 4);
    assert_eq!(
        pack(&theory, &shape.rows, &mut words, &cancellation)
            .unwrap_err()
            .interruption(),
        Some(zetesis_cpu::Stop::Cancelled)
    );
    assert!(words.is_empty());
}

// A mathematical reading of the wire, independent of the shader's limb
// arithmetic and domain propagation. Frozen evaluation supplies reduced truth.
fn numeric_truth(words: &[u32], count: u32, node: &[u32], values: &[bool]) -> bool {
    let descriptor = count as usize * 4 + node[1] as usize;
    let length = node[2] as usize;
    let row = count as usize * 4 + words[descriptor] as usize + 2;
    let integer = |start: usize, limbs: usize| {
        (0..limbs).fold(0u128, |value, limb| {
            value | (u128::from(words[start + limb]) << (32 * limb))
        })
    };
    let value = |index: usize| {
        u64::from(words[row + length + index]) | (u64::from(words[row + 2 * length + index]) << 32)
    };
    if node[0] == 7 {
        let sum: u128 = (0..length)
            .filter(|&i| values[words[row + i] as usize])
            .map(|i| u128::from(value(i)))
            .sum();
        return sum >= integer(descriptor + 1, 4);
    }
    let bound = integer(descriptor + 1, 2);
    let carrier = integer(descriptor + 3, 2);
    let mut minimum = carrier - 1;
    let mut maximum = 0;
    for i in 0..length {
        if values[words[row + i] as usize] {
            minimum = minimum.min(u128::from(value(i)));
            maximum = maximum.max(u128::from(value(i)));
        }
    }
    if node[0] == 8 {
        minimum < bound
    } else {
        maximum >= bound
    }
}

#[test]
fn shared_threshold_rows_keep_one_transported_column_set() {
    use zetesis_ferraris::FormulaNodes;
    let mut nodes =
        FormulaNodes::new(FormulaParts::new(vec![Node::atom(0), Node::atom(1)], vec![]).unwrap());
    let mut tx = nodes.transaction();
    let roots = tx
        .push_threshold_family(
            &[0, 1, 0],
            &[1, u64::MAX, 0],
            &[1, 2, i128::from(u64::MAX)],
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    tx.commit();
    let theory = Theory::new(2, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap();
    let shape = Shape::new(&theory, &wgpu::Limits::default()).unwrap();
    assert_eq!(shape.nodes, 5);
    assert_eq!(shape.wide_words, 3 * 5 + 2 + 3 * 3 + 3);
    let mut words = Vec::with_capacity(usize::try_from(shape.bytes).unwrap() / 4);
    pack(&theory, &shape.rows, &mut words, &Cancellation::default()).unwrap();
    let offsets: Vec<_> = theory
        .roots()
        .iter()
        .map(|&root| words[shape.nodes as usize * 4 + words[root * 4 + 1] as usize])
        .collect();
    assert_eq!(offsets, vec![15, 15, 15]);
}

#[test]
fn full_width_thresholds_preserve_frozen_truth_with_aliased_atoms() {
    use zetesis_ferraris::{FormulaNodes, Interpretation, Limits, NodeView, Threshold};
    let cancellation = Cancellation::default();
    for bound in [
        i128::MIN,
        -1,
        0,
        1,
        1i128 << 32,
        i128::from(u64::MAX),
        i128::from(u64::MAX) + 1,
        i128::from(u64::MAX) * 2,
        i128::MAX,
    ] {
        let mut nodes = FormulaNodes::new(
            FormulaParts::new(vec![Node::atom(0), Node::atom(0), Node::atom(1)], vec![]).unwrap(),
        );
        let mut tx = nodes.transaction();
        let root = tx
            .push(
                NodeView::Threshold(Threshold {
                    conditions: &[0, 1, 2, 2],
                    weights: &[u64::MAX, u64::MAX, 1, 0],
                    bound,
                }),
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        tx.commit();
        let theory = Theory::new(
            2,
            nodes.into_parts(),
            vec![root],
            AdmissionLimits::default(),
        )
        .unwrap();
        let shape = Shape::new(&theory, &wgpu::Limits::default()).unwrap();
        let mut words = Vec::with_capacity(usize::try_from(shape.bytes).unwrap() / 4);
        pack(&theory, &shape.rows, &mut words, &cancellation).unwrap();
        let interpretation = |mask| {
            Interpretation::new(&theory, (0..2).filter(|atom| mask & (1 << atom) != 0)).unwrap()
        };
        for candidate in 0..4 {
            let frozen = physical_truth(&words, shape.nodes, candidate, None);
            for tested in 0..4 {
                let actual = physical_truth(&words, shape.nodes, tested, Some(&frozen))[root];
                let expected = zetesis_ferraris::models_reduct(
                    &theory,
                    &interpretation(candidate),
                    &interpretation(tested),
                    Limits::default(),
                    &cancellation,
                )
                .unwrap();
                assert_eq!(actual, expected, "bound={bound}, M={candidate}, J={tested}");
            }
        }
    }
}

#[test]
fn constant_numeric_guards_omit_rows_and_directory_storage() {
    use zetesis_ferraris::{ExtremumRow, FormulaNodes};
    let mut nodes = FormulaNodes::new(FormulaParts::new(vec![Node::atom(0)], vec![]).unwrap());
    let mut tx = nodes.transaction();
    let mut roots = tx
        .push_threshold_family(
            &[0, 0],
            &[1, 2],
            &[i128::MIN, 0, 4, i128::MAX],
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    let cuts = tx
        .push_extremum_cut_family_range(
            ExtremumRow {
                conditions: &[0, 0],
                ranks: &[1, 1],
                rank_count: 4,
            },
            &[
                ExtremumCutRule::Prefix(0),
                ExtremumCutRule::Prefix(4),
                ExtremumCutRule::Suffix(0),
                ExtremumCutRule::Suffix(4),
                ExtremumCutRule::Prefix(1),
                ExtremumCutRule::Suffix(2),
            ],
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    roots.extend(cuts);
    tx.commit();
    let theory = Theory::new(1, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap();
    let shape = Shape::with_control(
        &theory,
        &wgpu::Limits::default(),
        0,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(
        (
            shape.wide_words,
            shape.edges,
            shape.numeric_work,
            shape.rows.bytes().unwrap()
        ),
        (0, 0, 0, 0)
    );
    let mut words = Vec::with_capacity(usize::try_from(shape.bytes).unwrap() / 4);
    pack(&theory, &shape.rows, &mut words, &Cancellation::default()).unwrap();
    let tags: Vec<_> = theory
        .roots()
        .iter()
        .map(|&index| words[index * 4])
        .collect();
    assert_eq!(tags, vec![10, 10, 0, 0, 0, 10, 10, 0, 0, 0]);
}

#[test]
fn numeric_directory_reservation_obeys_staging_allowance() {
    let theory = native_theory(2);
    let device = wgpu::Limits::default();
    let error = Shape::with_control(&theory, &device, 0, &Cancellation::default())
        .err()
        .unwrap();
    assert_eq!(error.kind(), GpuErrorKind::Capacity);
    assert!(error.to_string().contains("numeric directory"));
    let mut shape = Shape::with_control(&theory, &device, 1024, &Cancellation::default()).unwrap();
    assert!(shape.rows.bytes().unwrap() > 0);
    shape.rows.release();
    assert_eq!(shape.rows.bytes().unwrap(), 0);
}
