//! Independent exhaustive valuation and objective-budget regression checks.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use crate::support::programs::program;
use proptest::prelude::*;
use zetesis_core::{
    AtomPattern, Filter, Model, TemplateTerm, Term, Value, ValueLimits, ValueNodeRef,
};
use zetesis_cpu::Cancellation;
use zetesis_objective::{
    AdmissionError, AdmissionLimits, ErrorKind, Limits, ObjectiveProgram, ObjectiveTemplate, Stop,
    evaluate,
};
use zetesis_test_support::programs::{atom, number, numbered as fact, pattern, variable};

fn template(
    weight: Term,
    priority: i32,
    tuple: Vec<Term>,
    positive: Vec<AtomPattern>,
    filters: Vec<Filter>,
) -> ObjectiveTemplate {
    ObjectiveTemplate::new(weight, priority, tuple, positive, filters)
}
fn cost(weight: i32, priority: i32) -> ObjectiveTemplate {
    template(number(weight), priority, vec![], vec![], vec![])
}

#[test]
fn extrema_are_distinct_deduplicated_tuple_keys_with_exact_byte_limits() {
    let model = Model::new([
        atom("p", vec![Value::Infimum]),
        atom("p", vec![Value::Supremum]),
    ])
    .unwrap();
    let row = template(
        number(1),
        0,
        vec![variable(0)],
        vec![pattern("p", vec![variable(0)])],
        vec![],
    );
    let objectives = program(vec![row.clone(), row]);
    compare(&objectives, &model);
    let result = evaluate(
        &objectives,
        &model,
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
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
            &Cancellation::default(),
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
    ])
    .unwrap();
    compare(&objectives, &spellings);
    assert_eq!(
        evaluate(
            &objectives,
            &spellings,
            Limits::default(),
            &Cancellation::default()
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
                &Model::new([]).unwrap(),
                Limits::default(),
                &Cancellation::default()
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
        .flat_map(zetesis_core::catalog::AtomRef::values)
        .map(|value| value.to_value(ValueLimits::default()).unwrap())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut keys = BTreeSet::new();
    for template in program.templates() {
        let variables = template
            .positive()
            .iter()
            .flat_map(zetesis_core::PatternRef::terms)
            .filter_map(|term| {
                if let TemplateTerm::Variable(variable) = term {
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
            if !template.positive().iter().all(|pattern| {
                model.contains(
                    &pattern
                        .key(assignment.as_slice())
                        .unwrap()
                        .to_atom(ValueLimits::default())
                        .unwrap(),
                )
            }) || !template
                .filters()
                .iter()
                .all(|filter| filter.evaluate(assignment.as_slice()).unwrap())
            {
                continue;
            }
            let ValueNodeRef::Number(weight) = template
                .weight()
                .resolve(assignment.as_slice())
                .unwrap()
                .descriptor()
            else {
                panic!("numeric test domain");
            };
            let tuple = template
                .tuple()
                .iter()
                .map(|term| {
                    term.resolve(assignment.as_slice())
                        .unwrap()
                        .to_value(ValueLimits::default())
                        .unwrap()
                })
                .collect();
            keys.insert((template.priority(), weight, tuple));
        }
    }
    keys
}

fn compare(program: &ObjectiveProgram, model: &Model) {
    let expected = naive(program, model);
    let actual = evaluate(program, model, Limits::default(), &Cancellation::default()).unwrap();
    let keys: BTreeSet<_> = actual
        .contributions()
        .iter()
        .map(|key| {
            (
                key.priority(),
                key.weight(),
                key.tuple()
                    .iter()
                    .map(|value| {
                        value
                            .to_value(zetesis_core::ValueLimits::default())
                            .unwrap()
                    })
                    .collect::<Vec<_>>(),
            )
        })
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
    ])
    .unwrap();
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
        evaluate(
            &program,
            &model,
            Limits::default(),
            &Cancellation::default()
        )
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
    let model = Model::new([fact("p", &[1]), fact("p", &[2])]).unwrap();
    compare(&program, &model);
    let evaluation = evaluate(
        &program,
        &model,
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(evaluation.score().costs(), &[(1, 5), (0, 4)]);
    assert_eq!(evaluation.statistics().duplicates, 3);
    assert_eq!(evaluation.contributions().len(), 5);
}

#[test]
fn absent_inactive_zero_and_cancelled_objectives_remain_distinguishable() {
    let model = Model::default();
    let absent_program = ObjectiveProgram::none();
    let absent = evaluate(
        &absent_program,
        &model,
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let empty_program = program(vec![]);
    let empty = evaluate(
        &empty_program,
        &model,
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let zero_program = program(vec![cost(0, 7)]);
    let zero = evaluate(
        &zero_program,
        &model,
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let cancellation_program = program(vec![cost(2, 7), cost(-2, 7)]);
    let cancellation = evaluate(
        &cancellation_program,
        &model,
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let inactive_program = program(vec![template(
        number(4),
        7,
        vec![],
        vec![pattern("missing", vec![])],
        vec![],
    )]);
    let inactive = evaluate(
        &inactive_program,
        &model,
        Limits::default(),
        &Cancellation::default(),
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
    let left_program = program(vec![cost(2, 9), cost(-100, 1)]);
    let left = evaluate(
        &left_program,
        &model,
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let right_program = program(vec![cost(3, 9), cost(-200, 1)]);
    let right = evaluate(
        &right_program,
        &model,
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(left.score().compare_costs(right.score()), Ordering::Less);
    assert_eq!(right.score().compare_costs(left.score()), Ordering::Greater);
    let missing_program = program(vec![cost(0, 10), cost(2, 9), cost(-100, 1), cost(0, -5)]);
    let missing = evaluate(
        &missing_program,
        &model,
        Limits::default(),
        &Cancellation::default(),
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
    let inactive_model = Model::default();
    let inactive = evaluate(
        &input,
        &inactive_model,
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(inactive.score().costs(), &[(3, 0), (0, 1)]);
    let model = Model::new([fact("p", &[1])]).unwrap();
    let evaluation = evaluate(&input, &model, Limits::default(), &Cancellation::default()).unwrap();
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
    let result = evaluate(
        &filtered,
        &model,
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(result.score().costs(), &[(3, 0)]);
    assert_eq!(result.statistics().bindings, 1);
    assert_eq!(result.statistics().active_bindings, 0);
}

#[test]
fn every_runtime_ceiling_is_inclusive_and_duplicates_do_not_consume_key_budget() {
    let model = Model::new([fact("p", &[1]), fact("p", &[2])]).unwrap();
    let distinct = program(vec![template(
        number(1),
        0,
        vec![variable(0)],
        vec![pattern("p", vec![variable(0)])],
        vec![],
    )]);
    let full = evaluate(
        &distinct,
        &model,
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(full.statistics().key_bytes, 42); // Two 16-byte headers plus two tagged i32s.
    let exact = Limits {
        max_work: full.statistics().work,
        max_bindings: 2,
        max_keys: 2,
        max_key_bytes: 42,
    };
    assert!(evaluate(&distinct, &model, exact, &Cancellation::default()).is_ok());
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
            evaluate(&distinct, &model, limits, &Cancellation::default())
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
        &Cancellation::default(),
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
        max_condition_nodes: 0,
        // This fixture isolates logical shape bounds; physical storage has
        // its own admission regressions.
        ..AdmissionLimits::default()
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
    let cancellation = Cancellation::default();
    cancellation.cancel();
    for (cancellation, expected) in [
        (cancellation, Stop::Cancelled),
        (
            Cancellation::with_deadline(
                Instant::now().checked_sub(Duration::from_secs(1)).unwrap(),
            )
            .unwrap(),
            Stop::Deadline,
        ),
    ] {
        let error_program = ObjectiveProgram::none();
        let error_model = Model::default();
        let error = evaluate(
            &error_program,
            &error_model,
            Limits {
                max_work: 0,
                ..Limits::default()
            },
            &cancellation,
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
            let result_model = Model::new([fact("p", &[1])]).unwrap();
            let result = evaluate(
                &input,
                &result_model,
                Limits::default(),
                &Cancellation::default(),
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
        let model = Model::new(edges.into_iter().map(|(a,b)| fact("edge", &[a,b]))).unwrap();
        let first = template(variable(1), first_priority, vec![variable(0)], vec![pattern("edge", vec![variable(0), variable(1)])], vec![Filter::Neq(variable(0), variable(1))]);
        let second = template(variable(2), second_priority, vec![variable(0), number(17)], vec![pattern("edge", vec![variable(0), variable(1)]), pattern("edge", vec![variable(1), variable(2)])], vec![]);
        let templates = vec![first.clone(), first, second];
        compare(&program(templates.clone()), &model);
        let mut reversed = templates;
        reversed.reverse();
        let forward_program = program(reversed.clone());
        let forward = evaluate(&forward_program, &model, Limits::default(), &Cancellation::default()).unwrap();
        reversed.reverse();
        let backward_program = program(reversed);
        let backward = evaluate(&backward_program, &model, Limits::default(), &Cancellation::default()).unwrap();
        prop_assert_eq!(forward.score(), backward.score());
        prop_assert_eq!(forward.contributions(), backward.contributions());
    }
}
