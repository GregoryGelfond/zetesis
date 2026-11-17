use super::*;
use crate::formula_binding::Binding;
use crate::formula_support::{Accounting, SupportCatalog, testing};
use crate::grounding_observer::Work;
use crate::test_support::location;
use zetesis_algebra::Cancellation;
use zetesis_core::{AtomPattern, PatternRef, Predicate, Term, Value};

fn owner() -> (SupportCatalog, crate::formula_support::components::Pattern) {
    let mut catalog = SupportCatalog::default();
    let pattern = testing::admit_pattern(
        &mut catalog,
        &AtomPattern::new(
            Predicate::new("pending", 1).unwrap(),
            vec![Term::Constant(Value::Number(4))],
        )
        .unwrap(),
        &mut Counters::default(),
        location(),
    );
    (catalog, pattern)
}

fn pending<'source>(
    pattern: crate::formula_support::components::Pattern,
    append: &mut SupportAppend<'source>,
    support: &Support<'source>,
    counters: &mut Counters,
) -> (
    crate::formula_support::SourceAtom,
    Binding<'static>,
    PatternRef<'source>,
) {
    let limits = FormulaLimits::default();
    let mut computation = Computation::new(append, support);
    let binding = Binding::new(&computation, &limits, counters, location()).unwrap();
    let pattern = computation
        .static_pattern(pattern, &limits, counters, location())
        .unwrap();
    let atom = computation
        .atom(pattern, &binding, &limits, counters, location())
        .unwrap();
    computation
        .support(&atom, &limits, counters, location())
        .unwrap();
    (atom, binding, pattern)
}

#[test]
fn borrowed_lookup_reuses_pending_membership_and_exact_lookup_work() {
    let (mut catalog, pattern) = owner();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let (relations, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
    let (_, binding, pattern) = pending(pattern, &mut append, &support, &mut counters);
    assert!(
        Computation::new(&mut append, &support)
            .pending_reader()
            .is_some()
    );
    let before_bytes = support.live_bytes();
    let before = counters.accounting.work;
    assert!(
        Computation::new(&mut append, &support)
            .contains_pattern(pattern, &binding, &limits, &mut counters, location())
            .unwrap()
    );
    let ordinary_work = counters.accounting.work - before;
    let borrowed =
        Computation::borrowed(&append, &support, &limits, &mut counters, location()).unwrap();
    assert!(borrowed.pending_reader().is_some());
    let before = counters.accounting.work;
    assert!(
        borrowed
            .contains_pattern(pattern, &binding, &limits, &mut counters, location())
            .unwrap()
    );
    assert_eq!(counters.accounting.work - before, ordinary_work);
    assert!(ordinary_work > 0);
    assert_eq!(support.live_bytes(), before_bytes);
    assert_eq!(append.atoms().count(), 1);
    assert!(
        Computation::frozen(append.term_lookup(), &support)
            .pending_reader()
            .is_none()
    );
}

#[test]
fn borrowed_computation_cannot_publish_atoms_or_admit_missing_terms() {
    let (mut catalog, pattern) = owner();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let (relations, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
    let (atom, binding, pattern) = pending(pattern, &mut append, &support, &mut counters);
    let before_bytes = support.live_bytes();
    let mut borrowed =
        Computation::borrowed(&append, &support, &limits, &mut counters, location()).unwrap();
    for result in [
        borrowed
            .atom(pattern, &binding, &limits, &mut counters, location())
            .map(|_| ()),
        borrowed.support(&atom, &limits, &mut counters, location()),
    ] {
        assert!(matches!(
            result,
            Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                ..
            })
        ));
    }
    assert!(matches!(
        borrowed.number(731, &limits, &mut counters, location()),
        Err(FormulaFailure::UnadmittedTerm { .. })
    ));
    assert!(matches!(
        borrowed.import(
            (&Value::String("not admitted".into())).into(),
            &limits,
            &mut counters,
            location()
        ),
        Err(FormulaFailure::UnadmittedTerm { .. })
    ));
    let admitted = borrowed
        .number(4, &limits, &mut counters, location())
        .unwrap();
    assert_eq!(
        borrowed.read().term(&admitted).unwrap().descriptor(),
        ValueNodeRef::Number(4)
    );
    let empty = borrowed.read().assignment();
    assert!(
        borrowed
            .construct(
                ValueNodeRef::Number(4),
                empty.as_slice(),
                &[],
                zetesis_core::catalog::Limits {
                    max_nodes: 0,
                    ..zetesis_core::catalog::Limits::default()
                },
                GroundingWork::new(&limits, &mut counters, location())
            )
            .is_err(),
        "caller constructor limits are not replaced by permissive lookup limits"
    );
    assert_eq!(support.live_bytes(), before_bytes);
    assert_eq!(append.atoms().count(), 1);
}

#[test]
fn a_borrowed_reader_requires_the_exact_live_split_owner() {
    let (mut first, _) = owner();
    let (mut second, _) = owner();
    let (closed, _) = owner();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let (first_rows, first_append) = first.split(&limits, &mut counters, location()).unwrap();
    let (second_rows, _) = second.split(&limits, &mut counters, location()).unwrap();
    let closed_rows = closed.snapshot(&limits, &mut counters, location()).unwrap();
    assert!(first_append.belongs_to(&first_rows));
    for rows in [&second_rows, &closed_rows] {
        let support = Support::indexed(rows, &limits, &counters, location()).unwrap();
        let before = counters.accounting.work;
        assert!(matches!(
            Computation::borrowed(&first_append, &support, &limits, &mut counters, location()),
            Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                ..
            })
        ));
        assert_eq!(counters.accounting.work, before + 1);
    }
    assert!(first_append.is_empty());
}

#[test]
fn pending_membership_preserves_exact_refusal_and_cancellation_receipts() {
    let (mut catalog, pattern) = owner();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let (relations, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
    let (_, binding, pattern) = pending(pattern, &mut append, &support, &mut counters);
    let borrowed =
        Computation::borrowed(&append, &support, &limits, &mut counters, location()).unwrap();
    let before = counters.accounting.work;
    assert!(
        borrowed
            .contains_pattern(pattern, &binding, &limits, &mut counters, location())
            .unwrap()
    );
    let complete = counters.accounting.work - before;
    for allowance in [0, 1, complete - 1] {
        let before = counters.accounting.work;
        let bounded = FormulaLimits {
            max_work: before + allowance,
            ..limits
        };
        assert!(matches!(
            borrowed.contains_pattern(pattern, &binding, &bounded, &mut counters, location()),
            Err(FormulaFailure::Limit {
                resource: crate::FormulaResource::Work,
                ..
            })
        ));
        assert_eq!(counters.accounting.work, before + allowance);
    }
    let bounded = FormulaLimits {
        max_support_bytes: 0,
        ..limits
    };
    assert!(matches!(
        borrowed.contains_pattern(pattern, &binding, &bounded, &mut counters, location()),
        Err(FormulaFailure::Limit {
            resource: crate::FormulaResource::SupportBytes,
            ..
        })
    ));
    let cancellation = Cancellation::default();
    let prior = zetesis_algebra::POLL_INTERVAL - 2;
    let mut account = Accounting {
        work: prior,
        ..Accounting::default()
    };
    let result = account.controlled(Some(&cancellation), Work::default(), location(), |local| {
        cancellation.cancel();
        borrowed.contains_pattern(pattern, &binding, &limits, local, location())
    });
    assert!(matches!(result, Err(FormulaFailure::Interrupted { .. })));
    assert!(account.work > prior && account.work <= zetesis_algebra::POLL_INTERVAL);
    assert_eq!(append.atoms().count(), 1);
}
