use super::*;
use crate::formula_support::row_blocks::tests::{
    JOIN, atom, prepared_source, settle_expansion, worker_expansion,
};
use crate::formula_support::row_blocks::{RowPlan, cursor_rows::CursorRows};
use crate::formula_support::{Accounting, testing};
use crate::grounding_observer::Work;
use crate::{ExpansionLimits, ExpansionUsage, FormulaResource};
use std::sync::Barrier;
use zetesis_algebra::Cancellation;
use zetesis_core::{Atom, ValueLimits};

struct Outcome<'plan, 'source> {
    result: Result<Batch<'plan, 'source>, FormulaFailure>,
    work: u64,
    substitutions: u64,
    expansion: ExpansionUsage,
    worker: std::thread::ThreadId,
}

fn workers<'plan, 'source, 'read>(
    plan: &'plan WitnessPlan<'plan, 'source>,
    append: &'read super::super::super::SupportAppend<'source>,
    count: usize,
    limits: &FormulaLimits,
    expansion: ExpansionLimits,
    pending_read: bool,
) -> Vec<Outcome<'plan, 'source>>
where
    'source: 'plan + 'read,
{
    let barrier = Barrier::new(count);
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for block in 0..count {
            let barrier = &barrier;
            handles.push(scope.spawn(move || {
                let mut accounting = Accounting::default();
                let mut budget = Budget::new(expansion, usize::MAX);
                barrier.wait();
                let result =
                    accounting.controlled(None, Work::default(), plan.location(), |local| {
                        let block = plan.len() * block / count..plan.len() * (block + 1) / count;
                        if pending_read {
                            execute_borrowed(plan, block, append, limits, &mut budget, local)
                        } else {
                            execute(
                                plan,
                                block,
                                append.term_lookup(),
                                limits,
                                &mut budget,
                                local,
                            )
                        }
                    });
                Outcome {
                    result,
                    work: accounting.work,
                    substitutions: accounting.substitutions,
                    expansion: budget.usage(),
                    worker: std::thread::current().id(),
                }
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    })
}

fn settle(
    outcomes: &[Outcome<'_, '_>],
    counters: &mut Counters,
    budget: &mut Budget,
    limits: &FormulaLimits,
    location: ProgramSite,
) {
    let work: u128 = outcomes
        .iter()
        .map(|outcome| u128::from(outcome.work))
        .sum();
    counters.absorb(work, limits, location).unwrap();
    let substitutions: u128 = outcomes
        .iter()
        .map(|outcome| u128::from(outcome.substitutions))
        .sum();
    counters.accounting.substitutions = zetesis_algebra::Meter::check_receipt(
        counters.accounting.substitutions,
        substitutions,
        limits.max_substitutions,
    )
    .unwrap();
    for outcome in outcomes {
        settle_expansion(budget, outcome.expansion, location);
    }
}

fn retained_shape(batches: &[Batch<'_, '_>]) -> (usize, usize, usize) {
    let completions = batches.iter().map(Batch::len).sum();
    let suffix_rows = batches.iter().map(|batch| batch.suffixes.len()).sum();
    let positive_prefixes = batches
        .iter()
        .flat_map(|batch| batch.completions.iter())
        .filter(|completion| completion.unchanged > 0)
        .count();
    (completions, suffix_rows, positive_prefixes)
}

fn pending_heads(append: &crate::formula_support::SupportAppend<'_>) -> Vec<Atom> {
    append
        .atoms()
        .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
        .collect()
}

struct Report {
    heads: Vec<Atom>,
    completions: usize,
    suffix_rows: usize,
    full_rows: usize,
    published: usize,
    workers: Vec<std::thread::ThreadId>,
    positive_prefixes: usize,
}

/// Private transport observations are not charged as production execution.
/// Actual worker/replay buffers use the same disjoint-reservation rule as the
/// qualified binding prototype; every worker receipt settles before mutation.
fn run(source: &str, count: usize, pending: Option<Atom>) -> Report {
    run_with_reader(source, count, pending, false)
}

fn run_with_reader(
    source: &str,
    count: usize,
    pending: Option<Atom>,
    pending_read: bool,
) -> Report {
    let (program, mut catalog) = prepared_source(source);
    let rule = program.rules_without_facts().last().unwrap();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut budget = testing::budget();
    let (relations, mut append) = catalog
        .split(&limits, &mut counters, rule.location)
        .unwrap();
    if let Some(atom) = pending {
        append
            .atom((&atom).into(), &limits, &mut counters, rule.location)
            .unwrap();
    }
    let support = Support::indexed(&relations, &limits, &counters, rule.location).unwrap();
    let mut cursor = {
        let computation = Computation::new(&mut append, &support);
        Join::variant_rule(
            Ir::Source(rule),
            delta::Variant::Full,
            &support,
            None,
            &mut budget,
            Context::new(&computation, &limits, &mut counters, rule.location),
        )
        .unwrap()
    };
    let base = {
        let mut computation = Computation::new(&mut append, &support);
        CursorRows::prepare(rule, &mut cursor, &mut computation, &limits, &mut counters)
            .unwrap()
            .unwrap()
    };
    let plan = {
        let computation = Computation::new(&mut append, &support);
        WitnessPlan::new(
            base.descriptor(),
            &relations,
            &support,
            &computation,
            &limits,
            &mut counters,
        )
        .unwrap()
    };
    let scratch = (limits.max_support_bytes - support.live_bytes()) / 2 / count;
    let grant = FormulaLimits {
        max_work: (limits.max_work - counters.accounting.work) / count as u64,
        max_substitutions: (limits.max_substitutions - counters.accounting.substitutions)
            / count as u64,
        max_support_bytes: relations.current_bytes() + scratch,
        ..limits
    };
    let publication = FormulaLimits {
        max_support_bytes: limits.max_support_bytes - count * scratch,
        ..limits
    };
    let outcomes = workers(
        &plan,
        &append,
        count,
        &grant,
        worker_expansion(count, budget.usage()),
        pending_read,
    );
    settle(
        &outcomes,
        &mut counters,
        &mut budget,
        &limits,
        rule.location,
    );
    let ids = outcomes.iter().map(|outcome| outcome.worker).collect();
    let batches = outcomes
        .into_iter()
        .map(|outcome| outcome.result.unwrap())
        .collect::<Vec<_>>();
    let (completions, suffix_rows, positive_prefixes) = retained_shape(&batches);
    let published = {
        let mut computation = Computation::new(&mut append, &support);
        publish(
            &plan,
            &batches,
            &support,
            &mut computation,
            &publication,
            &mut counters,
            0,
        )
        .unwrap()
    };
    let heads = pending_heads(&append);
    Report {
        heads,
        completions,
        suffix_rows,
        full_rows: completions * base.descriptor().plan.patterns.len(),
        published,
        workers: ids,
        positive_prefixes,
    }
}

fn original_heads(source: &str, pending: Option<Atom>) -> (Vec<Atom>, u64) {
    let (program, mut catalog) = prepared_source(source);
    let rule = program.rules_without_facts().last().unwrap();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut budget = testing::budget();
    let (relations, mut append) = catalog
        .split(&limits, &mut counters, rule.location)
        .unwrap();
    if let Some(atom) = pending {
        append
            .atom((&atom).into(), &limits, &mut counters, rule.location)
            .unwrap();
    }
    let support = Support::indexed(&relations, &limits, &counters, rule.location).unwrap();
    {
        let mut computation = Computation::new(&mut append, &support);
        let mut join = Join::variant_rule(
            Ir::Source(rule),
            delta::Variant::Full,
            &support,
            None,
            &mut budget,
            Context::new(&computation, &limits, &mut counters, rule.location),
        )
        .unwrap();
        super::super::super::derive_rule(
            (0, rule),
            &mut join,
            &support,
            &mut computation,
            &limits,
            &mut budget,
            &mut counters,
        )
        .unwrap();
    }
    (pending_heads(&append), counters.accounting.substitutions)
}

#[test]
fn threaded_witness_transport_reuses_the_ordinary_row_head_and_complete_order() {
    let reference = original_heads(JOIN, None).0;
    for count in [1, 2, 3, 7] {
        let report = run(JOIN, count, None);
        assert_eq!(report.heads, reference, "workers={count}");
        assert_eq!(report.completions, 4);
        assert_eq!(report.published, report.completions);
        if count > 1 {
            assert_ne!(report.workers[0], report.workers[1]);
        }
    }
}

#[test]
fn unchanged_witness_prefixes_reduce_transport_without_removing_completions() {
    let source = "p(1). p(2). q(3). q(4). q(5). out(X,Y):-p(X),q(Y).";
    let report = run(source, 2, None);
    assert_eq!(report.heads, original_heads(source, None).0);
    assert_eq!(report.completions, 6);
    assert_eq!(report.published, 6);
    assert_eq!(report.full_rows, 12);
    assert_eq!(report.suffix_rows, 8);
    assert_eq!(report.positive_prefixes, 4);
}

#[test]
fn pending_heads_remain_exact_but_the_frozen_worker_does_not_claim_their_skip() {
    let pending = || Some(atom("out", &[1, 4]));
    let (expected, original_completions) = original_heads(JOIN, pending());
    let report = run(JOIN, 2, pending());
    assert_eq!(report.heads, expected);
    assert_eq!(
        report.heads,
        [
            atom("out", &[1, 4]),
            atom("out", &[2, 5]),
            atom("out", &[3, 5])
        ]
    );
    assert_eq!(
        original_completions, 2,
        "Append sees the existing pending head"
    );
    assert_eq!(
        report.completions, 4,
        "Frozen sees only published source relations"
    );
    assert_eq!(
        report.published, 4,
        "every staged completion reaches existing deduplication"
    );
}

#[test]
fn witness_transport_preserves_empty_results_and_nullary_heads() {
    let empty = run("p(1). q(2). r(1). out(X,Y):-p(X),q(X),r(Y).", 2, None);
    assert_eq!(empty.completions, 0);
    assert_eq!(empty.published, 0);
    assert!(empty.heads.is_empty());
    let ground = run("p(1). q(2). out:-p(X),q(Y).", 2, None);
    assert_eq!(ground.completions, 1);
    assert_eq!(ground.published, 1);
    assert_eq!(ground.heads, [atom("out", &[])]);
}

fn assert_serial_descriptor(source: &str) {
    let (program, mut catalog) = prepared_source(source);
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
    let positions = cursor.positions.clone();
    assert!(
        CursorRows::prepare(rule, &mut cursor, &mut computation, &limits, &mut counters,)
            .unwrap()
            .is_none()
    );
    assert_eq!(cursor.positions, positions);
    assert!(cursor.probes.iter().all(Option::is_none));
}

#[test]
fn single_generator_empty_and_nullary_rules_keep_serial_witnesses() {
    let empty = "p(1). q(2). out(X):-p(X),q(X).";
    assert_serial_descriptor(empty);
    let (heads, completions) = original_heads(empty, None);
    assert_eq!(completions, 0);
    assert!(heads.is_empty());
    let ground = "p. q. out:-p,q.";
    assert_serial_descriptor(ground);
    let (heads, completions) = original_heads(ground, None);
    assert_eq!(completions, 1);
    assert_eq!(heads, [atom("out", &[])]);
}

fn assert_invalid_block_precedes_admission<'plan, 'source: 'plan>(
    plan: &'plan WitnessPlan<'plan, 'source>,
    append: &crate::formula_support::SupportAppend<'source>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) {
    let no_allowance = FormulaLimits {
        max_work: 0,
        max_support_bytes: 0,
        ..*limits
    };
    let before = counters.accounting.work;
    for result in [
        execute(
            plan,
            plan.len()..plan.len() + 1,
            append.term_lookup(),
            &no_allowance,
            &mut testing::budget(),
            counters,
        ),
        execute_borrowed(
            plan,
            plan.len()..plan.len() + 1,
            append,
            &no_allowance,
            &mut testing::budget(),
            counters,
        ),
    ] {
        assert!(matches!(
            result,
            Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                ..
            })
        ));
    }
    assert_eq!(
        counters.accounting.work, before,
        "range ownership precedes worker admission"
    );
}

