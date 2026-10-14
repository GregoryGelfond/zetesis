//! Rejected rows keep source validation without retaining a formula.

use super::*;
use crate::formula_ground::{Purpose, ground};
use crate::formula_support::{Support, testing};
use crate::test_support::location;
use crate::{ExpansionFailure, ExpansionLimits, FormulaLimits, FormulaResource};
use themelios_program::program::DefaultNegation;
use zetesis_core::{AtomPattern, Predicate, Term, Value, ValueLimits};
use zetesis_cpu::{Cancellation, Stop};

fn with_atom<T>(
    value: Option<Value>,
    limits: &FormulaLimits,
    action: impl for<'source> FnOnce(
        &mut Builder<'_, '_, 'source>,
        &Support<'source>,
        &Binding<'_>,
        crate::formula_support::components::Pattern,
    ) -> T,
) -> T {
    with_arguments(value, vec![Term::Variable(0)], limits, action)
}

fn with_arguments<T>(
    value: Option<Value>,
    arguments: Vec<Term>,
    limits: &FormulaLimits,
    action: impl for<'source> FnOnce(
        &mut Builder<'_, '_, 'source>,
        &Support<'source>,
        &Binding<'_>,
        crate::formula_support::components::Pattern,
    ) -> T,
) -> T {
    let mut fixture = testing::Fixture::default();
    let pattern = fixture.pattern(
        &AtomPattern::new(Predicate::new("p", arguments.len()).unwrap(), arguments).unwrap(),
        location(),
    );
    fixture.with(location(), |support, computation, counters| {
        let assignment = testing::binding(&[value], computation, counters, location());
        let mut budget = crate::expansion::Budget::new(ExpansionLimits::default(), usize::MAX);
        let mut builder = Builder::empty(
            computation,
            limits,
            &mut budget,
            counters,
            Purpose::Theory,
            None,
            location(),
        )
        .unwrap();
        action(&mut builder, support, &assignment, pattern)
    })
}

#[test]
fn rejected_rows_keep_out_of_range_binding_errors() {
    with_arguments(
        None,
        vec![Term::Variable(1)],
        &FormulaLimits::default(),
        |builder, support, row, pattern| {
            let body = [LiteralIr::Atom(DefaultNegation::Not, pattern)];
            let failure = builder
                .validate_body(&body, row, support, location())
                .unwrap_err();
            assert!(
                matches!(failure, FormulaFailure::TermAssignment {
            error: zetesis_core::catalog::AssignmentError::Slot { slot: 1, len: 1 },
            location: site,
        } if site == location()),
                "{failure:?}"
            );
        },
    );
}

#[test]
fn streamed_keys_keep_unsafe_variable_errors() {
    for variable in [0, 1] {
        with_arguments(
            None,
            vec![Term::Variable(variable)],
            &FormulaLimits::default(),
            |builder, _, row, pattern| {
                let failure = builder
                    .validate_atom_keys(
                        &[LiteralIr::Atom(DefaultNegation::Not, pattern)],
                        row,
                        location(),
                        |variable, location| FormulaFailure::UnsafeVariable { variable, location },
                    )
                    .unwrap_err();
                assert!(
                    matches!(failure, FormulaFailure::UnsafeVariable {
                variable: actual, location: site,
            } if actual == variable && site == location()),
                    "{failure:?}"
                );
            },
        );
    }
}

#[test]
fn rejected_keys_account_for_every_argument() {
    let cost = |arguments| {
        with_arguments(
            Some(Value::Number(1)),
            arguments,
            &FormulaLimits::default(),
            |builder, support, row, pattern| {
                let before = builder.counters.accounting.work;
                builder
                    .validate_body(
                        &[LiteralIr::Atom(DefaultNegation::None, pattern)],
                        row,
                        support,
                        location(),
                    )
                    .unwrap();
                builder.counters.accounting.work - before
            },
        )
    };
    let one = cost(vec![Term::Variable(0)]);
    let repeated = cost(vec![Term::Variable(0); 64]);
    let constants = cost(vec![Term::Constant(Value::Number(1)); 64]);
    assert!(
        repeated >= one + 63,
        "repeated arguments still require key validation"
    );
    assert!(
        constants >= one + 63,
        "constant arguments still require key validation"
    );
}

#[test]
fn rejected_flat_rows_need_no_formula_nodes() {
    let mut limits = FormulaLimits::default();
    limits.theory.max_nodes = 0;
    with_atom(
        Some(Value::Number(7)),
        &limits,
        |builder, support, row, pattern| {
            // The caller has already rejected a complete row. Any discarded
            // formula construction would fail its very first node at this ceiling.
            let body = [
                LiteralIr::Atom(DefaultNegation::None, pattern),
                LiteralIr::Atom(DefaultNegation::Not, pattern),
                LiteralIr::Atom(DefaultNegation::NotNot, pattern),
            ];
            builder
                .validate_body(&body, row, support, location())
                .unwrap();
            assert_eq!(builder.nodes.view().len(), 0);
            assert_eq!(builder.catalog.len(), 0);
            assert!(builder.roots.is_empty());
        },
    );
}

#[test]
fn rejected_flat_rows_keep_unsafe_binding_locations() {
    with_atom(
        None,
        &FormulaLimits::default(),
        |builder, support, row, pattern| {
            let site = crate::ProgramSite::source(themelios_base::span::Location {
                source: themelios_base::source::SourceId::new(9),
                span: themelios_base::span::Span::empty(themelios_base::span::ByteOffset::new(17)),
            });
            let failure = builder
                .validate_body(
                    &[LiteralIr::Atom(DefaultNegation::Not, pattern)],
                    row,
                    support,
                    site,
                )
                .unwrap_err();
            assert!(
                matches!(failure, FormulaFailure::TermAssignment { error: zetesis_core::catalog::AssignmentError::Unbound { slot: 0 }, location } if location == site),
                "{failure:?}"
            );
            assert_eq!(builder.nodes.view().len(), 0);
            assert_eq!(builder.catalog.len(), 0);
        },
    );
}

