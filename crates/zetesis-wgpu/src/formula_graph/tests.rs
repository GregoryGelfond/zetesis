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
    let mut packed = Vec::new();
    let mut tail = 0;
    for index in 0..theory.view().len() {
        packed.extend(header(theory.view().node(index).unwrap(), &mut tail).unwrap());
    }
    append_operands(&theory, &mut packed, &Cancellation::default()).unwrap();
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
        append_operands(&theory(), &mut Vec::new(), &cancellation)
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
        let mut map = Vec::with_capacity(shape.map);
        let mut scratch = Vec::with_capacity(shape.scratch);
        pack(&theory, &mut words, &mut map, &mut scratch, &cancellation).unwrap();
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
            for (index, &physical) in map.iter().enumerate() {
                assert_eq!(
                    original[physical as usize],
                    truth.node_truth(index).unwrap()
                );
            }
            for tested in 0..4 {
                // Every interpretation is tested, including those outside M.
                let reduct = physical_truth(&words, shape.nodes, tested, Some(&original));
                let actual = theory
                    .roots()
                    .iter()
                    .all(|&root| reduct[map[root] as usize]);
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
    assert!(shape.nodes as usize > theory.view().len());
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
        Shape::with_control(&native_theory(2), &wgpu::Limits::default(), &cancellation)
            .err()
            .unwrap()
            .interruption(),
        Some(zetesis_cpu::Stop::Cancelled)
    );
}
