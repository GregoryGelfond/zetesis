use super::*;
use crate::FormulaResource;
use crate::expansion::Budget;
use crate::formula_support::row_blocks::tests::{JOIN, atom, prepared_source};
use crate::formula_support::{Binding, Ir, Support, delta, testing};
use zetesis_core::{Value, ValueLimits};

fn values(
    binding: &Binding<'_>,
    computation: &Computation<'_, '_>,
    location: ProgramSite,
) -> Vec<Value> {
    (0..binding.len())
        .map(|slot| {
            binding
                .read(slot, computation.read(), location)
                .unwrap()
                .to_value(ValueLimits::default())
                .unwrap()
        })
        .collect()
}

fn drain(
    cursor: &mut Join<'_, '_>,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: ProgramSite,
) -> Vec<Vec<Value>> {
    let mut result = Vec::new();
    while let Some(binding) = cursor
        .next_support(None, computation, limits, budget, counters, location)
        .unwrap()
    {
        result.push(values(&binding, computation, location));
    }
    result
}

/// Preparing an already-selected probe cannot replace its plan, spend planner
/// work, move the leading position, or remove the original cursor's owners.
fn assert_lent<'source>(
    rule: &'source RuleIr,
    cursor: &mut Join<'_, 'source>,
    computation: &mut Computation<'_, 'source>,
    limits: &FormulaLimits,
    budget: &Budget,
    counters: &mut Counters,
) {
    let first = cursor.plan.patterns[0];
    cursor
        .prepare_probe(
            first,
            Context::new(computation, limits, counters, rule.location),
        )
        .unwrap();
    let plan_pointer = std::ptr::from_ref(&*cursor.plan);
    let patterns_pointer = cursor.plan.patterns.as_ptr();
    let probe_pointer = std::ptr::from_ref(cursor.probes[0].as_ref().unwrap());
    let lease = cursor.lease.bytes();
    let position = cursor.positions.clone();
    let usage = budget.usage();
    let pivot = cursor.plan.pivot;
    let expected = {
        let mut selected = Vec::new();
        let mut position = 0;
        while let Some(row) = cursor.probes[0]
            .as_ref()
            .unwrap()
            .next(&mut position, limits, counters, rule.location)
            .unwrap()
        {
            selected.push(row);
        }
        selected
    };
    {
        let borrowed = CursorRows::prepare(rule, cursor, computation, limits, counters)
            .unwrap()
            .unwrap();
        let descriptor = borrowed.descriptor();
        assert_eq!(std::ptr::from_ref(descriptor.plan), plan_pointer);
        assert_eq!(descriptor.plan.patterns.as_ptr(), patterns_pointer);
        assert_eq!(descriptor.rows, expected);
        assert_eq!(descriptor.plan.pivot, pivot);
        assert_eq!(budget.usage(), usage, "no second join planner was invoked");
    }
    assert_eq!(cursor.lease.bytes(), lease);
    assert_eq!(cursor.positions, position);
    assert_eq!(cursor.depth, 0);
    assert!(matches!(cursor.traversal, Traversal::Searching));
    assert_eq!(
        std::ptr::from_ref(cursor.probes[0].as_ref().unwrap()),
        probe_pointer
    );
}

/// Mirror the real possible-support hook, including delta-only preparation.
/// This flat grammar has no separate static totality certificate.
fn select_source<'source>(
    rule: &'source RuleIr,
    cursor: &mut Join<'_, 'source>,
    computation: &mut Computation<'_, 'source>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) {
    assert!(
        !cursor
            .select_round_multiples(rule, computation, limits, counters)
            .unwrap()
    );
    cursor
        .select_scalar_body(rule, computation, limits, counters)
        .unwrap();
    cursor
        .select_numeric_probes(rule, limits, counters)
        .unwrap();
}

