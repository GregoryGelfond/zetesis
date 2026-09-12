//! Consuming completed closures preserves their owned interpretation storage.

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Model, Predicate, Program, Seed, Template, Term, Value,
};
use zetesis_cpu::{Control, Limits, check, lazy};

fn source(constraint: bool) -> Program {
    let head = AtomPattern::new(
        Predicate::new("message", 1).unwrap(),
        vec![Term::Constant(Value::String("retained payload".to_owned()))],
    )
    .unwrap();
    let mut rules = vec![Template::new(Some(head), vec![], vec![], vec![], vec![])];
    if constraint {
        rules.push(Template::new(None, vec![], vec![], vec![], vec![]));
    }
    Program::new(rules, AdmissionLimits::default()).unwrap()
}

fn expected() -> Model {
    Model::new([Atom::new(
        Predicate::new("message", 1).unwrap(),
        vec![Value::String("retained payload".to_owned())],
    )
    .unwrap()])
}

fn payload(model: &Model) -> &str {
    let atom = model.atoms().first().unwrap();
    let Value::String(value) = &atom.values()[0] else {
        panic!("fixture contains one string argument");
    };
    value
}

fn shared_check(constraint: bool) -> lazy::Check {
    let program = source(constraint);
    lazy::check_with(
        &program,
        &[Seed::new(&program, []).unwrap()],
        lazy::Limits::default(),
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap()
    .checks
    .pop()
    .unwrap()
}

#[test]
fn native_acceptance_transfers_payload_storage() {
    let program = source(false);
    let checked = check(
        &program,
        &Seed::new(&program, []).unwrap(),
        Limits::default(),
        &Control::default(),
    )
    .unwrap();
    let original = payload(checked.closure()).as_ptr();
    let model = checked
        .into_stable_interpretation()
        .unwrap()
        .into_interpretation();
    assert_eq!(model, expected());
    // Pointer identity witnesses transfer of this nonempty String allocation;
    // equal values alone would also accept an unnecessary clone.
    assert_eq!(payload(&model).as_ptr(), original);
}

#[test]
fn shared_acceptance_transfers_payload_storage() {
    let checked = shared_check(false);
    assert!(checked.accepted());
    let original = payload(checked.closure()).as_ptr();
    let model = checked.into_closure();
    assert_eq!(model, expected());
    assert_eq!(payload(&model).as_ptr(), original);
}

#[test]
fn rejected_shared_check_retains_its_raw_closure() {
    let checked = shared_check(true);
    assert!(!checked.accepted());
    assert!(checked.constraint_violated());
    assert_eq!(checked.into_closure(), expected());
}

#[test]
fn shared_closures_retain_one_finished_catalog() {
    let program = {
        let head = AtomPattern::new(Predicate::new("zchoice", 0).unwrap(), vec![]).unwrap();
        let mut templates = source(false).templates().to_vec();
        templates.push(Template::new(
            Some(head.clone()),
            vec![],
            vec![head],
            vec![],
            vec![],
        ));
        Program::new(templates, AdmissionLimits::default()).unwrap()
    };
    let chosen = Atom::new(Predicate::new("zchoice", 0).unwrap(), vec![]).unwrap();
    let checks = lazy::check_with(
        &program,
        &[
            Seed::new(&program, []).unwrap(),
            Seed::new(&program, [chosen.clone()]).unwrap(),
        ],
        lazy::Limits::default(),
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap()
    .checks;
    assert!(checks.iter().all(lazy::Check::accepted));
    let models: Vec<_> = checks.into_iter().map(lazy::Check::into_closure).collect();
    drop(program);
    assert_eq!(models[0], expected());
    assert_eq!(
        models[1],
        Model::new(expected().atoms().iter().cloned().chain([chosen]))
    );
    assert!(models[0].catalog().same_owner(models[1].catalog()));
    assert_eq!(payload(&models[0]).as_ptr(), payload(&models[1]).as_ptr());
}
