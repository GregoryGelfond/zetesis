use super::*;
use crate::{ExpansionLimits, ExpansionResource, StatementId};
use themelios_program::program::{Arguments, Atom, Rule};
use themelios_program::symbol::{Name, Sign, Symbol};
use themelios_program::term::{EvalError, Term as SourceTerm, Variable};
use zetesis_core::{Term, Value};
use zetesis_cpu::{Cancellation, Stop};

fn fact(lower: i32, upper: i32) -> WithProvenance<Statement> {
    WithProvenance::constructed(
        Rule::fact(Atom::new(
            Name::new("p").unwrap(),
            [SourceTerm::Interval {
                lower: Box::new(lower.into()),
                upper: Box::new(upper.into()),
            }],
        ))
        .into(),
    )
}

#[test]
fn cancellation_preserves_an_accepted_expansion_prefix() {
    let cancellation = Cancellation::default();
    let mut budget =
        Budget::new(ExpansionLimits::default(), 100).with_cancellation(Some(cancellation.clone()));
    let first = ProgramSite::statement(StatementId::new(0), None);
    let second = ProgramSite::statement(StatementId::new(1), None);
    assert_eq!(
        facts(&fact(1, 3), &mut budget, first)
            .unwrap()
            .unwrap()
            .len(),
        3
    );
    let accepted = budget.usage();
    assert_eq!(accepted.templates, 3);
    cancellation.cancel();
    let error = facts(&fact(4, 6), &mut budget, second).unwrap_err();
    assert!(matches!(error, ExpansionFailure::Interrupted {
        reason: Stop::Cancelled, location,
    } if location == second));
    assert_eq!(budget.usage(), accepted);
}

fn family_budget(bytes: usize) -> Budget {
    Budget::new(
        ExpansionLimits {
            max_templates: usize::MAX,
            max_values: usize::MAX,
            max_term_work: usize::MAX,
            max_scalar_bytes: usize::MAX,
            max_family_bytes: bytes,
            ..ExpansionLimits::default()
        },
        usize::MAX,
    )
}

#[test]
fn interval_facts_refuse_before_materialization() {
    let location = ProgramSite::statement(StatementId::new(0), None);
    let error = facts(&fact(1, 1_000_000), &mut family_budget(1024), location).unwrap_err();
    assert!(matches!(error, ExpansionFailure::Limit {
        resource: ExpansionResource::FamilyBytes, limit: 1024, observed, ..
    } if observed > 1_000_000));
}

#[test]
fn fact_family_storage_boundary_is_inclusive() {
    let location = ProgramSite::statement(StatementId::new(0), None);
    let error = facts(&fact(1, 3), &mut family_budget(0), location).unwrap_err();
    let ExpansionFailure::Limit {
        resource: ExpansionResource::FamilyBytes,
        observed,
        ..
    } = error
    else {
        panic!("wrong pre-materialization refusal: {error}");
    };
    let exact = usize::try_from(observed).unwrap();
    assert!(
        matches!(facts(&fact(1, 3), &mut family_budget(exact - 1), location), Err(ExpansionFailure::Limit {
        resource: ExpansionResource::FamilyBytes, observed: amount, ..
    }) if amount == observed)
    );
    assert_eq!(
        facts(&fact(1, 3), &mut family_budget(exact), location)
            .unwrap()
            .unwrap()
            .len(),
        3
    );
}

fn input(arguments: Vec<SourceTerm>) -> WithProvenance<Statement> {
    WithProvenance::constructed(Rule::fact(Atom::new(Name::new("p").unwrap(), arguments)).into())
}

fn range(lower: i32, upper: i32) -> SourceTerm {
    SourceTerm::Interval {
        lower: Box::new(lower.into()),
        upper: Box::new(upper.into()),
    }
}

fn heads(source: &WithProvenance<Statement>, budget: &mut Budget) -> Vec<Vec<Value>> {
    let location = crate::test_support::location();
    let mut facts = facts(source, budget, location).unwrap().unwrap();
    let count = facts.len();
    let mut output = Vec::new();
    while let Some(head) = facts.next(budget, location).unwrap() {
        output.push(
            head.terms()
                .iter()
                .map(|term| match term {
                    Term::Constant(value) => value.clone(),
                    Term::Variable(_) => panic!("fact acquired a variable"),
                })
                .collect(),
        );
    }
    assert_eq!(output.len(), count);
    assert_eq!(facts.len(), 0);
    assert!(facts.next(budget, location).unwrap().is_none());
    output
}

