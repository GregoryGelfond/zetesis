use super::*;
use zetesis_ferraris::{NumericDomain, NumericRow, ThresholdDescriptor};

fn interleaved() -> Theory {
    Theory::new(
        2,
        FormulaParts::from_numeric_raw(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::threshold(0),
                Node::threshold(1),
                Node::implies(3, 1),
                Node::threshold(2),
                Node::threshold(3),
                Node::implies(6, 1),
            ],
            vec![],
            vec![0, 0, 1, 0, 0, 1],
            vec![u64::MAX, 1, 0, u64::MAX, 1, 0],
            vec![
                NumericRow {
                    start: 0,
                    length: 3,
                    domain: NumericDomain::Weights,
                },
                NumericRow {
                    start: 3,
                    length: 3,
                    domain: NumericDomain::Weights,
                },
            ],
            vec![
                ThresholdDescriptor { row: 0, bound: 0 },
                ThresholdDescriptor { row: 0, bound: 1 },
                ThresholdDescriptor { row: 1, bound: 1 },
                ThresholdDescriptor {
                    row: 0,
                    bound: i128::from(u64::MAX) + 1,
                },
            ],
            vec![],
        )
        .unwrap(),
        vec![7],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn packed(theory: &Theory) -> (Shape, Vec<u32>) {
    let shape = Shape::new(theory, &wgpu::Limits::default()).unwrap();
    let mut words = Vec::with_capacity(usize::try_from(shape.bytes / 4).unwrap());
    pack(theory, &shape.rows, &mut words, &Cancellation::default()).unwrap();
    (shape, words)
}

#[test]
fn row_owners_preserve_source_identity_and_original_ids() {
    let (shape, words) = packed(&interleaved());
    let tail = shape.nodes as usize * 4;
    let row = |node: usize| tail + words[tail + words[node * 4 + 1] as usize] as usize;
    let first = row(3);
    assert_eq!(row(6), first);
    assert_ne!(
        row(5),
        first,
        "equal contents do not merge distinct admitted rows"
    );
    assert_eq!(&words[first..first + 3], &[3, 2, 4]);
    assert_eq!(&words[first + 12..first + 14], &[3, 6]);
    assert_eq!(&words[row(5)..row(5) + 3], &[5, 1, 4]);
    assert_eq!(words[2 * 4], 10, "constant guard is not a row owner");
    assert_eq!(&words[7 * 4..8 * 4], &[4, 6, 1, 0]);
}

#[test]
fn transported_width_covers_each_live_threshold_family() {
    use zetesis_ferraris::FormulaNodes;

    for (weights, width) in [
        ([u64::from(u32::MAX) - 1, 1, 0], 1),
        ([u64::from(u32::MAX), 1, 0], 2),
        ([u64::MAX - 1, 1, 0], 2),
        ([u64::MAX, 1, 0], 4),
    ] {
        let total: i128 = weights.iter().copied().map(i128::from).sum();
        let mut nodes = FormulaNodes::new(
            FormulaParts::new(vec![Node::atom(0), Node::atom(1)], vec![]).unwrap(),
        );
        let mut transaction = nodes.transaction();
        let roots = transaction
            .push_threshold_family(
                &[0, 1, 0],
                &weights,
                &[0, 1, total, total + 1],
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        transaction.commit();
        let theory = Theory::new(2, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap();
        let (shape, words) = packed(&theory);
        let tail = shape.nodes as usize * 4;
        let mut live = 0;
        for &root in theory.roots() {
            if words[root * 4] == 7 {
                let row = tail + words[tail + words[root * 4 + 1] as usize] as usize;
                assert_eq!(words[row + 2], width);
                live += 1;
            }
        }
        assert_eq!(live, 2);
        for bits in 0..4 {
            let truth = physical_truth(&words, shape.nodes, bits, None);
            let sum: i128 = weights
                .iter()
                .zip([0, 1, 0])
                .filter(|(_, atom)| bits & (1 << atom) != 0)
                .map(|(&weight, _)| i128::from(weight))
                .sum();
            for (&root, bound) in theory.roots().iter().zip([0, 1, total, total + 1]) {
                assert_eq!(truth[root], sum >= bound);
            }
        }
    }
}

#[test]
fn live_guards_share_one_dependency_level() {
    for theory in [interleaved(), ordinal_theory()] {
        let (shape, mut words) = packed(&theory);
        dependency::label_depths(
            shape.nodes,
            shape.wide_words,
            &mut words,
            &Cancellation::default(),
        )
        .unwrap();
        let tail = shape.nodes as usize * 4;
        for (index, node) in words.chunks_exact(4).take(shape.nodes as usize).enumerate() {
            if !(7..=9).contains(&node[0]) {
                continue;
            }
            let row = tail + words[tail + node[1] as usize] as usize;
            let owner = words[row] as usize;
            assert!(owner <= index);
            let depth = words[owner * 4 + 3];
            let guards = row + 3 + 3 * node[2] as usize;
            for &guard in &words[guards..guards + words[row + 1] as usize] {
                assert_eq!(words[guard as usize * 4 + 3], depth);
            }
            for &child in &words[row + 3..row + 3 + node[2] as usize] {
                assert!(words[child as usize * 4 + 3] < depth);
            }
        }
    }
}

#[test]
fn shared_rows_keep_original_and_frozen_truth() {
    use zetesis_ferraris::{Interpretation, Limits};
    let theory = interleaved();
    let (shape, words) = packed(&theory);
    let interpretation =
        |mask| Interpretation::new(&theory, (0..2).filter(|atom| mask & (1 << atom) != 0)).unwrap();
    for candidate in 0..4 {
        let original = physical_truth(&words, shape.nodes, candidate, None);
        for tested in 0..4 {
            let truth = physical_truth(&words, shape.nodes, tested, Some(&original));
            assert_eq!(
                truth[7],
                zetesis_ferraris::models_reduct(
                    &theory,
                    &interpretation(candidate),
                    &interpretation(tested),
                    Limits::default(),
                    &Cancellation::default(),
                )
                .unwrap()
            );
        }
    }
}

#[test]
fn live_directory_admits_its_exact_capacity() {
    let theory = interleaved();
    let device = wgpu::Limits::default();
    let bytes = Shape::new(&theory, &device).unwrap().rows.bytes().unwrap();
    for (limit, admitted) in [(bytes - 1, false), (bytes, true)] {
        let shape = Shape::with_control(&theory, &device, limit, &Cancellation::default());
        assert_eq!(shape.is_ok(), admitted);
        if let Err(error) = shape {
            assert_eq!(error.kind(), GpuErrorKind::Capacity);
        }
    }
}

#[test]
fn constant_only_fallback_matches_prepared_summaries() {
    for bound in [7, i128::MAX - 1] {
        let theory = native_theory(bound);
        let (shape, expected) = packed(&theory);
        assert_eq!(shape.wide_words, 0);
        assert_eq!(shape.rows.bytes().unwrap(), 0);
        let fallback = Shape::with_control(
            &theory,
            &wgpu::Limits::default(),
            0,
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(fallback.rows.bytes().unwrap(), 0);
        let mut actual = Vec::with_capacity(usize::try_from(fallback.bytes / 4).unwrap());
        pack(
            &theory,
            &fallback.rows,
            &mut actual,
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }
}
