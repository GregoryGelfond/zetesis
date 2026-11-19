use zetesis_core::ValueNodeRef;

use super::*;
use crate::formula_support::relations::SupportCatalog;
use crate::test_support::location;
use zetesis_test_support::programs::unary;

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
fn keyed_order_matches_the_typed_comparison_for_less_work() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let (_, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    let values: Vec<_> = (0..64).rev().collect();
    populate(&mut append, &values, &mut counters);
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
    append
        .order(&limits, &mut counters, location(), None)
        .unwrap();
    assert_eq!(*append.pending, comparison);
    // Reading each atom once costs less than reading both atoms in every
    // comparison.
    assert!(counters.accounting.work - before < comparison_counters.accounting.work);
}

#[test]
fn a_scratch_refusal_leaves_the_round_unordered() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let (_, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    populate(&mut append, &[3, 1, 2], &mut counters);
    let live =
        append.owner.storage_bytes() + append.outer_bytes(counters.accounting.workspace.bytes());
    let short = FormulaLimits {
        max_support_bytes: usize::try_from(live).unwrap(),
        ..limits
    };
    assert!(matches!(
        append.order(&short, &mut counters, location(), None),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            ..
        })
    ));
    assert_eq!(numbers(&append), [3, 1, 2]);
}

#[test]
fn a_work_refusal_leaves_the_round_unordered() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let (_, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    populate(&mut append, &[3, 1, 2], &mut counters);
    let stopped = FormulaLimits {
        max_work: counters.accounting.work,
        ..limits
    };
    assert!(matches!(
        append.order(&stopped, &mut counters, location(), None),
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
    let before = counters.accounting.work;
    append
        .order(&limits, &mut counters, location(), None)
        .unwrap();
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
    append
        .order(&limits, &mut counters, location(), None)
        .unwrap();
    assert_eq!(numbers(&append), [1, 2, 3]);
}