#[test]
fn products_keep_pool_occurrences_and_argument_order() {
    let source = input(vec![
        SourceTerm::Pool(vec![
            1.into(),
            range(4, 3),
            SourceTerm::Pool(vec![2.into(), 1.into()]),
        ]),
        range(7, 8),
    ]);
    let mut budget = family_budget(usize::MAX);
    assert_eq!(
        heads(&source, &mut budget),
        vec![
            vec![Value::Number(1), Value::Number(7)],
            vec![Value::Number(1), Value::Number(8)],
            vec![Value::Number(2), Value::Number(7)],
            vec![Value::Number(2), Value::Number(8)],
            vec![Value::Number(1), Value::Number(7)],
            vec![Value::Number(1), Value::Number(8)],
        ]
    );
    assert_eq!(budget.usage().templates, 6);
    assert_eq!(budget.usage().values, 17); // 3+2 alternatives, 6*2 arguments.
}

#[test]
fn a_nullary_fact_is_one_product() {
    let mut budget = family_budget(usize::MAX);
    assert_eq!(
        heads(&input(Vec::new()), &mut budget),
        vec![Vec::<Value>::new()]
    );
    assert_eq!(budget.usage().templates, 1);
    assert_eq!(budget.usage().values, 0);
}

#[test]
fn empty_products_preserve_logical_alternative_charges() {
    let source = input(vec![range(2, 1), range(1, 100)]);
    let mut budget = family_budget(0);
    assert!(heads(&source, &mut budget).is_empty());
    assert_eq!(budget.usage().templates, 0);
    assert_eq!(budget.usage().values, 100);
    assert_eq!(budget.usage().scalar_bytes, 0);
}

#[test]
fn empty_products_do_not_hide_later_invalid_arguments() {
    let location = crate::test_support::location();
    let source = input(vec![
        range(2, 1),
        SourceTerm::Symbolic(Symbol::String("bad\0".into())),
    ]);
    let error = facts(&source, &mut family_budget(usize::MAX), location).unwrap_err();
    assert!(
        matches!(error, ExpansionFailure::Admission(crate::AdmissionFailure::Profile {
        feature: crate::ProfileFeature::NulString, location: actual,
    }) if actual == location)
    );
}

#[test]
fn nonnumeric_lower_bounds_still_validate_upper_bounds() {
    let source = input(vec![SourceTerm::Interval {
        lower: Box::new(SourceTerm::Symbolic(Symbol::Supremum)),
        upper: Box::new(SourceTerm::Symbolic(Symbol::String("bad\0".into()))),
    }]);
    assert!(matches!(
        facts(
            &source,
            &mut family_budget(usize::MAX),
            crate::test_support::location()
        ),
        Err(ExpansionFailure::Admission(
            crate::AdmissionFailure::Profile {
                feature: crate::ProfileFeature::NulString,
                ..
            }
        ))
    ));
}

#[test]
fn numeric_endpoint_values_are_emitted_once() {
    for value in [i32::MIN, i32::MAX] {
        assert_eq!(
            heads(&fact(value, value), &mut family_budget(usize::MAX)),
            vec![vec![Value::Number(value)]]
        );
    }
}

#[test]
fn range_width_does_not_allocate_expanded_columns() {
    let location = crate::test_support::location();
    let retained = |facts: &Facts| {
        size_of::<Facts>()
            + facts.products.capacity() * size_of::<Product>()
            + facts
                .products
                .iter()
                .map(Product::retained_bytes)
                .sum::<usize>()
    };
    let small = facts(&fact(1, 3), &mut family_budget(usize::MAX), location)
        .unwrap()
        .unwrap();
    let mut large = facts(
        &fact(1, 1_000_000),
        &mut family_budget(usize::MAX),
        location,
    )
    .unwrap()
    .unwrap();
    assert_eq!(retained(&small), retained(&large));
    assert!(retained(&large) < 4096);
    let budget = family_budget(usize::MAX);
    assert_eq!(
        large.next(&budget, location).unwrap().unwrap().terms(),
        &[Term::Constant(Value::Number(1))]
    );
    assert_eq!(large.len(), 999_999);
}

#[test]
fn product_cardinality_does_not_allocate_fact_carriers() {
    let location = crate::test_support::location();
    let mut products = facts(
        &input(vec![range(1, 100), range(1, 100)]),
        &mut family_budget(usize::MAX),
        location,
    )
    .unwrap()
    .unwrap();
    assert_eq!(products.len(), 10_000);
    assert_eq!(products.products.len(), 1);
    assert!(products.products[0].retained_bytes() < 4096);
    let budget = family_budget(usize::MAX);
    for second in 1..=100 {
        let head = products.next(&budget, location).unwrap().unwrap();
        assert_eq!(
            head.terms(),
            &[
                Term::Constant(Value::Number(1)),
                Term::Constant(Value::Number(second))
            ]
        );
    }
    let head = products.next(&budget, location).unwrap().unwrap();
    assert_eq!(
        head.terms(),
        &[
            Term::Constant(Value::Number(2)),
            Term::Constant(Value::Number(1))
        ]
    );
}

