use super::{Bijection, validate_group};
use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::formula_ir::{ChoiceIr, Element, HeadElementKey, HeadLiteral, HeadMeasure, HeadOperand};
use crate::formula_support::Context;
use crate::formula_support::components::{Pattern, Term};
use crate::formula_support::testing::Fixture;
use crate::{
    AdmissionOptions, ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource,
    ProfileFeature,
};
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use themelios_program::program::DefaultNegation;
use zetesis_core::{AtomPattern, Predicate, Value};

fn location() -> Location {
    Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    }
}

fn budget() -> Budget {
    Budget::new(
        ExpansionLimits::default(),
        AdmissionOptions::default().core_limits.max_templates,
    )
}

fn pattern(fixture: &mut Fixture, name: &str) -> Pattern {
    fixture.pattern(
        &AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap(),
        location(),
    )
}

fn constant(fixture: &mut Fixture, value: i32) -> Term {
    Term::Constant(fixture.scalar(&Value::Number(value), location()))
}

fn element(fixture: &mut Fixture, keyed: bool) -> Element {
    Element {
        family: crate::formula_ir::LocalFamily(0),
        key: if keyed {
            HeadElementKey::Tuple(vec![constant(fixture, 1)])
        } else {
            HeadElementKey::Atom
        },
        head: HeadLiteral {
            negation: DefaultNegation::None,
            operand: HeadOperand::Atom(pattern(fixture, "p")),
        },
        condition: vec![],
        body_variables: 0,
        variables: 0,
    }
}

fn validate_measure(
    fixture: &mut Fixture,
    measure: HeadMeasure,
    prepare: impl FnOnce(&mut Fixture) -> Vec<Element>,
) -> Result<bool, FormulaFailure> {
    let group = ChoiceIr {
        measure,
        guards: vec![],
        elements: prepare(fixture),
    };
    validate(fixture, &group, &FormulaLimits::default()).map(|certificate| certificate.is_some())
}

fn group(fixture: &mut Fixture) -> ChoiceIr {
    ChoiceIr {
        measure: HeadMeasure::Count,
        guards: vec![],
        elements: vec![element(fixture, true)],
    }
}

fn validate(
    fixture: &mut Fixture,
    group: &ChoiceIr,
    limits: &FormulaLimits,
) -> Result<Option<Bijection>, FormulaFailure> {
    fixture.with(location(), |support, computation, counters| {
        let assignment =
            Binding::new(computation, &FormulaLimits::default(), counters, location()).unwrap();
        validate_group(
            group,
            &assignment,
            support,
            &mut budget(),
            Context::new(&mut *computation, limits, counters, location()),
        )
    })
}

#[test]
fn mixed_key_metadata_is_rejected() {
    for order in [[false, true], [true, false]] {
        let result = validate_measure(&mut Fixture::default(), HeadMeasure::Count, |fixture| {
            order.map(|keyed| element(fixture, keyed)).into()
        });
        assert!(matches!(
            result,
            Err(FormulaFailure::Expansion(
                crate::ExpansionFailure::Admission(crate::AdmissionFailure::Profile {
                    feature: ProfileFeature::HeadAggregateAlias,
                    ..
                })
            ))
        ));
    }
}

#[test]
fn homogeneous_count_groups_satisfy_the_key_invariant() {
    for keys in [vec![], vec![false, false], vec![true, true]] {
        assert!(
            validate_measure(&mut Fixture::default(), HeadMeasure::Count, |fixture| {
                keys.into_iter()
                    .map(|keyed| element(fixture, keyed))
                    .collect()
            })
            .is_ok()
        );
    }
}

#[test]
fn measured_groups_require_tuple_keys() {
    for measure in [
        HeadMeasure::Sum,
        HeadMeasure::SumPlus,
        HeadMeasure::Min,
        HeadMeasure::Max,
    ] {
        let error = validate_measure(&mut Fixture::default(), measure, |fixture| {
            vec![element(fixture, false)]
        })
        .unwrap_err();
        assert!(matches!(
            error,
            FormulaFailure::Expansion(crate::ExpansionFailure::Admission(
                crate::AdmissionFailure::Profile {
                    feature: ProfileFeature::HeadAggregateAlias,
                    ..
                }
            ))
        ));
    }
}

#[test]
fn empty_measured_groups_satisfy_the_key_invariant() {
    for measure in [
        HeadMeasure::Sum,
        HeadMeasure::SumPlus,
        HeadMeasure::Min,
        HeadMeasure::Max,
    ] {
        validate_measure(&mut Fixture::default(), measure, |_| vec![]).unwrap();
    }
}

