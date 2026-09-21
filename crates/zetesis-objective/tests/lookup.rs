//! Objective lookup preserves typed keys and checks only matching predicate rows.
//! Work controls distinguish complete absence from a refused comparison prefix.

use zetesis_core::{
    Atom, AtomPattern, Model, Predicate, Sign, Term, Value, ValueLimits, ValueNode,
};
use zetesis_cpu::Cancellation;
use zetesis_objective::{
    AdmissionLimits, Condition, ConditionNode, ErrorKind, Limits, ObjectiveProgram,
    ObjectiveTemplate, Stop, evaluate,
};

fn atom(name: &str, sign: Sign, values: Vec<Value>) -> Atom {
    Atom::new(
        Predicate::with_sign(name, values.len(), sign).unwrap(),
        values,
    )
    .unwrap()
}

fn values() -> Vec<Value> {
    vec![
        Value::Number(1),
        Value::Number(2),
        Value::String("x".into()),
        Value::Symbol("x".into()),
        Value::from_nodes(vec![ValueNode::Tuple { arity: 0 }], ValueLimits::default()).unwrap(),
    ]
}

fn model(noise: i32) -> Model {
    let mut atoms: Vec<_> = (0..noise)
        .map(|index| atom("a_noise", Sign::Positive, vec![Value::Number(index)]))
        .collect();
    for value in values() {
        atoms.push(atom(
            "p",
            Sign::Positive,
            vec![value.clone(), value.clone()],
        ));
        atoms.push(atom("q", Sign::Positive, vec![value]));
    }
    atoms.extend([
        atom(
            "p",
            Sign::Positive,
            vec![Value::Number(1), Value::Number(2)],
        ),
        atom(
            "p",
            Sign::Negative,
            vec![Value::Number(3), Value::Number(3)],
        ),
        atom("p", Sign::Positive, vec![Value::Number(3)]),
        atom("zz_gate", Sign::Negative, vec![]),
    ]);
    Model::new(atoms)
}

fn program() -> ObjectiveProgram {
    let template = ObjectiveTemplate::new(
        Term::Constant(Value::Number(2)),
        7,
        vec![Term::Variable(0)],
        vec![
            AtomPattern::new(Predicate::new("p", 2).unwrap(), vec![Term::Variable(0); 2]).unwrap(),
            AtomPattern::new(Predicate::new("q", 1).unwrap(), vec![Term::Variable(0)]).unwrap(),
        ],
        vec![],
    )
    .with_condition(Condition::new(vec![
        ConditionNode::Atom(atom("zz_gate", Sign::Negative, vec![])),
        ConditionNode::Atom(atom("zz_gate", Sign::Positive, vec![])),
        ConditionNode::Not(1),
        ConditionNode::And(0, 2),
    ]));
    ObjectiveProgram::new(vec![template.clone(), template], AdmissionLimits::default()).unwrap()
}

#[test]
fn predicate_windows_preserve_objective_contributions() {
    let program = program();
    let small = evaluate(
        &program,
        &model(100),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let large = evaluate(
        &program,
        &model(1_000),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(small.score().costs(), &[(7, 10)]);
    assert_eq!(small.score(), large.score());
    assert_eq!(small.contributions(), large.contributions());
    let tuples: Vec<_> = small
        .contributions()
        .iter()
        .map(|key| key.tuple().to_vec())
        .collect();
    assert_eq!(
        tuples,
        values()
            .into_iter()
            .map(|value| vec![value])
            .collect::<Vec<_>>()
    );
}

#[test]
fn predicate_windows_retain_duplicate_eligibility() {
    for size in [100, 1_000] {
        let result = evaluate(
            &program(),
            &model(size),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(result.statistics().bindings, 10);
        assert_eq!(result.statistics().duplicates, 5);
        assert_eq!(result.statistics().keys, 5);
    }
}

#[test]
fn lookup_work_avoids_scanning_unrelated_predicates() {
    let program = program();
    let small = evaluate(
        &program,
        &model(100),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let large = evaluate(
        &program,
        &model(1_000),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    // Ten times more unrelated atoms may deepen binary probes. They must not
    // produce ten times the old per-frame model scanning work.
    assert!(large.statistics().work < 2 * small.statistics().work);
}

#[test]
fn every_work_cutoff_retains_the_exact_prefix_without_a_score() {
    let program = program();
    let model = model(8);
    let cancellation = Cancellation::default();
    let complete = evaluate(&program, &model, Limits::default(), &cancellation).unwrap();
    let exact = complete.statistics().work;
    for limit in 0..exact {
        let error = evaluate(
            &program,
            &model,
            Limits {
                max_work: limit,
                ..Limits::default()
            },
            &cancellation,
        )
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Stopped(Stop::WorkLimit));
        assert_eq!(error.statistics().work, limit);
    }
    let repeated = evaluate(
        &program,
        &model,
        Limits {
            max_work: exact,
            ..Limits::default()
        },
        &cancellation,
    )
    .unwrap();
    assert_eq!(repeated.score(), complete.score());
    assert_eq!(repeated.contributions(), complete.contributions());
}