#[test]
fn staged_occurrences_are_authenticated_before_the_first_head_is_published() {
    let (program, mut catalog) = prepared_source(JOIN);
    let rule = program.rules_without_facts().last().unwrap();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let (relations, mut append) = catalog
        .split(&limits, &mut counters, rule.location)
        .unwrap();
    let support = Support::indexed(&relations, &limits, &counters, rule.location).unwrap();
    let base = {
        let mut computation = Computation::new(&mut append, &support);
        RowPlan::prepare(
            rule,
            delta::Variant::Full,
            &support,
            &mut computation,
            &limits,
            &mut testing::budget(),
            &mut counters,
        )
        .unwrap()
        .unwrap()
    };
    let plan = {
        let computation = Computation::new(&mut append, &support);
        WitnessPlan::new(
            base.descriptor(),
            &relations,
            &support,
            &computation,
            &limits,
            &mut counters,
        )
        .unwrap()
    };
    let mut account = Accounting::default();
    assert_invalid_block_precedes_admission(&plan, &append, &limits, &mut counters);
    let mut batch = account
        .controlled(None, Work::default(), rule.location, |local| {
            execute(
                &plan,
                0..plan.len(),
                append.term_lookup(),
                &limits,
                &mut testing::budget(),
                local,
            )
        })
        .unwrap();
    batch.suffixes.poison_source_index(0);
    counters
        .absorb(u128::from(account.work), &limits, rule.location)
        .unwrap();
    let result = {
        let mut computation = Computation::new(&mut append, &support);
        publish(
            &plan,
            &[batch],
            &support,
            &mut computation,
            &limits,
            &mut counters,
            0,
        )
    };
    assert!(matches!(
        result,
        Err(FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Owner,
            ..
        })
    ));
    assert!(append.is_empty());
}

