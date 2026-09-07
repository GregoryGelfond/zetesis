//! Independent exhaustive valuation and objective-budget regression checks.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use proptest::prelude::*;
use zetesis_core::{Atom, AtomPattern, Filter, Model, Predicate, Term, Value};
use zetesis_cpu::Control;
use zetesis_objective::{
    AdmissionError, AdmissionLimits, ErrorKind, Limits, ObjectiveProgram, ObjectiveTemplate, Stop,
    evaluate,
};

fn number(value: i32) -> Term {
    Term::Constant(Value::Number(value))
}
fn variable(index: usize) -> Term {
    Term::Variable(index)
}
fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms).unwrap()
}
fn atom(name: &str, values: Vec<Value>) -> Atom {
    Atom::new(Predicate::new(name, values.len()).unwrap(), values).unwrap()
}
fn fact(name: &str, numbers: &[i32]) -> Atom {
    atom(name, numbers.iter().copied().map(Value::Number).collect())
}
fn template(
    weight: Term,
    priority: i32,
    tuple: Vec<Term>,
    positive: Vec<AtomPattern>,
    filters: Vec<Filter>,
) -> ObjectiveTemplate {
    ObjectiveTemplate::new(weight, priority, tuple, positive, filters)
}
fn program(templates: Vec<ObjectiveTemplate>) -> ObjectiveProgram {
    ObjectiveProgram::new(templates, AdmissionLimits::default()).unwrap()
}
fn cost(weight: i32, priority: i32) -> ObjectiveTemplate {
    template(number(weight), priority, vec![], vec![], vec![])
}

#[test]
fn extrema_are_distinct_deduplicated_tuple_keys_with_exact_byte_limits() {
    let model = Model::new([
        atom("p", vec![Value::Infimum]),
        atom("p", vec![Value::Supremum]),
    ]);
    let row = template(
        number(1),
        0,
        vec![variable(0)],
        vec![pattern("p", vec![variable(0)])],
        vec![],
    );
    let objectives = program(vec![row.clone(), row]);
    compare(&objectives, &model);
    let result = evaluate(&objectives, &model, Limits::default(), &Control::default()).unwrap();
    assert_eq!(result.score().costs(), &[(0, 2)]);
    assert_eq!(result.contributions().len(), 2);
    // Each contribution has 16 fixed bytes and one extremum tag, with no payload.
    assert_eq!(result.statistics().key_bytes, 34);
    for (bytes, complete) in [(34, true), (33, false)] {
        let result = evaluate(
            &objectives,
            &model,
            Limits {
                max_key_bytes: bytes,
                ..Limits::default()
            },
            &Control::default(),
        );
        if complete {
            assert!(result.is_ok());
        } else {
            assert_eq!(
                result.unwrap_err().kind(),
                ErrorKind::Stopped(Stop::KeyBytesLimit)
            );
        }
    }
    let spellings = Model::new([
        atom("p", vec![Value::Infimum]),
        atom("p", vec![Value::String("#inf".into())]),
        atom("p", vec![Value::Supremum]),
        atom("p", vec![Value::String("#sup".into())]),
    ]);
    compare(&objectives, &spellings);
    assert_eq!(
        evaluate(
            &objectives,
            &spellings,
            Limits::default(),
            &Control::default()
        )
        .unwrap()
        .score()
        .costs(),
        &[(0, 4)]
    );
    for value in [Value::Infimum, Value::Supremum] {
        let nonnumeric = program(vec![template(
            Term::Constant(value),
            0,
            vec![],
            vec![],
            vec![],
        )]);
        assert_eq!(
            evaluate(
                &nonnumeric,
                &Model::new([]),
                Limits::default(),
                &Control::default()
            )
            .unwrap()
            .score()
            .costs(),
            &[(0, 0)]
        );
    }
}

type Key = (i32, i32, Vec<Value>);

fn naive(program: &ObjectiveProgram, model: &Model) -> BTreeSet<Key> {
    // This deliberately enumerates a tiny test domain rather than using joins.
    // Safety ensures every variable value comes from some true model atom.
    let domain: Vec<_> = model
        .atoms()
        .iter()
        .flat_map(Atom::values)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut keys = BTreeSet::new();
    for template in program.templates() {
        let variables = template
            .positive()
            .iter()
            .flat_map(AtomPattern::terms)
            .filter_map(|term| {
                if let Term::Variable(variable) = term {
                    Some(variable + 1)
                } else {
                    None
                }
            })
            .max()
            .unwrap_or(0);
        let count = domain.len().pow(u32::try_from(variables).unwrap());
        for mut code in 0..count {
            let mut assignment = Vec::new();
            for _ in 0..variables {
                assignment.push(domain[code % domain.len()].clone());
                code /= domain.len();
            }
            if !template
                .positive()
                .iter()
                .all(|pattern| model.contains(&pattern.instantiate(&assignment).unwrap()))
                || !template
                    .filters()
                    .iter()
                    .all(|filter| filter.evaluate(&assignment).unwrap())
            {
                continue;
            }
            let Value::Number(weight) = template.weight().resolve(&assignment).unwrap() else {
                panic!("numeric test domain");
            };
            let tuple = template
                .tuple()
                .iter()
                .map(|term| term.resolve(&assignment).unwrap().clone())
                .collect();
            keys.insert((template.priority(), *weight, tuple));
        }
    }
    keys
}