#[test]
fn one_tuple_with_different_heads_cannot_certify_a_bijection() {
    for measure in [
        HeadMeasure::Count,
        HeadMeasure::Sum,
        HeadMeasure::SumPlus,
        HeadMeasure::Min,
        HeadMeasure::Max,
    ] {
        assert!(
            !validate_measure(&mut Fixture::default(), measure, |fixture| {
                let first = element(fixture, true);
                let mut same_tuple = element(fixture, true);
                same_tuple.head.operand = HeadOperand::Atom(pattern(fixture, "q"));
                vec![first, same_tuple]
            })
            .unwrap()
        );
    }
}

#[test]
fn different_tuples_with_one_head_cannot_certify_a_bijection() {
    for measure in [
        HeadMeasure::Count,
        HeadMeasure::Sum,
        HeadMeasure::SumPlus,
        HeadMeasure::Min,
        HeadMeasure::Max,
    ] {
        assert!(
            !validate_measure(&mut Fixture::default(), measure, |fixture| {
                let first = element(fixture, true);
                let mut same_atom = element(fixture, true);
                same_atom.key = HeadElementKey::Tuple(vec![constant(fixture, 2)]);
                vec![first, same_atom]
            })
            .unwrap()
        );
    }
}

#[test]
fn boolean_elements_never_certify_atom_planning() {
    for value in [false, true] {
        for (measure, keyed) in [
            (HeadMeasure::Count, false),
            (HeadMeasure::Count, true),
            (HeadMeasure::Sum, true),
            (HeadMeasure::SumPlus, true),
            (HeadMeasure::Min, true),
            (HeadMeasure::Max, true),
        ] {
            assert!(
                !validate_measure(&mut Fixture::default(), measure, |fixture| {
                    let mut element = element(fixture, keyed);
                    element.head.operand = HeadOperand::Boolean(value);
                    vec![element]
                })
                .unwrap()
            );
        }
    }
}

#[test]
fn signed_elements_never_certify_atom_planning() {
    for negation in [DefaultNegation::Not, DefaultNegation::NotNot] {
        for keyed in [false, true] {
            assert!(
                !validate_measure(&mut Fixture::default(), HeadMeasure::Count, |fixture| {
                    let mut signed = element(fixture, keyed);
                    signed.head.negation = negation;
                    vec![signed]
                })
                .unwrap()
            );
        }
    }
}

#[test]
fn positive_elements_certify_atom_planning() {
    for keyed in [false, true] {
        assert!(
            validate_measure(&mut Fixture::default(), HeadMeasure::Count, |fixture| vec![
                element(fixture, keyed)
            ])
            .unwrap()
        );
    }
}

#[test]
fn repeated_tuple_head_pairs_count_once() {
    let mut fixture = Fixture::default();
    let group = ChoiceIr {
        measure: HeadMeasure::Count,
        guards: vec![],
        elements: vec![element(&mut fixture, true), element(&mut fixture, true)],
    };
    let certificate = validate(&mut fixture, &group, &FormulaLimits::default())
        .unwrap()
        .unwrap();
    assert!(certificate.accepts_members(1));
    assert!(!certificate.accepts_members(0));
    assert!(!certificate.accepts_members(2));
}

#[test]
fn an_empty_keyed_population_certifies_exactly_zero() {
    let mut fixture = Fixture::default();
    let mut absent = element(&mut fixture, true);
    absent.condition.push(crate::formula_ir::LiteralIr::Atom(
        DefaultNegation::None,
        pattern(&mut fixture, "absent"),
    ));
    let group = ChoiceIr {
        measure: HeadMeasure::Count,
        guards: vec![],
        elements: vec![absent],
    };
    let certificate = validate(&mut fixture, &group, &FormulaLimits::default())
        .unwrap()
        .unwrap();
    assert!(certificate.accepts_members(0));
    assert!(!certificate.accepts_members(1));
}

#[test]
fn tuple_limits_count_distinct_keys() {
    let mut limits = FormulaLimits::default();
    limits.aggregate.max_elements = 1;
    let mut fixture = Fixture::default();
    let repeated = ChoiceIr {
        measure: HeadMeasure::Count,
        guards: vec![],
        elements: vec![element(&mut fixture, true), element(&mut fixture, true)],
    };
    assert!(
        validate(&mut fixture, &repeated, &limits)
            .unwrap()
            .is_some()
    );
    let mut fixture = Fixture::default();
    let mut other = element(&mut fixture, true);
    other.key = HeadElementKey::Tuple(vec![constant(&mut fixture, 2)]);
    let distinct = ChoiceIr {
        measure: HeadMeasure::Count,
        guards: vec![],
        elements: vec![element(&mut fixture, true), other],
    };
    let error = validate(&mut fixture, &distinct, &limits).unwrap_err();
    assert!(matches!(
        error,
        FormulaFailure::Limit {
            resource: FormulaResource::AggregateElements,
            observed: 2,
            limit: 1,
            ..
        }
    ));
}