#[test]
fn refused_or_cancelled_staging_keeps_actual_work_and_never_returns_a_partial_batch() {
    let (program, mut catalog) = prepared_source(JOIN);
    let rule = program.rules_without_facts().last().unwrap();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let (relations, mut append) = catalog
        .split(&limits, &mut counters, rule.location)
        .unwrap();
    let support = Support::indexed(&relations, &limits, &counters, rule.location).unwrap();
    let base = {
        let mut computation = Computation::new(&mut append, &support);
        RowPlan::prepare(
            rule,
            delta::Variant::Full,
            &support,
            &mut computation,
            &limits,
            &mut testing::budget(),
            &mut counters,
        )
        .unwrap()
        .unwrap()
    };
    let plan = {
        let computation = Computation::new(&mut append, &support);
        WitnessPlan::new(
            base.descriptor(),
            &relations,
            &support,
            &computation,
            &limits,
            &mut counters,
        )
        .unwrap()
    };
    let mut complete = Accounting::default();
    let batch = complete
        .controlled(None, Work::default(), rule.location, |local| {
            execute(
                &plan,
                0..plan.len(),
                append.term_lookup(),
                &limits,
                &mut testing::budget(),
                local,
            )
        })
        .unwrap();
    assert_eq!(batch.len(), 4);
    drop(batch);
    for limit in [0, 1, complete.work / 2, complete.work - 1] {
        let bounded = FormulaLimits {
            max_work: limit,
            ..limits
        };
        let mut account = Accounting::default();
        let result = account.controlled(None, Work::default(), rule.location, |local| {
            execute(
                &plan,
                0..plan.len(),
                append.term_lookup(),
                &bounded,
                &mut testing::budget(),
                local,
            )
        });
        assert!(matches!(
            result,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ));
        assert!(account.work <= limit);
        assert!(limit == 0 || account.work > 0);
        assert_eq!(
            account.workspace.bytes(),
            0,
            "failed staging released its output leases"
        );
    }
    assert_cancelled_staging(&plan, &append, &limits);
}

