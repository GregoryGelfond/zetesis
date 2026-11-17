use super::*;
use crate::formula_binding::Binding;
use crate::formula_support::row_blocks::witness_rows::{Replay, RowBinding};
use crate::formula_support::{Ir, testing};
use crate::{FormulaResource, ProgramSite};
use zetesis_algebra::{Cancellation, Stop};
use zetesis_core::{Value, ValueLimits};

const SELECTED: &str = "a(1;2;3).b(1;2).out(X):-a(X),b(Y),X<=2,Y<=X.";

fn values(
    binding: &Binding<'_>,
    computation: &Computation<'_, '_>,
    location: ProgramSite,
) -> Vec<Value> {
    (0..binding.len())
        .map(|slot| {
            computation
                .read()
                .term(&binding.key(slot, location).unwrap())
                .unwrap()
                .to_value(ValueLimits::default())
                .unwrap()
        })
        .collect()
}

fn capture<'query, 'source: 'query>(
    rule: &'source RuleIr,
    support: &'query Support<'source>,
    computation: &mut Computation<'_, 'source>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<Family<'query, 'source>, FormulaFailure> {
    let mut budget = testing::budget();
    let mut cursor = Join::rule(
        Ir::Source(rule),
        support,
        computation,
        limits,
        &mut budget,
        counters,
    )?;
    let mut recording = Recording::new(
        rule,
        &cursor,
        Context::new(&*computation, limits, counters, rule.location),
    )?
    .unwrap();
    cursor.retain_witnesses(Context::new(&*computation, limits, counters, rule.location))?;
    while let Some(row) = cursor.next_matched_row(
        &mut budget,
        Context::new(computation, limits, counters, rule.location),
    )? {
        if row.passes {
            recording.stage(
                row.witnesses.as_ref().unwrap(),
                Context::new(&*computation, limits, counters, rule.location),
            )?;
        }
    }
    recording.finish(
        cursor,
        Context::new(&*computation, limits, counters, rule.location),
    )
}

fn decoded<'source>(
    family: &Family<'_, 'source>,
    support: &Support<'source>,
    computation: &Computation<'_, 'source>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Vec<Vec<Value>> {
    let location = family.rule.location;
    let plan = family
        .view(
            family.rule,
            support,
            Context::new(computation, limits, counters, location),
        )
        .unwrap();
    let mut replay = Replay::new(
        &plan,
        support,
        Context::new(computation, limits, counters, location),
    )
    .unwrap();
    let mut rows = replay.retained(family, limits, counters).unwrap();
    let mut binding = None;
    let mut result = Vec::new();
    while let Some(row) = rows
        .next(Context::new(computation, limits, counters, location))
        .unwrap()
    {
        if binding.is_none() {
            binding = Some(
                RowBinding::new(
                    &plan,
                    &row,
                    Context::new(computation, limits, counters, location),
                )
                .unwrap(),
            );
        }
        let binding = binding
            .as_mut()
            .unwrap()
            .read(&row, Context::new(computation, limits, counters, location))
            .unwrap();
        result.push(values(binding, computation, location));
    }
    result
}

#[test]
fn retained_comparison_stream_keeps_source_order_and_duplicate_head_witnesses() {
    testing::with_completed_source(SELECTED, |program, support, computation, counters| {
        let rule = program.rules_without_facts().last().unwrap();
        let limits = FormulaLimits::default();
        let mut budget = testing::budget();
        let mut cursor = Join::rule(
            Ir::Source(rule),
            support,
            computation,
            &limits,
            &mut budget,
            counters,
        )
        .unwrap();
        let mut expected = Vec::new();
        while let Some(binding) = cursor
            .next(computation, &limits, &mut budget, counters, rule.location)
            .unwrap()
        {
            expected.push(values(&binding, computation, rule.location));
        }
        assert_eq!(expected.len(), 3);
        // X=2 has two distinct selected witnesses; retaining only the head
        // projection would erase a real consumer occurrence.
        assert_eq!(
            expected
                .iter()
                .filter(|row| row[0] == Value::Number(2))
                .count(),
            2
        );
        let family = capture(rule, support, computation, &limits, counters).unwrap();
        assert_eq!(family.completions.len(), expected.len());
        assert!(family.completions.iter().any(|row| row.unchanged > 0));
        assert_eq!(
            decoded(&family, support, computation, &limits, counters),
            expected
        );
        assert_eq!(
            decoded(&family, support, computation, &limits, counters),
            expected
        );
    });
}