fn full_rows(variant: delta::Variant, add_delta: bool) -> Vec<Vec<Value>> {
    let (program, mut catalog) = prepared_source(JOIN);
    let rule = program.rules_without_facts().last().unwrap();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut budget = testing::budget();
    if add_delta {
        {
            let (_, mut append) = catalog
                .split(&limits, &mut counters, rule.location)
                .unwrap();
            append
                .atom(
                    (&atom("edge", &[4, 2])).into(),
                    &limits,
                    &mut counters,
                    rule.location,
                )
                .unwrap();
            append.order(&limits, &mut counters, rule.location).unwrap();
        }
        catalog
            .publish(&limits, &mut counters, rule.location)
            .unwrap();
    }
    let (relations, mut append) = catalog
        .split(&limits, &mut counters, rule.location)
        .unwrap();
    let support = Support::indexed(&relations, &limits, &counters, rule.location).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    let mut cursor = Join::variant_rule(
        Ir::Source(rule),
        variant,
        &support,
        None,
        &mut budget,
        Context::new(&computation, &limits, &mut counters, rule.location),
    )
    .unwrap();
    let mut reference = Join::variant_rule(
        Ir::Source(rule),
        variant,
        &support,
        None,
        &mut budget,
        Context::new(&computation, &limits, &mut counters, rule.location),
    )
    .unwrap();
    for selected in [&mut cursor, &mut reference] {
        select_source(rule, selected, &mut computation, &limits, &mut counters);
    }
    assert_lent(
        rule,
        &mut cursor,
        &mut computation,
        &limits,
        &budget,
        &mut counters,
    );
    let actual = drain(
        &mut cursor,
        &mut computation,
        &limits,
        &mut budget,
        &mut counters,
        rule.location,
    );
    let expected = drain(
        &mut reference,
        &mut computation,
        &limits,
        &mut budget,
        &mut counters,
        rule.location,
    );
    assert_eq!(actual, expected);
    actual
}

#[test]
fn selected_cursor_lends_its_existing_plan_and_remains_fully_usable() {
    assert_eq!(full_rows(delta::Variant::Full, false).len(), 4);
}

#[test]
fn first_new_pivots_and_their_exact_selected_order_survive_lending() {
    let first = full_rows(delta::Variant::Delta(0), true);
    let second = full_rows(delta::Variant::Delta(1), true);
    assert_eq!(first.len() + second.len(), 3);
    assert!(first.iter().all(|row| !second.contains(row)));
}

#[test]
fn a_started_cursor_declines_without_consuming_its_pending_undo_or_remaining_rows() {
    let (program, mut catalog) = prepared_source(JOIN);
    let rule = program.rules_without_facts().last().unwrap();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut budget = testing::budget();
    let (relations, mut append) = catalog
        .split(&limits, &mut counters, rule.location)
        .unwrap();
    let support = Support::indexed(&relations, &limits, &counters, rule.location).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    let mut cursor = Join::variant_rule(
        Ir::Source(rule),
        delta::Variant::Full,
        &support,
        None,
        &mut budget,
        Context::new(&computation, &limits, &mut counters, rule.location),
    )
    .unwrap();
    let mut actual = {
        let first = cursor
            .next_support(
                None,
                &mut computation,
                &limits,
                &mut budget,
                &mut counters,
                rule.location,
            )
            .unwrap()
            .unwrap();
        vec![values(&first, &computation, rule.location)]
    };
    let positions = cursor.positions.clone();
    let depth = cursor.depth;
    let lease = cursor.lease.bytes();
    assert!(
        CursorRows::prepare(rule, &mut cursor, &mut computation, &limits, &mut counters)
            .unwrap()
            .is_none()
    );
    assert_eq!(cursor.positions, positions);
    assert_eq!(cursor.depth, depth);
    assert_eq!(cursor.lease.bytes(), lease);
    actual.extend(drain(
        &mut cursor,
        &mut computation,
        &limits,
        &mut budget,
        &mut counters,
        rule.location,
    ));
    let mut reference = Join::variant_rule(
        Ir::Source(rule),
        delta::Variant::Full,
        &support,
        None,
        &mut budget,
        Context::new(&computation, &limits, &mut counters, rule.location),
    )
    .unwrap();
    assert_eq!(
        actual,
        drain(
            &mut reference,
            &mut computation,
            &limits,
            &mut budget,
            &mut counters,
            rule.location
        )
    );
    assert_eq!(actual.len(), 4);
}