fn compare(program: &ObjectiveProgram, model: &Model) {
    let expected = naive(program, model);
    let actual = evaluate(program, model, Limits::default(), &Control::default()).unwrap();
    let keys: BTreeSet<_> = actual
        .contributions()
        .iter()
        .map(|key| (key.priority(), key.weight(), key.tuple().to_vec()))
        .collect();
    assert_eq!(keys, expected);
    assert_eq!(actual.statistics().keys, expected.len());
    let costs: Vec<_> = program
        .priorities()
        .iter()
        .map(|priority| {
            (
                *priority,
                expected
                    .iter()
                    .filter(|(key_priority, _, _)| key_priority == priority)
                    .map(|(_, weight, _)| i64::from(*weight))
                    .sum::<i64>(),
            )
        })
        .collect();
    assert_eq!(actual.score().costs(), costs);
}

#[test]
fn partial_repeated_variable_matches_backtrack_without_leaking_bindings() {
    let model = Model::new([
        fact("pair", &[1, 2]),
        fact("pair", &[2, 2]),
        fact("next", &[2, 3]),
        fact("next", &[1, 9]),
    ]);
    let program = program(vec![template(
        variable(1),
        0,
        vec![variable(0)],
        vec![
            pattern("pair", vec![variable(0), variable(0)]),
            pattern("next", vec![variable(0), variable(1)]),
        ],
        vec![],
    )]);
    compare(&program, &model);
    assert_eq!(
        evaluate(&program, &model, Limits::default(), &Control::default())
            .unwrap()
            .score()
            .costs(),
        &[(0, 3)]
    );
}

#[test]
fn global_keys_include_weight_priority_and_scalar_value_class() {
    let duplicate = template(
        number(5),
        0,
        vec![],
        vec![pattern("p", vec![variable(0)])],
        vec![],
    );
    let program = program(vec![
        duplicate.clone(),
        duplicate,
        cost(-5, 0),
        cost(5, 1),
        template(
            number(2),
            0,
            vec![Term::Constant(Value::String("x".into()))],
            vec![],
            vec![],
        ),
        template(
            number(2),
            0,
            vec![Term::Constant(Value::Symbol("x".into()))],
            vec![],
            vec![],
        ),
    ]);
    let model = Model::new([fact("p", &[1]), fact("p", &[2])]);
    compare(&program, &model);
    let evaluation = evaluate(&program, &model, Limits::default(), &Control::default()).unwrap();
    assert_eq!(evaluation.score().costs(), &[(1, 5), (0, 4)]);
    assert_eq!(evaluation.statistics().duplicates, 3);
    assert_eq!(evaluation.contributions().len(), 5);
}

#[test]
fn absent_inactive_zero_and_cancelled_objectives_remain_distinguishable() {
    let model = Model::default();
    let absent = evaluate(
        &ObjectiveProgram::none(),
        &model,
        Limits::default(),
        &Control::default(),
    )
    .unwrap();
    let empty = evaluate(
        &program(vec![]),
        &model,
        Limits::default(),
        &Control::default(),
    )
    .unwrap();
    let zero = evaluate(
        &program(vec![cost(0, 7)]),
        &model,
        Limits::default(),
        &Control::default(),
    )
    .unwrap();
    let cancellation = evaluate(
        &program(vec![cost(2, 7), cost(-2, 7)]),
        &model,
        Limits::default(),
        &Control::default(),
    )
    .unwrap();
    let inactive = evaluate(
        &program(vec![template(
            number(4),
            7,
            vec![],
            vec![pattern("missing", vec![])],
            vec![],
        )]),
        &model,
        Limits::default(),
        &Control::default(),
    )
    .unwrap();
    assert!(!absent.score().is_present());
    for present in [&empty, &zero, &cancellation, &inactive] {
        assert!(present.score().is_present());
        assert_eq!(
            absent.score().compare_costs(present.score()),
            Ordering::Equal
        );
    }
    assert_eq!(zero.score().costs(), &[(7, 0)]);
    assert_eq!(cancellation.score().costs(), &[(7, 0)]);
    assert_eq!(inactive.score().costs(), &[(7, 0)]);
    assert!(inactive.contributions().is_empty());
    assert_eq!(cancellation.contributions().len(), 2);
}

