use super::*;
use crate::formula_support::testing::Fixture;
use crate::test_support::location;
use crate::{ExpansionLimits, FormulaResource};
use zetesis_core::catalog::AssignmentError;
use zetesis_core::{Atom, Predicate, Term, Value, ValueLimits};

fn fixture() -> (Fixture, AtomPattern) {
    let mut fixture = Fixture::default();
    let pattern = fixture.pattern(
        &zetesis_core::AtomPattern::new(
            Predicate::with_sign("prepared", 3, zetesis_core::Sign::Negative).unwrap(),
            vec![
                Term::Variable(1),
                Term::Constant(Value::Number(7)),
                Term::Variable(1),
            ],
        )
        .unwrap(),
        location(),
    );
    (fixture, pattern)
}

fn sequence(prepared: bool) -> (Vec<Atom>, u64) {
    let (mut fixture, pattern) = fixture();
    let limits = FormulaLimits::default();
    fixture.with(location(), |_, computation, counters| {
        let mut binding = Binding::new(computation, &limits, counters, location()).unwrap();
        binding
            .extend_scope(2, computation, &limits, counters, location())
            .unwrap();
        let mut budget = crate::expansion::Budget::new(ExpansionLimits::default(), usize::MAX);
        let mut derivation = super::super::Derivation {
            normal_head: None,
            computation,
            counters,
            budget: &mut budget,
            limits: &limits,
        };
        let mut work = 0;
        let mut result = Vec::new();
        for value in (0..32).chain([0, 7, 31]) {
            let key = derivation
                .computation
                .number(value, &limits, derivation.counters, location())
                .unwrap();
            binding
                .set(1, &key, &limits, derivation.counters, location())
                .unwrap();
            let start = derivation.counters.accounting.work;
            if prepared {
                derivation
                    .normal_head(pattern, &binding, location())
                    .unwrap();
                assert!(matches!(
                    derivation.normal_head.as_ref().unwrap().pattern,
                    Pattern::Prepared(_)
                ));
            } else {
                derivation.head(pattern, &binding, location()).unwrap();
            }
            work += derivation.counters.accounting.work - start;
            let pattern = derivation
                .computation
                .static_pattern(pattern, &limits, derivation.counters, location())
                .unwrap();
            assert!(
                derivation
                    .computation
                    .contains_pattern(pattern, &binding, &limits, derivation.counters, location())
                    .unwrap()
            );
            let source = derivation
                .computation
                .atom(pattern, &binding, &limits, derivation.counters, location())
                .unwrap();
            result.push(
                derivation
                    .computation
                    .source_atom(&source, &limits, derivation.counters, location())
                    .unwrap()
                    .to_atom(ValueLimits::default())
                    .unwrap(),
            );
        }
        (result, work)
    })
}

#[test]
fn prepared_heads_preserve_the_complete_producer_image() {
    let (prepared, _) = sequence(true);
    let (general, _) = sequence(false);
    assert_eq!(prepared, general);
    assert_eq!(prepared.len(), 35);
}

#[test]
fn prepared_heads_remove_repeated_validation_work() {
    let prepared_work = sequence(true).1;
    let general_work = sequence(false).1;
    assert!(
        prepared_work < general_work,
        "prepared={prepared_work}, general={general_work}"
    );
}

#[test]
fn prepared_head_metadata_has_one_live_owner() {
    let (mut fixture, pattern) = fixture();
    let limits = FormulaLimits::default();
    fixture.with(location(), |support, computation, counters| {
        let before = support.workspace_bytes();
        let head = Head::new(pattern, computation, &limits, counters, location()).unwrap();
        assert!(matches!(head.pattern, Pattern::Prepared(_)));
        assert_eq!(support.workspace_bytes(), before + size_of::<Head<'_>>());
        drop(head);
        assert_eq!(support.workspace_bytes(), before);
    });
    // A new support-round view prepares from its own source metadata and lease.
    fixture.with(location(), |support, computation, counters| {
        let before = support.workspace_bytes();
        let head = Head::new(pattern, computation, &limits, counters, location()).unwrap();
        assert_eq!(support.workspace_bytes(), before + size_of::<Head<'_>>());
        drop(head);
        assert_eq!(support.workspace_bytes(), before);
    });
}

#[test]
fn prepared_head_storage_requires_the_complete_header() {
    let (mut fixture, pattern) = fixture();
    fixture.with(location(), |support, computation, counters| {
        let before = support.workspace_bytes();
        let required = support.live_bytes() + size_of::<Head<'_>>();
        let exact = FormulaLimits {
            max_support_bytes: required,
            ..FormulaLimits::default()
        };
        let head = Head::new(pattern, computation, &exact, counters, location()).unwrap();
        drop(head);
        let bounded = FormulaLimits {
            max_support_bytes: required - 1,
            ..exact
        };
        assert!(matches!(
            Head::new(pattern, computation, &bounded, counters, location()),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                ..
            })
        ));
        assert_eq!(support.workspace_bytes(), before);
    });
}

#[test]
fn prepared_heads_preserve_missing_binding_evidence() {
    let (mut fixture, pattern) = fixture();
    let limits = FormulaLimits::default();
    fixture.with(location(), |_, computation, counters| {
        let head = Head::new(pattern, computation, &limits, counters, location()).unwrap();
        let mut binding = Binding::new(computation, &limits, counters, location()).unwrap();
        binding
            .extend_scope(2, computation, &limits, counters, location())
            .unwrap();
        let shared = head
            .derive(&binding, computation, &limits, counters, location())
            .unwrap_err();
        let pattern = computation
            .static_pattern(pattern, &limits, counters, location())
            .unwrap();
        let general = computation
            .atom(pattern, &binding, &limits, counters, location())
            .unwrap_err();
        for error in [shared, general] {
            assert!(matches!(error, FormulaFailure::TermAssignment {
                error: AssignmentError::Unbound { slot: 1 }, location: actual,
            } if actual == location()));
        }
    });
}

#[test]
fn prepared_head_work_refusal_does_not_select_support() {
    let (mut fixture, pattern) = fixture();
    let limits = FormulaLimits::default();
    fixture.with(location(), |_, computation, counters| {
        let head = Head::new(pattern, computation, &limits, counters, location()).unwrap();
        let mut binding = Binding::new(computation, &limits, counters, location()).unwrap();
        binding
            .extend_scope(2, computation, &limits, counters, location())
            .unwrap();
        let key = computation
            .number(4, &limits, counters, location())
            .unwrap();
        binding.set(1, &key, &limits, counters, location()).unwrap();
        let bounded = FormulaLimits {
            max_work: counters.accounting.work,
            ..limits
        };
        assert!(matches!(
            head.derive(&binding, computation, &bounded, counters, location()),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ));
        let pattern = computation
            .static_pattern(pattern, &limits, counters, location())
            .unwrap();
        assert!(
            !computation
                .contains_pattern(pattern, &binding, &limits, counters, location())
                .unwrap()
        );
        head.derive(&binding, computation, &limits, counters, location())
            .unwrap();
        assert!(
            computation
                .contains_pattern(pattern, &binding, &limits, counters, location())
                .unwrap()
        );
    });
}