#[test]
fn unsupported_evidence_and_foreign_query_owners_do_not_become_plain_source_cursors() {
    let (program, mut catalog) = prepared_source(JOIN);
    let rule = program.rules_without_facts().last().unwrap();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut budget = testing::budget();
    let (relations, mut append) = catalog
        .split(&limits, &mut counters, rule.location)
        .unwrap();
    let support = Support::indexed(&relations, &limits, &counters, rule.location).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    let mut cursor = Join::variant_rule(
        Ir::Source(rule),
        delta::Variant::Full,
        &support,
        None,
        &mut budget,
        Context::new(&computation, &limits, &mut counters, rule.location),
    )
    .unwrap();
    cursor.source_evidence = true;
    assert!(
        CursorRows::prepare(rule, &mut cursor, &mut computation, &limits, &mut counters)
            .unwrap()
            .is_none()
    );
    assert!(cursor.source_evidence);
    assert!(cursor.probes.iter().all(Option::is_none));
    cursor.source_evidence = false;
    let other = Support::indexed(&relations, &limits, &counters, rule.location).unwrap();
    let mut foreign = Computation::new(&mut append, &other);
    assert!(matches!(
        CursorRows::prepare(rule, &mut cursor, &mut foreign, &limits, &mut counters),
        Err(FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Owner,
            ..
        })
    ));
    assert!(cursor.probes.iter().all(Option::is_none));
}

#[test]
fn work_and_byte_refusals_leave_the_original_complete_cursor_available() {
    let (program, mut catalog) = prepared_source(JOIN);
    let rule = program.rules_without_facts().last().unwrap();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut budget = testing::budget();
    let (relations, mut append) = catalog
        .split(&limits, &mut counters, rule.location)
        .unwrap();
    let support = Support::indexed(&relations, &limits, &counters, rule.location).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    let mut cursor = Join::variant_rule(
        Ir::Source(rule),
        delta::Variant::Full,
        &support,
        None,
        &mut budget,
        Context::new(&computation, &limits, &mut counters, rule.location),
    )
    .unwrap();
    select_source(rule, &mut cursor, &mut computation, &limits, &mut counters);
    let workspace = counters.workspace_bytes();
    let work = counters.accounting.work;
    let bounded = FormulaLimits {
        max_work: work,
        ..limits
    };
    assert!(matches!(
        CursorRows::prepare(rule, &mut cursor, &mut computation, &bounded, &mut counters),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            ..
        })
    ));
    assert_eq!(counters.accounting.work, work);
    assert_eq!(counters.workspace_bytes(), workspace);
    assert!(cursor.probes.iter().all(Option::is_none));
    let bounded = FormulaLimits {
        max_support_bytes: support.live_bytes(),
        ..limits
    };
    assert!(matches!(
        CursorRows::prepare(rule, &mut cursor, &mut computation, &bounded, &mut counters),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            ..
        })
    ));
    assert!(
        counters.accounting.work > work,
        "actual admission reads survive refusal"
    );
    assert_eq!(
        counters.workspace_bytes(),
        workspace,
        "failed descriptor released its lease"
    );
    assert!(cursor.probes.iter().all(Option::is_none));
    assert!(cursor.positions.iter().all(|position| *position == 0));
    assert_eq!(
        drain(
            &mut cursor,
            &mut computation,
            &limits,
            &mut budget,
            &mut counters,
            rule.location
        )
        .len(),
        4
    );
}