#[test]
fn higher_priority_costs_dominate_and_missing_priorities_are_zero() {
    let model = Model::default();
    let left = evaluate(
        &program(vec![cost(2, 9), cost(-100, 1)]),
        &model,
        Limits::default(),
        &Control::default(),
    )
    .unwrap();
    let right = evaluate(
        &program(vec![cost(3, 9), cost(-200, 1)]),
        &model,
        Limits::default(),
        &Control::default(),
    )
    .unwrap();
    assert_eq!(left.score().compare_costs(right.score()), Ordering::Less);
    assert_eq!(right.score().compare_costs(left.score()), Ordering::Greater);
    let missing = evaluate(
        &program(vec![cost(0, 10), cost(2, 9), cost(-100, 1), cost(0, -5)]),
        &model,
        Limits::default(),
        &Control::default(),
    )
    .unwrap();
    assert_eq!(left.score().compare_costs(missing.score()), Ordering::Equal);
    assert_ne!(left.score(), missing.score());
}

#[test]
fn nonnumeric_weights_contribute_no_keys_and_preserve_declared_priorities() {
    let weight = Term::Constant(Value::Symbol("not_numeric".into()));
    let input = program(vec![
        cost(1, 0),
        template(
            weight.clone(),
            3,
            vec![],
            vec![pattern("p", vec![variable(0)])],
            vec![],
        ),
    ]);
    let inactive = evaluate(
        &input,
        &Model::default(),
        Limits::default(),
        &Control::default(),
    )
    .unwrap();
    assert_eq!(inactive.score().costs(), &[(3, 0), (0, 1)]);
    let model = Model::new([fact("p", &[1])]);
    let evaluation = evaluate(&input, &model, Limits::default(), &Control::default()).unwrap();
    assert_eq!(evaluation.score().costs(), &[(3, 0), (0, 1)]);
    assert_eq!(evaluation.statistics().keys, 1);
    assert_eq!(evaluation.statistics().active_bindings, 2);
    let filtered = program(vec![template(
        weight,
        3,
        vec![],
        vec![pattern("p", vec![variable(0)])],
        vec![Filter::Eq(variable(0), number(9))],
    )]);
    let result = evaluate(&filtered, &model, Limits::default(), &Control::default()).unwrap();
    assert_eq!(result.score().costs(), &[(3, 0)]);
    assert_eq!(result.statistics().bindings, 1);
    assert_eq!(result.statistics().active_bindings, 0);
}

#[test]
fn every_runtime_ceiling_is_inclusive_and_duplicates_do_not_consume_key_budget() {
    let model = Model::new([fact("p", &[1]), fact("p", &[2])]);
    let distinct = program(vec![template(
        number(1),
        0,
        vec![variable(0)],
        vec![pattern("p", vec![variable(0)])],
        vec![],
    )]);
    let full = evaluate(&distinct, &model, Limits::default(), &Control::default()).unwrap();
    assert_eq!(full.statistics().key_bytes, 42); // Two 16-byte headers plus two tagged i32s.
    let exact = Limits {
        max_work: full.statistics().work,
        max_bindings: 2,
        max_keys: 2,
        max_key_bytes: 42,
    };
    assert!(evaluate(&distinct, &model, exact, &Control::default()).is_ok());
    for (limits, expected) in [
        (
            Limits {
                max_work: exact.max_work - 1,
                ..exact
            },
            Stop::WorkLimit,
        ),
        (
            Limits {
                max_bindings: 1,
                ..exact
            },
            Stop::BindingLimit,
        ),
        (
            Limits {
                max_keys: 1,
                ..exact
            },
            Stop::KeyLimit,
        ),
        (
            Limits {
                max_key_bytes: 41,
                ..exact
            },
            Stop::KeyBytesLimit,
        ),
    ] {
        assert_eq!(
            evaluate(&distinct, &model, limits, &Control::default())
                .unwrap_err()
                .kind(),
            ErrorKind::Stopped(expected)
        );
    }
    let duplicate = program(vec![template(
        number(1),
        0,
        vec![],
        vec![pattern("p", vec![variable(0)])],
        vec![],
    )]);
    let evaluation = evaluate(
        &duplicate,
        &model,
        Limits {
            max_keys: 1,
            max_key_bytes: 16,
            ..Limits::default()
        },
        &Control::default(),
    )
    .unwrap();
    assert_eq!(evaluation.score().costs(), &[(0, 1)]);
    assert_eq!(evaluation.statistics().duplicates, 1);
}