#[test]
fn an_exhausted_empty_family_is_a_real_reusable_result() {
    testing::with_completed_source(
        "a(1;2).out(X):-a(X),X<0.",
        |program, support, computation, counters| {
            let rule = program.rules_without_facts().last().unwrap();
            let limits = FormulaLimits::default();
            let family = capture(rule, support, computation, &limits, counters).unwrap();
            assert_eq!(family.completions.len(), 0);
            assert_eq!(family.suffixes.len(), 0);
            assert!(decoded(&family, support, computation, &limits, counters).is_empty());
        },
    );
}

#[test]
fn unfinished_and_foreign_rule_owners_never_publish_a_complete_family() {
    testing::with_completed_source(
        "a(1;2).out(X):-a(X),X<=2.other(X):-a(X),X<=2.",
        |program, support, computation, counters| {
            let rules = program.rules_without_facts();
            let rule = &rules[0];
            let limits = FormulaLimits::default();
            let mut budget = testing::budget();
            let cursor = Join::rule(
                Ir::Source(rule),
                support,
                computation,
                &limits,
                &mut budget,
                counters,
            )
            .unwrap();
            let recording = Recording::new(
                rule,
                &cursor,
                Context::new(&*computation, &limits, counters, rule.location),
            )
            .unwrap()
            .unwrap();
            assert!(matches!(
                recording.finish(
                    cursor,
                    Context::new(&*computation, &limits, counters, rule.location)
                ),
                Err(FormulaFailure::SupportRelation {
                    error: zetesis_core::relation::Failure::Owner,
                    ..
                })
            ));
            let family = capture(rule, support, computation, &limits, counters).unwrap();
            assert!(matches!(
                family.view(
                    &rules[1],
                    support,
                    Context::new(&*computation, &limits, counters, rule.location)
                ),
                Err(FormulaFailure::SupportRelation {
                    error: zetesis_core::relation::Failure::Owner,
                    ..
                })
            ));
        },
    );
}

#[test]
fn admission_refusal_and_cancellation_keep_the_performed_prefix_without_a_family() {
    testing::with_completed_source(SELECTED, |program, support, computation, counters| {
        let rule = program.rules_without_facts().last().unwrap();
        let limits = FormulaLimits::default();
        let mut budget = testing::budget();
        let cursor = Join::rule(
            Ir::Source(rule),
            support,
            computation,
            &limits,
            &mut budget,
            counters,
        )
        .unwrap();
        let before = counters.accounting.work;
        let stopped = FormulaLimits {
            max_work: before,
            ..limits
        };
        assert!(matches!(Recording::new(rule, &cursor,
            Context::new(&*computation, &stopped, counters, rule.location)),
            Err(FormulaFailure::Limit { resource: FormulaResource::Work, limit, observed, .. })
                if limit == u128::from(before) && observed == u128::from(before) + 2));
        assert_eq!(counters.accounting.work, before);
        let stopped = FormulaLimits {
            max_support_bytes: 0,
            ..limits
        };
        assert!(matches!(
            Recording::new(
                rule,
                &cursor,
                Context::new(&*computation, &stopped, counters, rule.location)
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                limit: 0,
                ..
            })
        ));
        let cancellation = Cancellation::default();
        cancellation.cancel();
        let mut controlled = Counters::default().with_cancellation(Some(&cancellation));
        assert!(matches!(
            Recording::new(
                rule,
                &cursor,
                Context::new(&*computation, &limits, &mut controlled, rule.location)
            ),
            Err(FormulaFailure::Interrupted {
                reason: Stop::Cancelled,
                ..
            })
        ));
    });
}