#[test]
fn ineligible_certificates_do_not_hide_measure_errors() {
    let mut fixture = Fixture::default();
    let mut signed = element(&mut fixture, true);
    signed.head.negation = DefaultNegation::Not;
    signed.key = HeadElementKey::Tuple(vec![constant(&mut fixture, i32::MAX)]);
    let zero = fixture.scalar(&Value::Number(0), location());
    let error = validate(
        &mut fixture,
        &ChoiceIr {
            measure: HeadMeasure::Min,
            guards: vec![crate::formula_ir::AggregateGuard {
                relation: themelios_program::program::Relation::Ge,
                bound: crate::formula_ir::Expression {
                    nodes: vec![crate::formula_ir::Operation::Constant(zero)],
                },
            }],
            elements: vec![signed],
        },
        &FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        FormulaFailure::Expansion(crate::ExpansionFailure::Admission(
            crate::AdmissionFailure::ExtremumEndpoint {
                value: i32::MAX,
                ..
            }
        ))
    ));
}

#[test]
fn validation_discovers_identity_without_selecting_support() {
    let mut fixture = Fixture::default();
    let group = group(&mut fixture);
    fixture.with(location(), |support, computation, counters| {
        let limits = FormulaLimits::default();
        let assignment = Binding::new(computation, &limits, counters, location()).unwrap();
        assert!(
            validate_group(
                &group,
                &assignment,
                support,
                &mut budget(),
                Context::new(&mut *computation, &limits, counters, location())
            )
            .unwrap()
            .is_some()
        );
        let pattern = computation
            .static_pattern(
                *group.elements[0].head.positive_atom().unwrap(),
                &limits,
                counters,
                location(),
            )
            .unwrap();
        assert!(
            !computation
                .contains_pattern(pattern, &assignment, &limits, counters, location())
                .unwrap()
        );
    });
}

#[test]
fn every_work_refusal_withholds_the_certificate() {
    let mut fixture = Fixture::default();
    let complete = group(&mut fixture);
    let needed = fixture.with(location(), |support, computation, counters| {
        let limits = FormulaLimits::default();
        let assignment = Binding::new(computation, &limits, counters, location()).unwrap();
        let before = counters.accounting.work;
        validate_group(
            &complete,
            &assignment,
            support,
            &mut budget(),
            Context::new(&mut *computation, &limits, counters, location()),
        )
        .unwrap()
        .unwrap();
        counters.accounting.work - before
    });
    assert!(needed > 0);
    for cutoff in 0..needed {
        let mut fixture = Fixture::default();
        let group = group(&mut fixture);
        fixture.with(location(), |support, computation, counters| {
            let limits = FormulaLimits::default();
            let assignment = Binding::new(computation, &limits, counters, location()).unwrap();
            let bounded = FormulaLimits {
                max_work: counters.accounting.work + cutoff,
                ..limits
            };
            assert!(matches!(
                validate_group(
                    &group,
                    &assignment,
                    support,
                    &mut budget(),
                    Context::new(&mut *computation, &bounded, counters, location())
                ),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                })
            ));
            let certificate = validate_group(
                &group,
                &assignment,
                support,
                &mut budget(),
                Context::new(&mut *computation, &limits, counters, location()),
            )
            .unwrap()
            .unwrap();
            assert!(certificate.accepts_members(1));
        });
    }
}

#[test]
fn validation_scratch_obeys_the_shared_storage_ceiling() {
    let mut fixture = Fixture::default();
    let group = group(&mut fixture);
    fixture.with(location(), |support, computation, counters| {
        let limits = FormulaLimits::default();
        let assignment = Binding::new(computation, &limits, counters, location()).unwrap();
        let empty = computation.lease();
        let live = limits.max_support_bytes - computation.allowance(&empty, &limits, location()).unwrap();
        let bounded = FormulaLimits { max_support_bytes: live, ..limits };
        assert!(matches!(validate_group(&group,
&assignment,
support,
&mut budget(),
Context::new(&mut *computation, &bounded, counters, location())), Err(FormulaFailure::Limit { resource: FormulaResource::SupportBytes, observed, limit, .. }) if observed > limit && limit == live as u128));
        assert!(validate_group(&group,
&assignment,
support,
&mut budget(),
Context::new(&mut *computation, &limits, counters, location())).unwrap().unwrap().accepts_members(1));
    });
}
