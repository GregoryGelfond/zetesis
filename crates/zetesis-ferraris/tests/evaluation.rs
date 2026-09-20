//! Original truth is reusable only with its evaluated subject and complete scan.

use std::time::Instant;

use zetesis_cpu::{Control, Stop};
use zetesis_ferraris::{
    AdmissionLimits, EvaluationError, EvaluationLimits, EvaluationWorkspace, Interpretation, Node,
    Theory,
};

fn theory(roots: Vec<usize>) -> Theory {
    // a, b, false, not a, a or b, (a or b) and not a, b -> a.
    Theory::new(
        2,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::False,
            Node::Implies(0, 2),
            Node::Or(0, 1),
            Node::And(4, 3),
            Node::Implies(1, 0),
        ],
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn node_truth_describes_the_exact_interpretation() {
    let program = theory(vec![4, 6]);
    let mut workspace = EvaluationWorkspace::default();
    for bits in 0_u8..4 {
        let interpretation =
            Interpretation::new(&program, (0..2).filter(|atom| bits & (1 << atom) != 0)).unwrap();
        let attempt = workspace.evaluate(
            &interpretation,
            EvaluationLimits::default(),
            &Control::default(),
        );
        let truth = attempt.result.unwrap();
        let a = bits & 1 != 0;
        let b = bits & 2 != 0;
        let expected = [a, b, false, !a, a || b, !a && b, !b || a];
        for (node, value) in expected.into_iter().enumerate() {
            assert_eq!(truth.node_truth(node), Some(value));
        }
        assert_eq!(truth.node_truth(7), None);
        assert!(std::ptr::eq(
            truth.interpretation(),
            &raw const interpretation
        ));
        assert!(truth.theory().same_instance(&program));
        // The two asserted formulas together are equivalent to a.
        assert_eq!(truth.is_model(), a);
        assert_eq!(
            truth.failed_root(),
            if !a && !b {
                Some(4)
            } else if b && !a {
                Some(6)
            } else {
                None
            }
        );
        assert_eq!(attempt.work, if a || b { 9 } else { 8 });
    }
}

#[test]
fn root_scan_exhaustion_exposes_no_truth() {
    let program = theory(vec![4, 6]);
    let candidate = Interpretation::new(&program, [0]).unwrap();
    let mut workspace = EvaluationWorkspace::default();
    for max_work in 0..9 {
        let attempt = workspace.evaluate(
            &candidate,
            EvaluationLimits {
                max_work,
                ..EvaluationLimits::default()
            },
            &Control::default(),
        );
        assert_eq!(
            attempt.result.unwrap_err(),
            EvaluationError::Stopped(Stop::WorkLimit)
        );
        assert_eq!(attempt.work, max_work);
    }
    let attempt = workspace.evaluate(
        &candidate,
        EvaluationLimits {
            max_work: 9,
            ..EvaluationLimits::default()
        },
        &Control::default(),
    );
    assert!(attempt.result.unwrap().is_model());
    assert_eq!(attempt.work, 9);
}

#[test]
fn workspace_reuse_replaces_the_subject() {
    let first = theory(vec![4]);
    let other = theory(vec![5]);
    let one = Interpretation::new(&first, [0]).unwrap();
    let two = Interpretation::new(&other, [1]).unwrap();
    let mut workspace = EvaluationWorkspace::default();
    let bytes = {
        let attempt = workspace.evaluate(&one, EvaluationLimits::default(), &Control::default());
        assert!(attempt.result.unwrap().is_model());
        attempt.retained_bytes
    };
    let attempt = workspace.evaluate(&two, EvaluationLimits::default(), &Control::default());
    let truth = attempt.result.unwrap();
    assert!(truth.theory().same_instance(&other));
    assert!(!truth.theory().same_instance(&first));
    assert_eq!(truth.node_truth(0), Some(false));
    assert!(truth.is_model());
    assert_eq!(attempt.retained_bytes, bytes);
}

#[test]
fn reserved_capacity_does_not_supply_original_truth() {
    let program = theory(vec![5]);
    let mut workspace = EvaluationWorkspace::default();
    workspace
        .reserve(&program, usize::MAX, &Control::default())
        .unwrap();
    let bytes = workspace.retained_bytes();
    let candidate = Interpretation::new(&program, [1]).unwrap();
    let stopped = workspace.evaluate(
        &candidate,
        EvaluationLimits {
            max_work: 0,
            ..EvaluationLimits::default()
        },
        &Control::default(),
    );
    assert_eq!(
        stopped.result.unwrap_err(),
        EvaluationError::Stopped(Stop::WorkLimit)
    );
    assert_eq!(stopped.retained_bytes, bytes);
    let complete = workspace.evaluate(&candidate, EvaluationLimits::default(), &Control::default());
    assert!(complete.result.unwrap().is_model());
    assert_eq!(complete.work, 8);
    assert_eq!(complete.retained_bytes, bytes);
}

#[test]
fn storage_ceiling_includes_existing_capacity() {
    let large = theory(vec![]);
    let empty = Theory::new(0, vec![], vec![], AdmissionLimits::default()).unwrap();
    let candidate = Interpretation::new(&large, []).unwrap();
    let nothing = Interpretation::new(&empty, []).unwrap();
    let mut workspace = EvaluationWorkspace::default();
    let header = usize::try_from(workspace.retained_bytes()).unwrap();
    let required = header + large.nodes().len() * size_of::<bool>();
    let limited = workspace.evaluate(
        &candidate,
        EvaluationLimits {
            max_bytes: required - 1,
            ..EvaluationLimits::default()
        },
        &Control::default(),
    );
    assert_eq!(
        limited.result.unwrap_err(),
        EvaluationError::Storage {
            required: required as u128,
            limit: required - 1,
        }
    );
    assert_eq!(limited.work, 0);
    assert_eq!(limited.retained_bytes, header as u128);
    let admitted = workspace.evaluate(&candidate, EvaluationLimits::default(), &Control::default());
    assert!(admitted.result.unwrap().is_model());
    let retained = usize::try_from(admitted.retained_bytes).unwrap();
    let lowered = workspace.evaluate(
        &nothing,
        EvaluationLimits {
            max_bytes: retained - 1,
            ..EvaluationLimits::default()
        },
        &Control::default(),
    );
    assert_eq!(
        lowered.result.unwrap_err(),
        EvaluationError::Storage {
            required: retained as u128,
            limit: retained - 1,
        }
    );
    assert_eq!(lowered.retained_bytes, retained as u128);
    let exact = workspace.evaluate(
        &nothing,
        EvaluationLimits {
            max_work: 0,
            max_bytes: retained,
        },
        &Control::default(),
    );
    assert!(exact.result.unwrap().is_model());
    assert_eq!(exact.work, 0);
}

#[test]
fn empty_evaluation_observes_control_before_storage() {
    let program = Theory::new(0, vec![], vec![], AdmissionLimits::default()).unwrap();
    let candidate = Interpretation::new(&program, []).unwrap();
    let cancelled = Control::default();
    cancelled.cancel();
    let mut workspace = EvaluationWorkspace::default();
    for (control, stop) in [
        (cancelled, Stop::Cancelled),
        (
            Control::with_deadline(Instant::now()).unwrap(),
            Stop::Deadline,
        ),
    ] {
        let attempt = workspace.evaluate(
            &candidate,
            EvaluationLimits {
                max_work: 0,
                max_bytes: 0,
            },
            &control,
        );
        assert_eq!(attempt.result.unwrap_err(), EvaluationError::Stopped(stop));
        assert_eq!(attempt.work, 0);
    }
}
