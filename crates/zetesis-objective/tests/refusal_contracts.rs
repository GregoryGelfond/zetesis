//! Objective refusals preserve caller evidence and never expose a partial score.

use std::error::Error as _;
use std::time::Instant;

use zetesis_core::{Atom, AtomPattern, Filter, Model, Predicate, Term, Value};
use zetesis_cpu::Control;
use zetesis_objective::{
    AdmissionError, AdmissionLimits, AdmissionResource, ErrorKind, Limits, ObjectiveProgram,
    ObjectiveTemplate, Stop, WeightPolarity, evaluate,
};

fn pattern(variable: usize) -> AtomPattern {
    AtomPattern::new(
        Predicate::new("p", 1).unwrap(),
        vec![Term::Variable(variable)],
    )
    .unwrap()
}

fn template(variable: usize) -> ObjectiveTemplate {
    ObjectiveTemplate::new(
        Term::Variable(variable),
        7,
        vec![Term::Variable(variable)],
        vec![pattern(variable)],
        vec![Filter::Neq(
            Term::Variable(variable),
            Term::Constant(Value::Number(0)),
        )],
    )
}

fn model() -> Model {
    Model::new([1, 2].map(|value| {
        Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(value)]).unwrap()
    }))
}

#[test]
fn unresolved_priority_fields_keep_variable_safety() {
    let weight = Term::Variable(1);
    let positive = [pattern(0)];
    assert_eq!(
        ObjectiveTemplate::validate_fields(
            &weight,
            &[],
            &positive,
            &[],
            AdmissionLimits::default(),
            7,
        ),
        Err(AdmissionError::UnsafeVariable {
            template: 7,
            variable: 1
        }),
    );
}

#[test]
fn unresolved_priority_fields_report_bound_variables() {
    let weight = Term::Constant(Value::Number(1));
    let positive = [pattern(0)];
    assert_eq!(
        ObjectiveTemplate::validate_fields(
            &weight,
            &[],
            &positive,
            &[],
            AdmissionLimits::default(),
            7,
        ),
        Ok(1),
    );
}

#[test]
fn every_admission_dimension_reports_template_scope_before_a_successful_retry() {
    let valid = template(0);
    for (limits, dimension, template_index) in [
        (
            AdmissionLimits {
                max_templates: 0,
                ..Default::default()
            },
            AdmissionResource::Templates,
            None,
        ),
        (
            AdmissionLimits {
                max_tuple_width: 0,
                ..Default::default()
            },
            AdmissionResource::TupleWidth,
            Some(0),
        ),
        (
            AdmissionLimits {
                max_variables_per_template: 0,
                ..Default::default()
            },
            AdmissionResource::Variables,
            Some(0),
        ),
        (
            AdmissionLimits {
                max_positive_body: 0,
                ..Default::default()
            },
            AdmissionResource::PositiveBody,
            Some(0),
        ),
        (
            AdmissionLimits {
                max_predicate_arity: 0,
                ..Default::default()
            },
            AdmissionResource::PredicateArity,
            Some(0),
        ),
        (
            AdmissionLimits {
                max_filters: 0,
                ..Default::default()
            },
            AdmissionResource::Filters,
            Some(0),
        ),
    ] {
        let error = ObjectiveProgram::new(vec![valid.clone()], limits).unwrap_err();
        assert!(
            matches!(error, AdmissionError::Limit { resource, actual: 1, limit: 0, .. } if resource == dimension),
            "{error}"
        );
        assert_eq!(error.template_index(), template_index);
        let message = error.to_string();
        assert!(message.contains(&format!("{dimension:?}")), "{message}");
        assert!(message.contains('1') && message.contains('0'), "{message}");
        assert!(error.source().is_none());
        let program =
            ObjectiveProgram::new(vec![valid.clone()], AdmissionLimits::default()).unwrap();
        assert_eq!(
            evaluate(&program, &model(), Limits::default(), &Control::default())
                .unwrap()
                .score()
                .costs(),
            &[(7, 3)]
        );
    }
}

#[test]
fn unsafe_and_sparse_variables_remain_attributed_to_the_original_template() {
    let unbound = ObjectiveTemplate::new(Term::Variable(0), 7, vec![], vec![], vec![]);
    let error =
        ObjectiveProgram::new(vec![template(0), unbound], AdmissionLimits::default()).unwrap_err();
    assert_eq!(
        error,
        AdmissionError::UnsafeVariable {
            template: 1,
            variable: 0
        }
    );
    assert_eq!(error.template_index(), Some(1));
    assert!(error.to_string().contains("variable 0"));
    assert!(error.to_string().contains('1'));
    let error = ObjectiveProgram::new(vec![template(1)], AdmissionLimits::default()).unwrap_err();
    assert_eq!(
        error,
        AdmissionError::NonDenseVariable {
            template: 0,
            variable: 0
        }
    );
    assert_eq!(error.template_index(), Some(0));
    assert!(error.to_string().contains('0'));
    assert!(error.source().is_none());
}