#[test]
fn copy_refusal_keeps_the_first_excess_argument_charge() {
    let source = input(vec![
        SourceTerm::Pool(vec![
            SourceTerm::Symbolic(Symbol::String("a".into())),
            SourceTerm::Symbolic(Symbol::String("bbb".into())),
        ]),
        range(1, 2),
    ]);
    // Four predicate-name bytes plus four scalar-column bytes precede copies
    // of a,a,bbb,bbb. A ceiling of 12 accepts two one-byte copies, then refuses
    // the complete three-byte argument at observed=13, retaining usage=10.
    let mut budget = Budget::new(
        crate::ExpansionLimits {
            max_scalar_bytes: 12,
            ..Default::default()
        },
        100,
    );
    let error = facts(&source, &mut budget, crate::test_support::location()).unwrap_err();
    assert!(matches!(
        error,
        ExpansionFailure::Limit {
            resource: crate::ExpansionResource::ScalarBytes,
            limit: 12,
            observed: 13,
            ..
        }
    ));
    assert_eq!(budget.usage().scalar_bytes, 10);
}

#[test]
fn copied_scalar_bytes_are_preflighted_once() {
    let source = input(vec![
        SourceTerm::Pool(vec![
            SourceTerm::Symbolic(Symbol::String("a".into())),
            SourceTerm::Symbolic(Symbol::String("bbb".into())),
        ]),
        range(1, 2),
    ]);
    let mut budget = Budget::new(
        crate::ExpansionLimits {
            max_scalar_bytes: 16,
            ..Default::default()
        },
        100,
    );
    let mut products = facts(&source, &mut budget, crate::test_support::location())
        .unwrap()
        .unwrap();
    assert_eq!(budget.usage().scalar_bytes, 16);
    let accepted = budget.usage();
    while products
        .next(&budget, crate::test_support::location())
        .unwrap()
        .is_some()
    {}
    assert_eq!(budget.usage(), accepted);
}

#[test]
fn an_interrupted_fact_cursor_cannot_resume_its_prefix() {
    let cancellation = Cancellation::default();
    let mut budget = family_budget(usize::MAX).with_cancellation(Some(cancellation.clone()));
    let location = crate::test_support::location();
    let mut products = facts(&fact(1, 3), &mut budget, location).unwrap().unwrap();
    assert!(products.next(&budget, location).unwrap().is_some());
    let accepted = budget.usage();
    cancellation.cancel();
    assert!(
        matches!(products.next(&budget, location), Err(ExpansionFailure::Interrupted {
        reason: Stop::Cancelled, location: actual,
    }) if actual == location)
    );
    assert_eq!(products.len(), 2);
    assert!(products.next(&budget, location).unwrap().is_none());
    assert_eq!(budget.usage(), accepted);
}

fn alternatives(rows: Vec<Vec<SourceTerm>>) -> WithProvenance<Statement> {
    WithProvenance::constructed(
        Rule::fact(Atom {
            name: Name::new("p").unwrap(),
            sign: Sign::Positive,
            arguments: Arguments::Pooled(rows),
        })
        .into(),
    )
}

#[test]
fn alternative_arities_keep_their_product_order() {
    let source = alternatives(vec![
        vec![range(1, 2)],
        vec![3.into(), range(4, 5)],
        Vec::new(),
    ]);
    let location = crate::test_support::location();
    let mut budget = family_budget(usize::MAX);
    let mut products = facts(&source, &mut budget, location).unwrap().unwrap();
    assert_eq!(products.len(), 5);
    let mut arities = Vec::new();
    while let Some(head) = products.next(&budget, location).unwrap() {
        arities.push(head.predicate().arity());
    }
    assert_eq!(arities, vec![1, 1, 2, 2, 0]);
    assert_eq!(
        heads(&source, &mut family_budget(usize::MAX)),
        vec![
            vec![Value::Number(1)],
            vec![Value::Number(2)],
            vec![Value::Number(3), Value::Number(4)],
            vec![Value::Number(3), Value::Number(5)],
            Vec::new(),
        ]
    );
}