fn assert_cancelled_staging(
    plan: &WitnessPlan<'_, '_>,
    append: &super::super::super::SupportAppend<'_>,
    limits: &FormulaLimits,
) {
    let cancellation = Cancellation::default();
    let mut account = Accounting {
        work: 7,
        ..Accounting::default()
    };
    let result = account.controlled(
        Some(&cancellation),
        Work::default(),
        plan.location(),
        |local| {
            cancellation.cancel();
            execute(
                plan,
                0..plan.len(),
                append.term_lookup(),
                limits,
                &mut testing::budget(),
                local,
            )
        },
    );
    assert!(matches!(result, Err(FormulaFailure::Interrupted { .. })));
    assert!(account.work > 7 && account.work <= zetesis_algebra::POLL_INTERVAL);
    assert_eq!(account.workspace.bytes(), 0);
    assert!(append.is_empty());
}

#[test]
fn seeded_prefixes_preserve_first_row_order_and_heads() {
    // Three generators and a decided default negation so the rule is Prime,
    // expands after the first pattern, and has a short first probe no matter
    // whether the unary or the 4-row binary leads. The large third atom
    // supplies at least 56 tuples at d = 2 or d = 3.
    let mut source = String::from("r(1). r(2). e(1,10). e(1,11). e(2,10). e(2,11).\n");
    for y in 10..=11 {
        for z in 1..=30 {
            source.push_str(&format!("f({y},{z}). "));
        }
        source.push('\n');
    }
    source.push_str("p(X,Y,Z):-r(X),e(X,Y),f(Y,Z),not q(X).");
    assert_prefix_depth_matches_serial(&source);
}

