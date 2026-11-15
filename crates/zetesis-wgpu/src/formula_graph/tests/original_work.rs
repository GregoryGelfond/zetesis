use super::*;
use zetesis_ferraris::{FormulaNodes, NumericDomain, NumericRow, ThresholdDescriptor};

fn family(weights: &[u64], bounds: &[i128]) -> Theory {
    let mut nodes = FormulaNodes::new(FormulaParts::new(vec![Node::atom(0)], vec![]).unwrap());
    let mut transaction = nodes.transaction();
    let roots = transaction
        .push_threshold_family(
            &vec![0; weights.len()],
            weights,
            bounds,
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    transaction.commit();
    Theory::new(1, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap()
}

#[test]
fn original_row_discount_preserves_small_and_single_guard_tariffs() {
    // Independent expected tariffs: legacy g*(48n+32), shared 48n+32+68g+8.
    let cases: [(usize, u32, u32); 6] = [
        (1, 2, 0),
        (2, 2, 0),
        (3, 2, 32),
        (4, 2, 80),
        (4, 4, 392),
        (4, 1, 0),
    ];
    for (length, guards, expected) in cases {
        let bounds: Vec<_> = (1..=guards).map(i128::from).collect();
        let theory = family(&vec![16; length], &bounds);
        let shape = Shape::new(&theory, &wgpu::Limits::default()).unwrap();
        assert_eq!(shape.original_discount, expected);
        assert_eq!(
            u64::from(shape.numeric_work),
            u64::from(guards) * (48 * u64::try_from(length).unwrap() + 32)
        );
        assert_eq!(shape.rows.live_rows(), 1);
    }
}

#[test]
fn original_discount_keeps_portable_width_and_complete_truth() {
    for (weights, width) in [
        ([u64::from(u32::MAX) - 1, 1, 0], 1),
        ([u64::from(u32::MAX), 1, 0], 2),
        ([u64::MAX, 1, 0], 4),
    ] {
        let total = weights.iter().copied().map(i128::from).sum::<i128>();
        let theory = family(&weights, &[0, 1, total, total + 1]);
        let shape = Shape::new(&theory, &wgpu::Limits::default()).unwrap();
        assert_eq!((shape.numeric_work, shape.original_discount), (352, 32));
        let mut words = Vec::with_capacity(usize::try_from(shape.bytes / 4).unwrap());
        pack(&theory, &shape.rows, &mut words, &Cancellation::default()).unwrap();
        let tail = usize::try_from(shape.nodes).unwrap() * 4;
        let guard = theory.roots()[1];
        let row = tail
            + usize::try_from(words[tail + usize::try_from(words[guard * 4 + 1]).unwrap()])
                .unwrap();
        assert_eq!(words[row + 2], width);
        for candidate in 0..2 {
            let truth = physical_truth(&words, shape.nodes, candidate, None);
            assert_eq!(
                theory
                    .roots()
                    .iter()
                    .map(|root| truth[*root])
                    .collect::<Vec<_>>(),
                [true, candidate != 0, candidate != 0, false]
            );
        }
    }
}

#[test]
fn original_discount_uses_live_source_rows_including_unreachable_guards() {
    // Equal contents at distinct row IDs remain separate; the third raw row has
    // no guard and is never executed. All guards are unreachable from atom root0.
    let theory = Theory::new(
        1,
        FormulaParts::from_numeric_raw(
            vec![
                Node::atom(0),
                Node::threshold(0),
                Node::threshold(1),
                Node::threshold(2),
                Node::threshold(3),
                Node::threshold(4),
            ],
            vec![],
            vec![0; 9],
            vec![16; 9],
            (0..3)
                .map(|index| NumericRow {
                    start: index * 3,
                    length: 3,
                    domain: NumericDomain::Weights,
                })
                .collect(),
            vec![
                ThresholdDescriptor { row: 0, bound: 0 },
                ThresholdDescriptor { row: 0, bound: 1 },
                ThresholdDescriptor { row: 0, bound: 2 },
                ThresholdDescriptor { row: 1, bound: 1 },
                ThresholdDescriptor { row: 1, bound: 49 },
            ],
            vec![],
        )
        .unwrap(),
        vec![0],
        AdmissionLimits::default(),
    )
    .unwrap();
    let shape = Shape::new(&theory, &wgpu::Limits::default()).unwrap();
    assert_eq!((shape.numeric_work, shape.original_discount), (528, 32));
    assert_eq!(shape.rows.live_rows(), 2);
    assert_eq!(
        shape.rows.owners(&Cancellation::default()).unwrap(),
        [(0, 2), (1, 4)]
    );
}

#[test]
fn original_discount_keeps_constant_only_refusal_and_cancellation_boundaries() {
    let constants = family(&[1, 2], &[0, 4]);
    let shape = Shape::with_control(
        &constants,
        &wgpu::Limits::default(),
        0,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(
        (
            shape.numeric_work,
            shape.original_discount,
            shape.rows.bytes().unwrap()
        ),
        (0, 0, 0)
    );
    let live = family(&[16; 4], &[1, 2]);
    let refused = Shape::with_control(&live, &wgpu::Limits::default(), 0, &Cancellation::default())
        .err()
        .unwrap();
    assert_eq!(refused.kind(), GpuErrorKind::Capacity);
    assert!(refused.to_string().contains("numeric directory"));
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let stopped = Shape::with_control(&live, &wgpu::Limits::default(), u64::MAX, &cancellation)
        .err()
        .unwrap();
    assert_eq!(
        stopped.interruption(),
        Some(zetesis_algebra::Stop::Cancelled)
    );
}
