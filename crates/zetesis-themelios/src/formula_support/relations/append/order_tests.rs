use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Span};
use zetesis_core::ValueNodeRef;

use super::*;
use crate::formula_support::relations::SupportCatalog;
use zetesis_test_support::programs::unary;

fn location() -> Location {
    Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    }
}

fn populate(append: &mut SupportAppend<'_>, values: &[i32], counters: &mut Counters) {
    for &value in values {
        append
            .atom(
                (&unary("row", value)).into(),
                &FormulaLimits::default(),
                counters,
                location(),
            )
            .unwrap();
    }
}

fn numbers(append: &SupportAppend<'_>) -> Vec<i32> {
    append
        .atoms()
        .map(|atom| match atom.values().at(0).unwrap().descriptor() {
            ValueNodeRef::Number(value) => value,
            _ => panic!("numeric ordering fixture"),
        })
        .collect()
}

#[test]
fn dense_order_reuses_the_semantic_index() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let (_, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    let values: Vec<_> = (0..64).rev().collect();
    populate(&mut append, &values, &mut counters);
    assert!(
        append
            .selected_order_limits(&limits, &counters, location())
            .unwrap()
            .is_some()
    );
    let mut comparison = append.pending.clone();
    let mut comparison_counters = Counters::default();
    crate::formula_support::sort::by(
        &mut comparison,
        GroundingWork::new(&limits, &mut comparison_counters, location()),
        |left, right, work| {
            append
                .owner
                .get(*left)
                .unwrap()
                .compare_ref_with(append.owner.get(*right).unwrap(), || {
                    work.counters.work(work.limits, work.location)
                })
        },
    )
    .unwrap();
    let before = counters.accounting.work;
    append.order(&limits, &mut counters, location()).unwrap();
    assert_eq!(*append.pending, comparison);
    // Replacing index traversal with the comparison route breaks this bound.
    assert!(counters.accounting.work - before < comparison_counters.accounting.work);
}

#[test]
fn scratch_refusal_selects_the_comparison_route() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let (_, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    populate(&mut append, &[3, 1, 2], &mut counters);
    let required = append
        .owner
        .selected_order_storage(append.pending.len())
        .unwrap()
        + append.outer_bytes(counters.accounting.workspace.bytes());
    let short = FormulaLimits {
        max_support_bytes: usize::try_from(required - 1).unwrap(),
        ..limits
    };
    assert!(
        append
            .selected_order_limits(&short, &counters, location())
            .unwrap()
            .is_none()
    );
    append.order(&short, &mut counters, location()).unwrap();
    assert_eq!(numbers(&append), [1, 2, 3]);
}

#[test]
fn selected_order_does_not_retry_a_work_refusal() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let (_, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    populate(&mut append, &[3, 1, 2], &mut counters);
    assert!(
        append
            .selected_order_limits(&limits, &counters, location())
            .unwrap()
            .is_some()
    );
    let stopped = FormulaLimits {
        max_work: counters.accounting.work,
        ..limits
    };
    assert!(matches!(
        append.order(&stopped, &mut counters, location()),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            ..
        })
    ));
    assert_eq!(numbers(&append), [3, 1, 2]);
}

fn sparse_work(history: i32) -> u64 {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    for value in 0..history {
        catalog = catalog
            .insert(&unary("row", value), &limits, &mut counters, location())
            .unwrap();
    }
    let (_, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    populate(&mut append, &[-1, -2], &mut counters);
    assert!(
        append
            .selected_order_limits(&limits, &counters, location())
            .unwrap()
            .is_none()
    );
    let before = counters.accounting.work;
    append.order(&limits, &mut counters, location()).unwrap();
    assert_eq!(numbers(&append), [-2, -1]);
    counters.accounting.work - before
}

#[test]
fn sparse_order_work_is_independent_of_history() {
    assert_eq!(sparse_work(64), sparse_work(128));
}

#[test]
fn pending_order_can_include_an_older_discovery() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let earlier = {
        let (_, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
        let earlier = append
            .discover_ref(
                (&unary("row", 1)).into(),
                0,
                &limits,
                &mut counters,
                location(),
            )
            .unwrap();
        populate(&mut append, &[0], &mut counters);
        earlier
    };
    catalog.publish(&limits, &mut counters, location()).unwrap();
    let (_, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    populate(&mut append, &[3, 2], &mut counters);
    append
        .retain(&earlier, 0, &limits, &mut counters, location())
        .unwrap();
    assert!(
        append
            .selected_order_limits(&limits, &counters, location())
            .unwrap()
            .is_some()
    );
    append.order(&limits, &mut counters, location()).unwrap();
    assert_eq!(numbers(&append), [1, 2, 3]);
}
