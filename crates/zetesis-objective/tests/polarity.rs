//! Signed normalization precedes global tuple identity and checked accumulation.
use zetesis_core::{Atom, AtomPattern, Model, Predicate, Term, Value};
use zetesis_cpu::Cancellation;
use zetesis_objective::{
    AdmissionLimits, ErrorKind, Limits, ObjectiveProgram, ObjectiveTemplate, Stop, WeightPolarity,
    evaluate,
};

fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms).unwrap()
}
fn atom(name: &str, values: Vec<Value>) -> Atom {
    Atom::new(Predicate::new(name, values.len()).unwrap(), values).unwrap()
}
fn row(weight: Term, priority: i32, key: &str, positive: Vec<AtomPattern>) -> ObjectiveTemplate {
    ObjectiveTemplate::new(
        weight,
        priority,
        vec![Term::Constant(Value::Symbol(key.into()))],
        positive,
        vec![],
    )
}
fn number(weight: i32) -> Term {
    Term::Constant(Value::Number(weight))
}
fn program(rows: Vec<ObjectiveTemplate>) -> ObjectiveProgram {
    ObjectiveProgram::new(rows, AdmissionLimits::default()).unwrap()
}
#[test]
fn normalization_has_one_checked_machine_boundary_and_keeps_default_api() {
    for value in [i32::MIN, i32::MIN + 1, -3, -1, 0, 1, 3, i32::MAX] {
        assert_eq!(WeightPolarity::AsWritten.normalize(value), Some(value));
        assert_eq!(
            WeightPolarity::Negated.normalize(value).map(i64::from),
            i32::try_from(-i64::from(value)).ok().map(i64::from)
        );
    }
    assert_eq!(
        row(number(3), 0, "k", vec![]).weight_polarity(),
        WeightPolarity::AsWritten
    );
    let source = row(number(3), 0, "k", vec![]).with_weight_polarity(WeightPolarity::Negated);
    assert_eq!(source.weight(), &number(3));
}

#[test]
fn signed_lifted_keys_coalesce_across_directions_before_adding_costs() {
    let maximize = row(
        Term::Variable(0),
        2,
        "k",
        vec![pattern("p", vec![Term::Variable(0)])],
    )
    .with_weight_polarity(WeightPolarity::Negated);
    let minimize = row(number(-2), 2, "k", vec![pattern("q", vec![])]);
    let zero = row(number(0), 7, "k", vec![]).with_weight_polarity(WeightPolarity::Negated);
    let objectives = program(vec![maximize, minimize.clone(), minimize, zero]);
    let model = Model::new([
        atom("p", vec![Value::Number(2)]),
        atom("p", vec![Value::Number(-3)]),
        atom("p", vec![Value::String("ignored".into())]),
        atom("q", vec![]),
    ]);
    let result = evaluate(
        &objectives,
        &model,
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(result.score().costs(), [(7, 0), (2, 1)]);
    assert_eq!(result.contributions().len(), 3);
    let keys: Vec<_> = result
        .contributions()
        .iter()
        .map(|key| (key.priority(), key.weight()))
        .collect();
    assert_eq!(keys, [(2, -2), (2, 3), (7, 0)]);
    let work = result.statistics().work;
    assert!(
        evaluate(
            &objectives,
            &model,
            Limits {
                max_work: work,
                ..Limits::default()
            },
            &Cancellation::default()
        )
        .is_ok()
    );
    let stopped = evaluate(
        &objectives,
        &model,
        Limits {
            max_work: work - 1,
            ..Limits::default()
        },
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(stopped.kind(), ErrorKind::Stopped(Stop::WorkLimit));
    assert_eq!(stopped.statistics().work, work - 1);
}

#[test]
fn normalized_signed_costs_use_wide_accumulation_and_preserve_minimize_min() {
    let objectives = program(vec![
        row(number(i32::MAX), 0, "a", vec![]).with_weight_polarity(WeightPolarity::Negated),
        row(number(i32::MAX), 0, "b", vec![]).with_weight_polarity(WeightPolarity::Negated),
        row(number(i32::MIN), 1, "old", vec![]),
    ]);
    let result = evaluate(
        &objectives,
        &Model::new([]),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(
        result.score().costs(),
        [(1, i64::from(i32::MIN)), (0, -2 * i64::from(i32::MAX))]
    );
}

#[test]
fn an_eligible_unrepresentable_negation_has_typed_template_evidence() {
    let objectives = program(vec![
        row(number(1), 0, "a", vec![]),
        row(
            Term::Variable(0),
            0,
            "b",
            vec![pattern("p", vec![Term::Variable(0)])],
        )
        .with_weight_polarity(WeightPolarity::Negated),
    ]);
    let error = evaluate(
        &objectives,
        &Model::new([atom("p", vec![Value::Number(i32::MIN)])]),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::WeightNormalizationOverflow);
    assert_eq!(error.template_index(), Some(1));
    assert!(error.statistics().work > 0);
    let empty = evaluate(
        &objectives,
        &Model::new([]),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(empty.score().costs(), [(0, 1)]);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let error = evaluate(
        &objectives,
        &Model::new([]),
        Limits::default(),
        &cancellation,
    )
    .unwrap_err();
    assert!(matches!(error.kind(), ErrorKind::Stopped(Stop::Cancelled)));
}