#[test]
fn rejected_flat_rows_preserve_work_refusals() {
    let limits = FormulaLimits {
        max_work: 1_000_000,
        ..FormulaLimits::default()
    };
    with_atom(
        Some(Value::Number(1)),
        &limits,
        |builder, support, row, pattern| {
            builder.counters.accounting.work = limits.max_work;
            let failure = builder
                .validate_body(
                    &[LiteralIr::Atom(DefaultNegation::None, pattern)],
                    row,
                    support,
                    location(),
                )
                .unwrap_err();
            assert!(matches!(failure, FormulaFailure::Limit {
            resource: FormulaResource::Work,
            observed,
            limit,
            location: site,
        } if observed == u128::from(limits.max_work) + 1
            && limit == u128::from(limits.max_work) && site == location()));
            assert_eq!(builder.nodes.view().len(), 0);
        },
    );
}

#[test]
fn rejected_flat_rows_preserve_cancellation() {
    with_atom(
        Some(Value::Number(1)),
        &FormulaLimits::default(),
        |builder, support, row, pattern| {
            let cancellation = Cancellation::default();
            *builder.counters =
                std::mem::take(builder.counters).with_cancellation(Some(&cancellation));
            cancellation.cancel();
            let failure = builder
                .validate_body(
                    &[LiteralIr::Atom(DefaultNegation::None, pattern)],
                    row,
                    support,
                    location(),
                )
                .unwrap_err();
            assert!(matches!(failure, FormulaFailure::Interrupted {
            reason: Stop::Cancelled,
            location: site,
        } if site == location()));
            assert_eq!(builder.nodes.view().len(), 0);
        },
    );
}

#[test]
fn scoped_and_generated_rows_keep_scoped_validation() {
    for source in [
        "d(1). p :- d(X),X=2,#count{Y:q(Y)}=0.",
        "d(1). p(X,Y) :- d(X),Y=X+1,X=2.",
        "d(1). p(X,Y) :- d(X),Y=X..X+1,X=2.",
    ] {
        testing::with_completed_source(source, |prepared, support, computation, counters| {
            let rule = prepared
                .rules_without_facts()
                .iter()
                .find(|rule| !rule.body.is_empty())
                .unwrap();
            assert!(
                rule.body.iter().any(|literal| matches!(
                    literal,
                    LiteralIr::Aggregate(_) | LiteralIr::Bind { .. } | LiteralIr::Range { .. }
                )),
                "fixture must retain its scoped/generated route: {source}"
            );
            let values = vec![Some(Value::Number(1)); rule.variables];
            let row = testing::binding(&values, computation, counters, rule.location);
            let mut limits = FormulaLimits::default();
            limits.theory.max_nodes = 0;
            let mut budget = crate::expansion::Budget::new(ExpansionLimits::default(), usize::MAX);
            let mut builder = Builder::empty(
                computation,
                &limits,
                &mut budget,
                counters,
                Purpose::Theory,
                None,
                rule.location,
            )
            .unwrap();
            let failure = builder
                .validate_body(&rule.body, &row, support, rule.location)
                .unwrap_err();
            assert!(
                matches!(failure, FormulaFailure::Limit {
                resource: FormulaResource::Nodes, limit: 0, location, ..
            } if location == rule.location),
                "{source}: {failure}"
            );
        });
    }
}

#[test]
fn rejected_rows_leave_the_original_theory_unchanged() {
    let original = ground(testing::prepare("{d(1);d(2)}."), None, None).unwrap();
    let actual = ground(
        testing::prepare("{d(1);d(2)}. p(X) :- d(X),X>2."),
        None,
        None,
    )
    .unwrap();
    let atoms = |compiled: &crate::formula::Compiled| {
        compiled
            .atoms
            .atoms()
            .iter()
            .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
            .collect::<Vec<_>>()
    };
    // Exact vocabulary and graph identity entail identical original and
    // arbitrary frozen satisfaction, not just matching answer counts.
    assert_eq!(atoms(&actual), atoms(&original));
    assert_eq!(actual.theory.nodes(), original.theory.nodes());
    assert_eq!(actual.theory.operands(), original.theory.operands());
    assert_eq!(actual.theory.roots(), original.theory.roots());
}

#[test]
fn false_bodies_do_not_evaluate_heads() {
    let actual = ground(testing::prepare("d(0). p(1/X) :- d(X),X!=0."), None, None).unwrap();
    let atoms = actual
        .atoms
        .atoms()
        .iter()
        .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        atoms,
        vec![zetesis_test_support::programs::atom(
            "d",
            vec![Value::Number(0)]
        )]
    );
}

#[test]
fn flat_source_families_keep_zero_divisor_warnings() {
    let actual = ground(testing::prepare("d(0;2). p(X) :- d(X),1/X=1."), None, None).unwrap();
    assert_eq!(actual.warnings.len(), 1);
    assert_eq!(actual.atoms.atoms().len(), 2);
}

#[test]
fn flat_source_families_keep_independent_overflow() {
    let result = ground(
        testing::prepare("d(0). e(1). p(X) :- d(X),e(Y),1/X-(2147483647+Y)=0."),
        None,
        None,
    );
    assert!(matches!(
        result,
        Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation {
            error: crate::observation::EvaluationError::Overflow,
            ..
        }))
    ));
}