#[test]
fn only_a_pre_reserved_cell_removes_the_inline_family_charge() {
    testing::with_completed_source(SELECTED, |program, support, computation, counters| {
        let rule = program.rules_without_facts().last().unwrap();
        let limits = FormulaLimits::default();
        let mut slot =
            Buffer::<Option<Family<'_, '_>>>::new(&*computation, &limits, counters, rule.location)
                .unwrap();
        slot.push(None, &*computation, &limits, counters, rule.location)
            .unwrap();
        let family = capture(rule, support, computation, &limits, counters).unwrap();
        let plan_bytes = usize::try_from(family.plan.retained_bytes()).unwrap();
        assert_eq!(family.lease.bytes(), plan_bytes + Family::header());
        slot.slice_mut()[0] = Some(family.embedded(rule.location).unwrap());
        let retained = slot.slice()[0].as_ref().unwrap();
        assert_eq!(retained.lease.bytes(), plan_bytes);
        assert_eq!(
            decoded(retained, support, computation, &limits, counters).len(),
            3
        );
    });
}

#[test]
fn cancellation_after_a_real_selected_prefix_keeps_receipts_but_cannot_seal_it() {
    testing::with_completed_source(SELECTED, |program, support, computation, counters| {
        let rule = program.rules_without_facts().last().unwrap();
        let limits = FormulaLimits::default();
        let mut budget = testing::budget();
        let mut cursor = Join::rule(
            Ir::Source(rule),
            support,
            computation,
            &limits,
            &mut budget,
            counters,
        )
        .unwrap();
        let mut recording = Recording::new(
            rule,
            &cursor,
            Context::new(&*computation, &limits, counters, rule.location),
        )
        .unwrap()
        .unwrap();
        cursor
            .retain_witnesses(Context::new(
                &*computation,
                &limits,
                counters,
                rule.location,
            ))
            .unwrap();
        let before = counters.accounting.work;
        {
            let row = cursor
                .next_matched_row(
                    &mut budget,
                    Context::new(computation, &limits, counters, rule.location),
                )
                .unwrap()
                .unwrap();
            assert!(row.passes);
            recording
                .stage(
                    row.witnesses.as_ref().unwrap(),
                    Context::new(&*computation, &limits, counters, rule.location),
                )
                .unwrap();
        }
        assert_eq!(recording.completions.len(), 1);
        let performed = counters.accounting.work;
        assert!(performed > before);
        let cancellation = Cancellation::default();
        counters.attach_test_control(&cancellation);
        cancellation.cancel();
        assert!(matches!(
            cursor.next_matched_row(
                &mut budget,
                Context::new(computation, &limits, counters, rule.location)
            ),
            Err(FormulaFailure::Interrupted {
                reason: Stop::Cancelled,
                ..
            })
        ));
        assert_eq!(counters.accounting.work, performed);
        assert!(matches!(
            recording.finish(
                cursor,
                Context::new(&*computation, &limits, counters, rule.location)
            ),
            Err(FormulaFailure::Interrupted {
                reason: Stop::Cancelled,
                ..
            })
        ));
        counters.detach_test_control();
    });
}

#[test]
fn richer_ordinary_producers_decline_without_consuming_the_cursor() {
    for source in [
        "a(1;2).{b(1)}.out(X):-a(X),not b(X).",
        "a(1;2).out(f(X)):-a(X).",
        "a(1;2).out(X+1):-a(X).",
    ] {
        testing::with_completed_source(source, |program, support, computation, counters| {
            let rule = program.rules_without_facts().last().unwrap();
            let limits = FormulaLimits::default();
            let mut budget = testing::budget();
            let mut cursor = Join::rule(
                Ir::Source(rule),
                support,
                computation,
                &limits,
                &mut budget,
                counters,
            )
            .unwrap();
            assert!(
                Recording::new(
                    rule,
                    &cursor,
                    Context::new(&*computation, &limits, counters, rule.location)
                )
                .unwrap()
                .is_none()
            );
            assert!(matches!(cursor.traversal, Traversal::Searching));
            assert_eq!(cursor.depth, 0);
            assert!(
                cursor
                    .next(computation, &limits, &mut budget, counters, rule.location)
                    .unwrap()
                    .is_some(),
                "the ordinary continuation remains usable"
            );
        });
    }
}
