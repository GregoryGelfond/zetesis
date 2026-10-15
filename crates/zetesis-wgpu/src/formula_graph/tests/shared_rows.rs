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
    assert_eq!(&words[first..first + 2], &[3, 2]);
    assert_eq!(&words[first + 11..first + 13], &[3, 6]);
    assert_eq!(&words[row(5)..row(5) + 2], &[5, 1]);
    assert_eq!(words[2 * 4], 10, "constant guard is not a row owner");
    assert_eq!(&words[7 * 4..8 * 4], &[4, 6, 1, 0]);
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
            let guards = row + 2 + 3 * node[2] as usize;
            for &guard in &words[guards..guards + words[row + 1] as usize] {
                assert_eq!(words[guard as usize * 4 + 3], depth);
            }
            for &child in &words[row + 2..row + 2 + node[2] as usize] {
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