#[test]
fn admission_enforces_safe_dense_variables_and_each_shape_bound() {
    let valid = template(
        variable(0),
        0,
        vec![variable(0)],
        vec![pattern("p", vec![variable(0)])],
        vec![Filter::Neq(variable(0), number(0))],
    );
    let exact = AdmissionLimits {
        max_templates: 1,
        max_tuple_width: 1,
        max_variables_per_template: 1,
        max_positive_body: 1,
        max_predicate_arity: 1,
        max_filters: 1,
    };
    assert!(ObjectiveProgram::new(vec![valid.clone()], exact).is_ok());
    for limit in [
        AdmissionLimits {
            max_templates: 0,
            ..exact
        },
        AdmissionLimits {
            max_tuple_width: 0,
            ..exact
        },
        AdmissionLimits {
            max_variables_per_template: 0,
            ..exact
        },
        AdmissionLimits {
            max_positive_body: 0,
            ..exact
        },
        AdmissionLimits {
            max_predicate_arity: 0,
            ..exact
        },
        AdmissionLimits {
            max_filters: 0,
            ..exact
        },
    ] {
        assert!(matches!(
            ObjectiveProgram::new(vec![valid.clone()], limit),
            Err(AdmissionError::Limit { .. })
        ));
    }
    let unsafe_weight = template(
        variable(0),
        0,
        vec![],
        vec![],
        vec![Filter::Eq(variable(0), number(1))],
    );
    let error = ObjectiveProgram::new(vec![cost(0, 0), unsafe_weight], AdmissionLimits::default())
        .unwrap_err();
    assert!(matches!(
        error,
        AdmissionError::UnsafeVariable {
            template: 1,
            variable: 0
        }
    ));
    assert_eq!(error.template_index(), Some(1));
    let sparse = template(
        variable(1),
        0,
        vec![],
        vec![pattern("p", vec![variable(1)])],
        vec![],
    );
    assert!(matches!(
        ObjectiveProgram::new(vec![sparse], AdmissionLimits::default()),
        Err(AdmissionError::NonDenseVariable { .. })
    ));
    let huge = template(
        variable(usize::MAX),
        0,
        vec![],
        vec![pattern("p", vec![variable(usize::MAX)])],
        vec![],
    );
    assert!(matches!(
        ObjectiveProgram::new(vec![huge], AdmissionLimits::default()),
        Err(AdmissionError::Overflow { .. })
    ));
}

#[test]
fn cancellation_and_deadlines_precede_evaluation_even_without_templates() {
    let control = Control::default();
    control.cancel();
    for (control, expected) in [
        (control, Stop::Cancelled),
        (
            Control::with_deadline(Instant::now().checked_sub(Duration::from_secs(1)).unwrap()),
            Stop::Deadline,
        ),
    ] {
        let error = evaluate(
            &ObjectiveProgram::none(),
            &Model::default(),
            Limits {
                max_work: 0,
                ..Limits::default()
            },
            &control,
        )
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Stopped(expected));
        assert_eq!(error.statistics().work, 0);
    }
}

#[test]
fn deep_positive_joins_use_heap_frames_on_a_small_thread_stack() {
    std::thread::Builder::new()
        .stack_size(65_536)
        .spawn(|| {
            let input = program(vec![template(
                number(3),
                0,
                vec![variable(0)],
                vec![pattern("p", vec![variable(0)]); 1_024],
                vec![],
            )]);
            let result = evaluate(
                &input,
                &Model::new([fact("p", &[1])]),
                Limits::default(),
                &Control::default(),
            )
            .unwrap();
            assert_eq!(result.score().costs(), &[(0, 3)]);
            assert_eq!(result.statistics().bindings, 1);
        })
        .unwrap()
        .join()
        .unwrap();
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn joined_scores_and_global_keys_match_independent_valuation(
        edges in prop::collection::vec((-2i32..=2, -2i32..=2), 0..18),
        first_priority in -3i32..=3,
        second_priority in -3i32..=3,
    ) {
        let model = Model::new(edges.into_iter().map(|(a,b)| fact("edge", &[a,b])));
        let first = template(variable(1), first_priority, vec![variable(0)], vec![pattern("edge", vec![variable(0), variable(1)])], vec![Filter::Neq(variable(0), variable(1))]);
        let second = template(variable(2), second_priority, vec![variable(0), number(17)], vec![pattern("edge", vec![variable(0), variable(1)]), pattern("edge", vec![variable(1), variable(2)])], vec![]);
        let templates = vec![first.clone(), first, second];
        compare(&program(templates.clone()), &model);
        let mut reversed = templates;
        reversed.reverse();
        let forward = evaluate(&program(reversed.clone()), &model, Limits::default(), &Control::default()).unwrap();
        reversed.reverse();
        let backward = evaluate(&program(reversed), &model, Limits::default(), &Control::default()).unwrap();
        prop_assert_eq!(forward.score(), backward.score());
        prop_assert_eq!(forward.contributions(), backward.contributions());
    }
}