fn assert_prefix_depth_matches_serial(source: &str) {
    let (expected, _) = original_heads(source, None);
    let (program, mut catalog) = prepared_source(source);
    let rule = program.rules_without_facts().last().unwrap();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut budget = testing::budget();
    let (relations, mut append) = catalog
        .split(&limits, &mut counters, rule.location)
        .unwrap();
    let support = Support::indexed(&relations, &limits, &counters, rule.location).unwrap();
    let mut cursor = {
        let computation = Computation::new(&mut append, &support);
        Join::variant_rule(
            Ir::Source(rule),
            delta::Variant::Full,
            &support,
            None,
            &mut budget,
            Context::new(&computation, &limits, &mut counters, rule.location),
        )
        .unwrap()
    };
    let base = {
        let mut computation = Computation::new(&mut append, &support);
        CursorRows::prepare(rule, &mut cursor, &mut computation, &limits, &mut counters)
            .unwrap()
            .expect("Prime comparison/negation body should be admitted")
    };
    assert!(
        base.descriptor().prime,
        "default negation must take the A′ path"
    );
    assert!(
        base.descriptor().len() < super::super::prime::PRIME_GROUPS,
        "the first probe must be short enough to deepen"
    );
    let plan = {
        let computation = Computation::new(&mut append, &support);
        WitnessPlan::new(
            base.descriptor(),
            &relations,
            &support,
            &computation,
            &limits,
            &mut counters,
        )
        .unwrap()
    };
    let prefixes = {
        let mut computation = Computation::new(&mut append, &support);
        super::super::prime::prefixes::adapt(
            plan.rule(),
            plan.join_plan(),
            plan.first_rows(),
            &support,
            &mut budget,
            Context::new(&mut computation, &limits, &mut counters, rule.location),
        )
        .unwrap()
    };
    assert!(prefixes.depth() >= 2 && prefixes.depth() <= 3);
    assert!(prefixes.len() >= super::super::prime::PRIME_GROUPS);
    let first = {
        let mut local = Counters::default();
        let mut walk = testing::budget();
        execute_borrowed(
            &plan,
            0..plan.len(),
            &append,
            &limits,
            &mut walk,
            &mut local,
        )
        .unwrap()
    };
    let seeded = {
        let mut local = Counters::default();
        let mut walk = testing::budget();
        execute_prefixed(
            &plan,
            &prefixes,
            0..prefixes.len(),
            &append,
            &limits,
            &mut walk,
            &mut local,
        )
        .unwrap()
    };
    assert_eq!(seeded.len(), first.len());
    let published = {
        let mut computation = Computation::new(&mut append, &support);
        publish(
            &plan,
            [&seeded],
            &support,
            &mut computation,
            &limits,
            &mut counters,
            0,
        )
        .unwrap()
    };
    assert_eq!(pending_heads(&append), expected);
    assert_eq!(published, expected.len());
}
