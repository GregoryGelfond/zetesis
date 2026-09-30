//! Closed objective conditions read full models without supplying support.

use crate::support::programs::program;
use zetesis_core::{Atom, AtomPattern, Model, Predicate, Sign, Term, Value};
use zetesis_cpu::Cancellation;
use zetesis_objective::{
    AdmissionError, AdmissionLimits, AdmissionResource, Condition, ConditionNode as Node,
    ErrorKind, Limits, ObjectiveProgram, ObjectiveTemplate, Stop, evaluate,
};
use zetesis_test_support::programs::nullary as atom;

fn row(condition: Vec<Node>) -> ObjectiveTemplate {
    ObjectiveTemplate::new(
        Term::Constant(Value::Number(2)),
        7,
        vec![Term::Constant(Value::Symbol("key".into()))],
        vec![],
        vec![],
    )
    .with_condition(Condition::new(condition))
}

#[test]
fn closed_conditions_follow_the_original_model() {
    let query = program(vec![row(vec![
        Node::Atom(atom("p")),
        Node::Atom(atom("q")),
        Node::Not(1),
        Node::And(0, 2),
        Node::Atom(atom("r")),
        Node::Or(3, 4),
    ])]);
    for mask in 0..8 {
        let p = mask & 1 != 0;
        let q = mask & 2 != 0;
        let r = mask & 4 != 0;
        let model = Model::new(
            ["p", "q", "r"]
                .into_iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, name)| atom(name)),
        )
        .unwrap();
        let before = model.clone();
        let result = evaluate(&query, &model, Limits::default(), &Cancellation::default()).unwrap();
        assert_eq!(
            result.score().costs(),
            &[(7, if (p && !q) || r { 2 } else { 0 })]
        );
        assert_eq!(model, before);
        assert_eq!(query.priorities(), &[7]);
    }
}

#[test]
fn absent_queries_retain_zero_priority_slots() {
    let query = program(vec![row(vec![Node::Atom(atom("outside"))])]);
    let result_model = Model::new([]).unwrap();
    let result = evaluate(
        &query,
        &result_model,
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(result.score().is_present());
    assert_eq!(result.score().costs(), &[(7, 0)]);
    assert!(result.contributions().is_empty());
}

#[test]
fn closed_and_lifted_rows_share_complete_keys() {
    let lifted = ObjectiveTemplate::new(
        Term::Constant(Value::Number(2)),
        7,
        vec![Term::Constant(Value::Symbol("key".into()))],
        vec![AtomPattern::new(Predicate::new("p", 0).unwrap(), vec![]).unwrap()],
        vec![],
    );
    let query = program(vec![lifted, row(vec![Node::Atom(atom("q")), Node::Not(0)])]);
    let result_model = Model::new([atom("p")]).unwrap();
    let result = evaluate(
        &query,
        &result_model,
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(result.score().costs(), &[(7, 2)]);
    assert_eq!(result.contributions().len(), 1);
    assert_eq!(result.statistics().duplicates, 1);
}

#[test]
fn condition_atoms_preserve_signed_value_identity() {
    let negative = Atom::new(
        Predicate::with_sign("p", 1, Sign::Negative).unwrap(),
        vec![Value::Number(1)],
    )
    .unwrap();
    let query = program(vec![row(vec![Node::Atom(negative.clone())])]);
    for (model, expected) in [
        (Model::new([negative]).unwrap(), 2),
        (
            Model::new([
                Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(1)]).unwrap(),
            ])
            .unwrap(),
            0,
        ),
        (
            Model::new([Atom::new(
                Predicate::with_sign("p", 1, Sign::Negative).unwrap(),
                vec![Value::String("1".into())],
            )
            .unwrap()])
            .unwrap(),
            0,
        ),
    ] {
        let result = evaluate(&query, &model, Limits::default(), &Cancellation::default()).unwrap();
        assert_eq!(result.score().costs(), &[(7, expected)]);
    }
}

#[test]
fn condition_admission_rejects_nonpreceding_operands() {
    for node in [Node::Not(0), Node::And(0, 1), Node::Or(usize::MAX, 0)] {
        let error =
            ObjectiveProgram::new(vec![row(vec![node])], AdmissionLimits::default()).unwrap_err();
        assert!(matches!(
            error,
            AdmissionError::ConditionReference {
                template: 0,
                node: 0,
                ..
            }
        ));
        assert_eq!(error.template_index(), Some(0));
        assert!(error.to_string().contains("condition node 0"));
    }
}

#[test]
fn condition_node_limits_are_inclusive() {
    let row = row(vec![Node::Boolean(false), Node::Not(0)]);
    let limits = AdmissionLimits {
        max_condition_nodes: 2,
        ..AdmissionLimits::default()
    };
    assert!(ObjectiveProgram::new(vec![row.clone()], limits).is_ok());
    assert_eq!(
        ObjectiveProgram::new(
            vec![row.clone()],
            AdmissionLimits {
                max_condition_nodes: 1,
                ..limits
            }
        )
        .unwrap_err(),
        AdmissionError::Limit {
            resource: AdmissionResource::ConditionNodes,
            template: Some(0),
            actual: 2,
            limit: 1
        },
    );
    assert!(ObjectiveProgram::new(vec![row], limits).is_ok());
}

#[test]
fn condition_work_refusal_returns_no_score() {
    let query = program(vec![row(vec![Node::Atom(atom("p")), Node::Not(0)])]);
    let model = Model::new([atom("other")]).unwrap();
    let complete = evaluate(&query, &model, Limits::default(), &Cancellation::default()).unwrap();
    let exact = complete.statistics().work;
    let limits = Limits {
        max_work: exact,
        ..Limits::default()
    };
    assert_eq!(
        evaluate(&query, &model, limits, &Cancellation::default())
            .unwrap()
            .score(),
        complete.score()
    );
    let error = evaluate(
        &query,
        &model,
        Limits {
            max_work: exact - 1,
            ..limits
        },
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Stopped(Stop::WorkLimit));
    assert_eq!(error.statistics().work, exact - 1);
    assert_eq!(
        evaluate(&query, &model, limits, &Cancellation::default())
            .unwrap()
            .score(),
        complete.score()
    );
}
