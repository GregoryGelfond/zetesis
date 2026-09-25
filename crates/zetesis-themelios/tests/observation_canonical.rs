//! Canonical observation execution across independently admitted input owners.
use zetesis_core::{Atom, Model, Predicate, Value};
use zetesis_cpu::Cancellation;
use zetesis_themelios::observation::{ErrorKind, Limits, Resource};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

fn source(text: &str) -> zetesis_themelios::AdmittedFormula {
    admit_formula(
        text.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}
#[test]
fn foreign_model_values_keep_their_types_in_output() {
    let input = source("#show. #show X:p(X).");
    let predicate = Predicate::new("p", 1).unwrap();
    let model = Model::new(
        [
            Value::Symbol("same".into()),
            Value::String("same".into()),
            Value::Number(7),
        ]
        .into_iter()
        .map(|value| Atom::new(predicate.clone(), vec![value]).unwrap()),
    )
    .unwrap();
    let shown = input
        .metadata()
        .observations()
        .render(
            &model,
            input.metadata().output(),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(shown.text(), "7 same \"same\"");
}
#[test]
fn named_term_capacity_has_an_inclusive_ceiling() {
    let input = source("#show. #show f(X):X=1..3.");
    let model = Model::default();
    let cancellation = Cancellation::default();
    let run = |max_term_storage_bytes| {
        input.metadata().observations().evaluate(
            &model,
            Limits {
                max_term_storage_bytes,
                ..Limits::default()
            },
            &cancellation,
        )
    };
    let complete = run(Limits::default().max_term_storage_bytes).unwrap();
    let peak = usize::try_from(complete.statistics().peak_term_storage_bytes).unwrap();
    assert!(peak > 0);
    assert_eq!(run(peak).unwrap().symbols(), complete.symbols());
    assert!(
        matches!(run(peak-1),Err(error) if matches!(error.kind(),ErrorKind::Limit{resource:Resource::TermStorageBytes,..}))
    );
}
#[test]
fn anonymous_keys_have_no_tag_value_collisions() {
    let input = source("#show. #show N:N={not p(_);not p((0,));not p((1,0));not p((2,0))}.");
    let shown = input
        .metadata()
        .observations()
        .render(
            &Model::default(),
            input.metadata().output(),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(shown.text(), "4");
}
#[test]
fn every_observation_work_cutoff_refuses_complete_output() {
    let input = source("p(1).p(2). #show. #show f(N):N=#count{X:p(X)}.");
    let model = Model::from_positions(input.atom_catalog(), 0..input.atoms().len()).unwrap();
    let cancellation = Cancellation::default();
    let run = |max_work| {
        input.metadata().observations().evaluate(
            &model,
            Limits {
                max_work,
                ..Limits::default()
            },
            &cancellation,
        )
    };
    let complete = run(Limits::default().max_work).unwrap();
    for cutoff in 0..complete.statistics().work {
        let error = run(cutoff).unwrap_err();
        assert!(matches!(
            error.kind(),
            ErrorKind::Limit {
                resource: Resource::Work,
                ..
            }
        ));
        assert!(error.statistics().work <= cutoff);
    }
}