#[test]
fn later_invalid_alternatives_refuse_before_emission() {
    let source = alternatives(vec![
        vec![range(1, 100)],
        vec![SourceTerm::Variable(Variable::Anonymous)],
    ]);
    let location = crate::test_support::location();
    let mut budget = Budget::new(
        crate::ExpansionLimits {
            max_origin_locations: 0,
            ..Default::default()
        },
        1000,
    );
    assert!(matches!(facts(&source, &mut budget, location),
        Err(ExpansionFailure::Evaluation { error: EvalError::NotGround { variable: Variable::Anonymous },
            location: actual }) if actual == location));
    // No cursor was returned for a consumer to replace this error with its
    // zero Origins ceiling, although the preceding alternative was checked.
    assert_eq!(budget.usage().templates, 100);
    assert_eq!(budget.usage().origin_locations, 0);
}

#[test]
fn pool_validation_keeps_its_reverse_leaf_order() {
    let source = input(vec![SourceTerm::Pool(vec![
        SourceTerm::Variable(Variable::Anonymous),
        SourceTerm::Symbolic(Symbol::String("bad\0".into())),
    ])]);
    assert!(matches!(
        facts(
            &source,
            &mut family_budget(usize::MAX),
            crate::test_support::location()
        ),
        Err(ExpansionFailure::Admission(
            crate::AdmissionFailure::Profile {
                feature: crate::ProfileFeature::NulString,
                ..
            }
        ))
    ));
}

#[test]
fn complete_i32_ranges_refuse_without_numeric_materialization() {
    let mut budget = Budget::new(crate::ExpansionLimits::default(), usize::MAX);
    assert!(matches!(
        facts(
            &fact(i32::MIN, i32::MAX),
            &mut budget,
            crate::test_support::location()
        ),
        Err(ExpansionFailure::Limit {
            resource: crate::ExpansionResource::Templates,
            observed: 4_294_967_296,
            ..
        })
    ));
    assert_eq!(budget.usage().templates, 0);
}

#[test]
fn every_scalar_copy_limit_keeps_the_original_prefix() {
    let source = input(vec![
        SourceTerm::Pool(vec![
            SourceTerm::Symbolic(Symbol::String("a".into())),
            SourceTerm::Symbolic(Symbol::String("bbb".into())),
        ]),
        range(1, 2),
    ]);
    for limit in 0..=16 {
        let mut expected = 0_u128;
        let mut first_excess = None;
        for charge in [4_u128, 1, 3, 1, 0, 1, 0, 3, 0, 3, 0] {
            if expected + charge > limit as u128 {
                first_excess = Some(expected + charge);
                break;
            }
            expected += charge;
        }
        let mut budget = Budget::new(
            crate::ExpansionLimits {
                max_scalar_bytes: limit,
                ..Default::default()
            },
            100,
        );
        let result = facts(&source, &mut budget, crate::test_support::location());
        match first_excess {
            Some(expected) => assert!(matches!(result, Err(ExpansionFailure::Limit {
                resource: crate::ExpansionResource::ScalarBytes, observed, ..
            }) if observed == expected)),
            None => assert_eq!(result.unwrap().unwrap().len(), 4),
        }
        assert_eq!(budget.usage().scalar_bytes as u128, expected);
    }
}

#[test]
fn relational_admission_keeps_each_expanded_facts_evidence() {
    let source = "p(1..2,3..4).";
    let admitted = crate::admit_extended(
        source.into(),
        crate::AdmissionOptions::default(),
        crate::ExpansionLimits::default(),
    )
    .unwrap();
    assert_eq!(admitted.program().templates().len(), 4);
    assert_eq!(admitted.template_origins().len(), 4);
    let first = &admitted.template_origins()[0];
    assert!(!first.is_empty());
    assert!(
        admitted
            .template_origins()
            .iter()
            .all(|origins| origins == first)
    );
    for origin in first {
        assert_eq!(admitted.source().slice(origin.span).unwrap(), source);
    }
}

#[test]
fn formula_admission_keeps_expanded_analysis_and_root_evidence() {
    let source = "p(1..2,3..4).";
    let admitted = crate::test_support::formula(source);
    assert_eq!(admitted.analyzed_program().statements().count(), 4);
    assert_eq!(admitted.atoms().len(), 4);
    assert_eq!(admitted.expansion_usage().templates, 4);
    assert_eq!(admitted.expansion_usage().values, 12);
    assert!(!admitted.formula_origins().is_empty());
    for origins in admitted.formula_origins() {
        assert!(!origins.is_empty());
        for origin in origins {
            let original = origin.location().expect("parsed fact keeps its source");
            assert_eq!(
                admitted.source().unwrap().slice(original.span).unwrap(),
                source
            );
        }
    }
}