#[test]
fn evaluation_limits_keep_downcastable_causes_and_reusable_model_and_program() {
    let program = ObjectiveProgram::new(vec![template(0)], AdmissionLimits::default()).unwrap();
    let model = model();
    let original_templates = program.templates().to_vec();
    let original_model = model.clone();
    let exact = evaluate(&program, &model, Limits::default(), &Control::default()).unwrap();
    for (limits, reason) in [
        (
            Limits {
                max_work: 0,
                ..Default::default()
            },
            Stop::WorkLimit,
        ),
        (
            Limits {
                max_bindings: 1,
                ..Default::default()
            },
            Stop::BindingLimit,
        ),
        (
            Limits {
                max_keys: 1,
                ..Default::default()
            },
            Stop::KeyLimit,
        ),
        (
            Limits {
                max_key_bytes: exact.statistics().key_bytes - 1,
                ..Default::default()
            },
            Stop::KeyBytesLimit,
        ),
    ] {
        let error = evaluate(&program, &model, limits, &Control::default()).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Stopped(reason));
        assert_eq!(
            error.source().unwrap().downcast_ref::<Stop>(),
            Some(&reason)
        );
        assert!(error.to_string().contains("limit"), "{error}");
        assert!(error.statistics().keys <= exact.statistics().keys);
        assert_eq!(program.templates(), original_templates);
        assert_eq!(model, original_model);
        let retry = evaluate(&program, &model, Limits::default(), &Control::default()).unwrap();
        assert_eq!(retry.score(), exact.score());
        assert_eq!(retry.statistics(), exact.statistics());
    }
}

#[test]
fn cancelled_and_expired_scores_have_control_causes_and_zero_work() {
    let cancelled = Control::default();
    cancelled.cancel();
    let program = ObjectiveProgram::new(vec![template(0)], AdmissionLimits::default()).unwrap();
    for (control, reason, phrase) in [
        (cancelled, Stop::Cancelled, "cancelled"),
        (
            Control::with_deadline(Instant::now()).unwrap(),
            Stop::Deadline,
            "deadline expired",
        ),
    ] {
        let error = evaluate(&program, &model(), Limits::default(), &control).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Stopped(reason));
        assert_eq!(
            error.source().unwrap().downcast_ref::<Stop>(),
            Some(&reason)
        );
        assert!(error.to_string().contains(phrase));
        assert_eq!(error.statistics().work, 0);
        assert_eq!(error.template_index(), None);
    }
}

#[test]
fn an_active_unrepresentable_maximize_weight_is_distinct_from_a_missing_binding() {
    let row = template(0).with_weight_polarity(WeightPolarity::Negated);
    let program = ObjectiveProgram::new(vec![row], AdmissionLimits::default()).unwrap();
    let extreme = Model::new([Atom::new(
        Predicate::new("p", 1).unwrap(),
        vec![Value::Number(i32::MIN)],
    )
    .unwrap()]);
    let error = evaluate(&program, &extreme, Limits::default(), &Control::default()).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::WeightNormalizationOverflow);
    assert_eq!(error.template_index(), Some(0));
    assert_eq!(error.statistics().keys, 0);
    assert!(
        error
            .to_string()
            .contains("negation exceeds signed 32-bit range")
    );
    assert!(error.to_string().contains("template 0"));
    assert!(error.source().is_none());
    assert_eq!(
        evaluate(
            &program,
            &Model::default(),
            Limits::default(),
            &Control::default()
        )
        .unwrap()
        .score()
        .costs(),
        &[(7, 0)]
    );
}

#[test]
fn source_expression_scope_shares_field_admission_bounds() {
    let template = template(0);
    for maximum in 0..=2 {
        for limits in [
            AdmissionLimits {
                max_tuple_width: maximum,
                ..Default::default()
            },
            AdmissionLimits {
                max_variables_per_template: maximum,
                ..Default::default()
            },
            AdmissionLimits {
                max_positive_body: maximum,
                ..Default::default()
            },
            AdmissionLimits {
                max_predicate_arity: maximum,
                ..Default::default()
            },
            AdmissionLimits {
                max_filters: maximum,
                ..Default::default()
            },
        ] {
            assert_eq!(
                ObjectiveTemplate::validate_scope(
                    template.tuple().len(),
                    template.positive(),
                    template.filters(),
                    limits,
                    4
                ),
                ObjectiveTemplate::validate_fields(
                    template.weight(),
                    template.tuple(),
                    template.positive(),
                    template.filters(),
                    limits,
                    4
                ),
            );
        }
    }
}

#[test]
fn source_expression_scope_rejects_unbound_filters() {
    assert_eq!(
        ObjectiveTemplate::validate_scope(
            0,
            &[pattern(0)],
            &[Filter::Eq(Term::Variable(0), Term::Variable(1))],
            AdmissionLimits::default(),
            4
        ),
        Err(AdmissionError::UnsafeVariable {
            template: 4,
            variable: 1
        }),
    );
}
