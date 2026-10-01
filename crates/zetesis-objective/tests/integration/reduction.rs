//! Numeric reduction consumes established key eligibility without reconstructing evidence.

use zetesis_core::{Term, Value};
use zetesis_cpu::Cancellation;
use zetesis_objective::{ErrorKind, ObjectiveProgram, ObjectiveTemplate, Stop, reduce_costs};

fn program() -> ObjectiveProgram {
    super::support::programs::program(
        [7, 2, 0]
            .into_iter()
            .map(|priority| {
                ObjectiveTemplate::new(
                    Term::Constant(Value::Number(0)),
                    priority,
                    vec![],
                    vec![],
                    vec![],
                )
            })
            .collect(),
    )
}

fn keys(priority: i32) -> Vec<Option<i32>> {
    match priority {
        7 => vec![None],
        2 => vec![Some(-3), Some(-3), None, Some(5)],
        0 => vec![Some(i32::MIN), Some(i32::MAX)],
        _ => panic!("unexpected priority"),
    }
}

#[test]
fn reduction_preserves_the_fixed_priority_score() {
    let score = reduce_costs(&program(), keys, u64::MAX, &Cancellation::default())
        .unwrap()
        .into_score();
    assert!(score.is_present());
    assert_eq!(score.costs(), [(7, 0), (2, -1), (0, -1)]);
}

#[test]
fn empty_reduction_preserves_objective_presence() {
    for program in [
        ObjectiveProgram::none(),
        super::support::programs::program(vec![]),
    ] {
        let score = reduce_costs(&program, |_| Vec::new(), 1, &Cancellation::default())
            .unwrap()
            .into_score();
        assert_eq!(score.is_present(), program.is_present());
        assert!(score.costs().is_empty());
    }
}

#[test]
fn every_reduction_cutoff_retains_its_work_prefix() {
    let program = program();
    let cancellation = Cancellation::default();
    let completed = reduce_costs(&program, keys, u64::MAX, &cancellation).unwrap();
    for cutoff in 0..completed.work() {
        let error = reduce_costs(&program, keys, cutoff, &cancellation).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Stopped(Stop::WorkLimit));
        assert_eq!(
            error.statistics(),
            zetesis_objective::Statistics {
                work: cutoff,
                ..Default::default()
            }
        );
        assert_eq!(error.template_index(), None);
    }
    let exact = reduce_costs(&program, keys, completed.work(), &cancellation).unwrap();
    assert_eq!(exact.work(), completed.work());
    assert_eq!(exact.into_score(), completed.into_score());
}

#[test]
fn refused_work_does_not_visit_another_key() {
    let visited = std::cell::Cell::new(0);
    let error = reduce_costs(
        &program(),
        |_| {
            std::iter::from_fn(|| {
                visited.set(visited.get() + 1);
                Some(Some(1))
            })
        },
        2,
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Stopped(Stop::WorkLimit));
    assert_eq!(visited.get(), 0);
}

#[test]
fn cancellation_prevents_reduction_publication() {
    let cancellation = Cancellation::default();
    let program = super::support::programs::program(vec![ObjectiveTemplate::new(
        Term::Constant(Value::Number(1)),
        0,
        vec![],
        vec![],
        vec![],
    )]);
    let error = reduce_costs(
        &program,
        |_| {
            std::iter::once_with(|| {
                cancellation.cancel();
                Some(1)
            })
        },
        u64::MAX,
        &cancellation,
    )
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Stopped(Stop::Cancelled));
    assert_eq!(error.statistics().work, 3);
}
