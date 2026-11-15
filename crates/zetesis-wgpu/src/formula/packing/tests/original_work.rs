use super::*;
use zetesis_ferraris::{FormulaNodes, FormulaParts};

fn shared() -> Theory {
    let mut nodes = FormulaNodes::new(FormulaParts::new(vec![Node::atom(0)], vec![]).unwrap());
    let mut transaction = nodes.transaction();
    let roots = transaction
        .push_threshold_family(&[0; 4], &[16; 4], &[1, 2], usize::MAX, usize::MAX)
        .unwrap();
    transaction.commit();
    Theory::new(1, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap()
}

#[test]
fn shared_original_setup_has_exact_inclusive_work_and_byte_admission() {
    let theory = shared();
    let shape = Shape::new(&theory, &device()).unwrap();
    // Three nodes, eight edges, one atom, two roots and projection1+2.
    // Legacy numeric448 becomes368; original evaluation still executes.
    assert_eq!(shape.setup_work, 388);
    assert_eq!(shape.sweep_work, 477, "G leaves S frozen sweeps unchanged");
    let schedule = Schedule::new(&shape, 0, &device()).unwrap();
    for (ceiling, expected) in [(387, false), (388, true), (389, true)] {
        let result = Plan::layout(
            &shape,
            &schedule,
            1,
            FormulaLimits {
                max_work_per_candidate: ceiling,
                ..Default::default()
            },
            &device(),
            true,
            1,
        );
        assert_eq!(result.is_ok(), expected);
        if let Err(error) = result {
            assert_eq!(error.kind(), GpuErrorKind::Capacity);
            assert!(
                error
                    .to_string()
                    .contains("mandatory formula setup or epoch")
            );
        }
    }
    let admitted = Plan::layout(
        &shape,
        &schedule,
        1,
        FormulaLimits::default(),
        &device(),
        true,
        1,
    )
    .unwrap();
    for (bytes, expected) in [(admitted.accounted - 1, false), (admitted.accounted, true)] {
        assert_eq!(
            Plan::layout(
                &shape,
                &schedule,
                1,
                FormulaLimits {
                    max_batch_bytes: bytes,
                    ..Default::default()
                },
                &device(),
                true,
                1
            )
            .is_ok(),
            expected
        );
    }
    assert_eq!(
        Plan::layout(
            &shape,
            &schedule,
            1,
            FormulaLimits::default(),
            &device(),
            true,
            0
        )
        .err()
        .unwrap()
        .kind(),
        GpuErrorKind::Capacity
    );
}
