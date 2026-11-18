use super::*;
use crate::formula_support::{Accounting, SupportCatalog, testing};
use crate::grounding_observer::Work;
use crate::{ExpansionLimits, ExpansionResource, ExpansionUsage, FormulaResource};
use std::sync::Barrier;
use zetesis_algebra::Cancellation;
use zetesis_core::{Atom, Predicate, Value, ValueLimits};

pub(super) const JOIN: &str =
    "edge(1,2). edge(1,3). edge(2,4). edge(3,4). edge(4,5). out(X,Z):-edge(X,Y),edge(Y,Z).";

pub(super) fn prepared_source(source: &str) -> (crate::formula_ir::Prepared, SupportCatalog) {
    let preparation = testing::prepare(source);
    let mut program = preparation.program;
    // Keep all admitted source patterns and values, but complete only inputs.
    // This makes the subsequent ordered head publication observable as a real
    // new support delta rather than re-inserting already-derived output facts.
    let saved = std::mem::replace(
        &mut program.rules_without_facts_mut().last_mut().unwrap().head,
        HeadIr::Normal(None),
    );
    let mut budget = preparation.budget;
    let mut counters = Counters::resume(preparation.accounting, Work::default());
    let catalog = crate::formula_support::build(
        preparation.catalog,
        &program,
        None,
        &preparation.limits,
        &mut budget,
        &mut counters,
        preparation.location,
    )
    .unwrap();
    program.rules_without_facts_mut().last_mut().unwrap().head = saved;
    (program, catalog.catalog)
}

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

fn decoded(
    batch: &RowBatch,
    computation: &Computation<'_, '_>,
    location: ProgramSite,
) -> Vec<Vec<Value>> {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut binding = Binding::new(computation, &limits, &mut counters, location).unwrap();
    binding
        .extend_scope(batch.width, computation, &limits, &mut counters, location)
        .unwrap();
    (0..batch.rows)
        .map(|row| {
            for variable in 0..batch.width {
                binding
                    .set(
                        variable,
                        &batch.values.slice()[row * batch.width + variable],
                        &limits,
                        &mut counters,
                        location,
                    )
                    .unwrap();
            }
            values(&binding, computation, location)
        })
        .collect()
}

pub(super) fn atom(name: &str, arguments: &[i32]) -> Atom {
    Atom::new(
        Predicate::new(name, arguments.len()).unwrap(),
        arguments
            .iter()
            .map(|value| Value::Number(*value))
            .collect(),
    )
    .unwrap()
}

pub(super) fn worker_expansion(workers: usize, used: ExpansionUsage) -> ExpansionLimits {
    let parent = ExpansionLimits::default();
    ExpansionLimits {
        max_constants: parent.max_constants / workers,
        max_term_work: (parent.max_term_work - used.term_work) / workers,
        max_templates: (parent.max_templates - used.templates) / workers,
        max_values: (parent.max_values - used.values) / workers,
        max_scalar_bytes: (parent.max_scalar_bytes - used.scalar_bytes) / workers,
        max_family_bytes: parent.max_family_bytes / workers,
        max_origin_locations: (parent.max_origin_locations - used.origin_locations) / workers,
        max_metadata_statements: parent.max_metadata_statements / workers,
    }
}

pub(super) fn settle_expansion(budget: &mut Budget, usage: ExpansionUsage, location: ProgramSite) {
    for (resource, amount) in [
        (ExpansionResource::TermWork, usage.term_work),
        (ExpansionResource::Templates, usage.templates),
        (ExpansionResource::Values, usage.values),
        (ExpansionResource::ScalarBytes, usage.scalar_bytes),
        (ExpansionResource::Origins, usage.origin_locations),
    ] {
        budget.charge(resource, amount as u128, location).unwrap();
    }
}

type WorkerResult = (
    Result<RowBatch, FormulaFailure>,
    u64,
    u64,
    std::thread::ThreadId,
    ExpansionUsage,
);

fn run_blocks<'source>(
    plan: &RowPlan<'_, 'source>,
    relations: &'source relations::Relations<'source>,
    append: &'source super::super::SupportAppend<'source>,
    workers: usize,
    per_worker: &FormulaLimits,
    expansion_grant: ExpansionLimits,
) -> Vec<WorkerResult> {
    let barrier = Barrier::new(workers);
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for block in 0..workers {
            let lookup = append.term_lookup();
            let barrier = &barrier;
            let start = plan.len() * block / workers;
            let end = plan.len() * (block + 1) / workers;
            handles.push(scope.spawn(move || {
                let mut accounting = Accounting::default();
                let mut budget = Budget::new(expansion_grant, usize::MAX);
                barrier.wait();
                let result =
                    accounting.controlled(None, Work::default(), plan.location(), |local| {
                        execute(
                            plan,
                            start..end,
                            relations,
                            lookup,
                            per_worker,
                            &mut budget,
                            local,
                        )
                    });
                (
                    result,
                    accounting.work,
                    accounting.substitutions,
                    std::thread::current().id(),
                    budget.usage(),
                )
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().expect("worker did not panic"))
            .collect::<Vec<_>>()
    })
}

fn settle_workers(
    results: &[WorkerResult],
    counters: &mut Counters,
    budget: &mut Budget,
    limits: &FormulaLimits,
    location: ProgramSite,
) {
    // All tasks already finished. Settle all their real work before any
    // publication can consume the remaining common allowance. A reservation
    // is not a receipt, and later siblings' work is not refundable.
    let completed_work: u128 = results
        .iter()
        .map(|(_, work, _, _, _)| u128::from(*work))
        .sum();
    counters.absorb(completed_work, limits, location).unwrap();
    let completed_substitutions: u128 = results
        .iter()
        .map(|(_, _, substitutions, _, _)| u128::from(*substitutions))
        .sum();
    let total_substitutions = zetesis_algebra::Meter::check_receipt(
        counters.accounting.substitutions,
        completed_substitutions,
        limits.max_substitutions,
    )
    .unwrap();
    counters.accounting.substitutions = total_substitutions;
    for (_, _, _, _, usage) in results {
        settle_expansion(budget, *usage, location);
    }
    assert!(results.iter().all(|(result, _, _, _, _)| result.is_ok()));
}

fn completed_pair<'source>(
    plan: &RowPlan<'_, 'source>,
    relations: &'source relations::Relations<'source>,
    append: &'source super::super::SupportAppend<'source>,
    grants: &FormulaLimits,
    expansion: ExpansionLimits,
) -> Vec<(RowBatch, u64, u64, ExpansionUsage)> {
    std::thread::scope(|scope| {
        let handles = (0..2)
            .map(|block| {
                let lookup = append.term_lookup();
                scope.spawn(move || {
                    let mut account = Accounting::default();
                    let mut budget = Budget::new(expansion, usize::MAX);
                    let result =
                        account.controlled(None, Work::default(), plan.location(), |local| {
                            execute(
                                plan,
                                plan.len() * block / 2..plan.len() * (block + 1) / 2,
                                relations,
                                lookup,
                                grants,
                                &mut budget,
                                local,
                            )
                        });
                    (
                        result.unwrap(),
                        account.work,
                        account.substitutions,
                        budget.usage(),
                    )
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    })
}

fn reference_and_plan<'plan, 'source: 'plan>(
    rule: &'source RuleIr,
    variant: delta::Variant,
    support: &'plan Support<'source>,
    append: &mut super::super::SupportAppend<'source>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> (RowPlan<'plan, 'source>, Vec<Vec<Value>>) {
    let mut computation = Computation::new(append, support);
    let mut join = Join::variant_rule(
        Ir::Source(rule),
        variant,
        support,
        None,
        budget,
        Context::new(&computation, limits, counters, rule.location),
    )
    .unwrap();
    let mut serial = Vec::new();
    while let Some(binding) = join
        .next_support(
            None,
            &mut computation,
            limits,
            budget,
            counters,
            rule.location,
        )
        .unwrap()
    {
        serial.push(values(&binding, &computation, rule.location));
    }
    drop(join);
    let plan = RowPlan::prepare(
        rule,
        variant,
        support,
        &mut computation,
        limits,
        budget,
        counters,
    )
    .unwrap()
    .unwrap();
    (plan, serial)
}

struct Comparison {
    serial: Vec<Vec<Value>>,
    parallel: Vec<Vec<Value>>,
    heads: Vec<Atom>,
    ids: Vec<std::thread::ThreadId>,
    work: Vec<u64>,
    substitutions: Vec<u64>,
}

/// A deliberately explicit qualification harness: thread handles, the barrier
/// and test-owned observations are not claimed as production account owners.
/// Actual cursor/output storage is bounded by disjoint worker grants. No
/// worker receives the whole parent work, substitution or scratch allowance.
fn comparison(source: &str, workers: usize) -> Comparison {
    comparison_variant(source, workers, delta::Variant::Full, None)
}

fn comparison_variant(
    source: &str,
    workers: usize,
    variant: delta::Variant,
    added: Option<Atom>,
) -> Comparison {
    let (program, mut catalog) = prepared_source(source);
    let rule = program.rules_without_facts().last().unwrap();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut budget = testing::budget();
    if let Some(added) = added {
        {
            let (_, mut append) = catalog
                .split(&limits, &mut counters, rule.location)
                .unwrap();
            append
                .atom((&added).into(), &limits, &mut counters, rule.location)
                .unwrap();
            append
                .order(&limits, &mut counters, rule.location, None)
                .unwrap();
        }
        catalog
            .publish(&limits, &mut counters, rule.location)
            .unwrap();
    }
    let (completed, mut append) = catalog
        .split(&limits, &mut counters, rule.location)
        .unwrap();
    let relations = &completed;
    let support = Support::indexed(relations, &limits, &counters, rule.location).unwrap();
    let (plan, serial) = reference_and_plan(
        rule,
        variant,
        &support,
        &mut append,
        &limits,
        &mut budget,
        &mut counters,
    );
    let remaining_work = limits.max_work - counters.accounting.work;
    let remaining_substitutions = limits.max_substitutions - counters.accounting.substitutions;
    let shared_bytes = relations.current_bytes();
    let coordinator_bytes = support.live_bytes();
    assert!(coordinator_bytes < limits.max_support_bytes);
    // Keep half the remaining capacity for serial head publication. Until
    // every result is dropped the complete worker reservation remains outside
    // the coordinator ledger, including each still-owned output buffer.
    let scratch = (limits.max_support_bytes - coordinator_bytes) / 2 / workers;
    let worker_reservation = scratch * workers;
    let publication_limits = FormulaLimits {
        max_support_bytes: limits.max_support_bytes - worker_reservation,
        ..limits
    };
    let per_worker = FormulaLimits {
        max_work: remaining_work / workers as u64,
        max_substitutions: remaining_substitutions / workers as u64,
        max_support_bytes: shared_bytes + scratch,
        ..limits
    };
    assert!(u128::from(per_worker.max_work) * workers as u128 <= u128::from(remaining_work));
    assert!(scratch * workers + coordinator_bytes <= limits.max_support_bytes);
    let expansion_grant = worker_expansion(workers, budget.usage());
    let results = run_blocks(
        &plan,
        relations,
        &append,
        workers,
        &per_worker,
        expansion_grant,
    );
    settle_workers(&results, &mut counters, &mut budget, &limits, rule.location);
    let mut parallel = Vec::new();
    let mut ids = Vec::new();
    let mut work = Vec::new();
    let mut substitutions = Vec::new();
    {
        let mut computation = Computation::new(&mut append, &support);
        for (result, charged, selected, id, _) in results {
            let result = result.unwrap();
            parallel.extend(decoded(&result, &computation, rule.location));
            result
                .publish(rule, &mut computation, &publication_limits, &mut counters)
                .unwrap();
            ids.push(id);
            work.push(charged);
            substitutions.push(selected);
        }
    }
    let heads = append
        .atoms()
        .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
        .collect();
    Comparison {
        serial,
        parallel,
        heads,
        ids,
        work,
        substitutions,
    }
}

#[test]
fn scoped_workers_reuse_the_actual_join_and_publish_the_same_ordered_heads() {
    let result = comparison(JOIN, 2);
    assert_eq!(result.parallel, result.serial);
    assert_eq!(result.serial.len(), 4);
    assert_ne!(result.ids[0], result.ids[1]);
    assert!(result.work.iter().all(|work| *work > 0));
    assert!(result.substitutions.iter().sum::<u64>() >= result.serial.len() as u64);
    assert_eq!(
        result.heads,
        [
            atom("out", &[1, 4]),
            atom("out", &[2, 5]),
            atom("out", &[3, 5])
        ]
    );
}

#[test]
fn changing_only_block_boundaries_preserves_complete_binding_multiplicity() {
    for workers in [1, 2, 3, 7] {
        let result = comparison(JOIN, workers);
        assert_eq!(result.parallel, result.serial, "workers={workers}");
        assert_eq!(result.parallel.len(), 4);
    }
}

#[test]
fn constants_repeated_variables_and_empty_suffixes_still_use_original_matching() {
    for source in [
        "r(1,1). r(1,2). r(2,2). out(X):-r(X,X).",
        "r(1,1). r(1,2). r(2,2). out(X):-r(1,X).",
        "r(1). r(2). q(3). out(X):-r(X),q(X).",
        "r(f(1)). r(f(2)). out:-r(f(1)).",
    ] {
        let result = comparison(source, 3);
        assert_eq!(result.parallel, result.serial, "{source}");
    }
}

#[test]
fn a_ground_zero_variable_binding_is_not_lost_by_empty_output_cells() {
    let result = comparison("p. out:-p.", 2);
    assert_eq!(result.serial, [Vec::<Value>::new()]);
    assert_eq!(result.parallel, result.serial);
    assert_eq!(result.heads, [atom("out", &[])]);
}

#[test]
fn unsupported_source_shapes_decline_before_parallel_selection() {
    for source in [
        "p(1). out(f(X)):-p(X).",
        "p(f(1)). out(X):-p(f(X)).",
        "p(1). out(X):-p(X),not q(X).",
        "p(1). out(X+1):-p(X).",
        "p(1). out(X):-p(X),X>0.",
        "p(1). {out(X)}:-p(X).",
    ] {
        let preparation = testing::prepare(source);
        let rule = preparation.program.rules_without_facts().last().unwrap();
        assert!(!eligible(rule), "{source}");
    }
}

#[test]
fn refused_block_retains_actual_work_and_publishes_no_heads() {
    let (program, mut catalog) = prepared_source(JOIN);
    let rule = program.rules_without_facts().last().unwrap();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut budget = testing::budget();
    let (completed, mut append) = catalog
        .split(&limits, &mut counters, rule.location)
        .unwrap();
    let support = Support::indexed(&completed, &limits, &counters, rule.location).unwrap();
    let plan = {
        let mut computation = Computation::new(&mut append, &support);
        RowPlan::prepare(
            rule,
            delta::Variant::Full,
            &support,
            &mut computation,
            &limits,
            &mut budget,
            &mut counters,
        )
        .unwrap()
        .unwrap()
    };
    let mut complete = Accounting::default();
    let result = complete
        .controlled(None, Work::default(), rule.location, |local| {
            execute(
                &plan,
                0..plan.len(),
                &completed,
                append.term_lookup(),
                &limits,
                &mut testing::budget(),
                local,
            )
        })
        .unwrap();
    assert_eq!(result.len(), 4);
    drop(result);
    for work in [0, 1, complete.work / 2, complete.work - 1] {
        let bounded = FormulaLimits {
            max_work: work,
            ..limits
        };
        let mut accounting = Accounting::default();
        let result = accounting.controlled(None, Work::default(), rule.location, |local| {
            execute(
                &plan,
                0..plan.len(),
                &completed,
                append.term_lookup(),
                &bounded,
                &mut testing::budget(),
                local,
            )
        });
        assert!(
            matches!(
                result,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                })
            ),
            "limit={work}"
        );
        assert!(accounting.work <= work);
        assert!(work == 0 || accounting.work > 0);
        assert!(append.is_empty());
    }
}

#[test]
fn cancelled_worker_keeps_its_prior_receipt_without_publishing_a_partial_block() {
    let (program, mut catalog) = prepared_source(JOIN);
    let rule = program.rules_without_facts().last().unwrap();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let (completed, mut append) = catalog
        .split(&limits, &mut counters, rule.location)
        .unwrap();
    let support = Support::indexed(&completed, &limits, &counters, rule.location).unwrap();
    let plan = {
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
    let cancellation = Cancellation::default();
    let mut accounting = Accounting::default();
    // Record genuine previous charged work, then stop before this task enters.
    accounting
        .controlled::<_, FormulaFailure>(None, Work::default(), rule.location, |local| {
            local.charge_work(7, &limits, rule.location)
        })
        .unwrap();
    let result = accounting.controlled(
        Some(&cancellation),
        Work::default(),
        rule.location,
        |local| {
            cancellation.cancel();
            execute(
                &plan,
                0..plan.len(),
                &completed,
                append.term_lookup(),
                &limits,
                &mut testing::budget(),
                local,
            )
        },
    );
    assert!(matches!(result, Err(FormulaFailure::Interrupted { .. })));
    assert!(
        accounting.work > 7,
        "the actual join performed work after entry"
    );
    assert!(accounting.work <= zetesis_algebra::POLL_INTERVAL);
    assert!(append.is_empty());
}

#[test]
fn first_new_occurrence_variants_partition_the_new_bindings_before_row_blocks() {
    let added = || Some(atom("edge", &[4, 2]));
    let mut heads = Vec::new();
    let mut rows = 0;
    for pivot in 0..2 {
        let result = comparison_variant(JOIN, 3, delta::Variant::Delta(pivot), added());
        assert_eq!(result.parallel, result.serial, "pivot={pivot}");
        rows += result.parallel.len();
        heads.extend(result.heads);
    }
    assert_eq!(rows, 3);
    heads.sort();
    let mut expected = vec![
        atom("out", &[4, 4]),
        atom("out", &[2, 2]),
        atom("out", &[3, 2]),
    ];
    expected.sort();
    assert_eq!(heads, expected);
}

#[test]
fn completed_sibling_work_is_settled_before_a_tight_publication_refusal() {
    let (program, mut catalog) = prepared_source(JOIN);
    let rule = program.rules_without_facts().last().unwrap();
    let limits = FormulaLimits::default();
    let mut coordinator = Counters::default();
    let mut budget = testing::budget();
    let (relations, mut append) = catalog
        .split(&limits, &mut coordinator, rule.location)
        .unwrap();
    let support = Support::indexed(&relations, &limits, &coordinator, rule.location).unwrap();
    let plan = {
        let mut computation = Computation::new(&mut append, &support);
        RowPlan::prepare(
            rule,
            delta::Variant::Full,
            &support,
            &mut computation,
            &limits,
            &mut budget,
            &mut coordinator,
        )
        .unwrap()
        .unwrap()
    };
    let grants = FormulaLimits {
        max_work: (limits.max_work - coordinator.accounting.work) / 2,
        max_substitutions: (limits.max_substitutions - coordinator.accounting.substitutions) / 2,
        max_support_bytes: relations.current_bytes()
            + (limits.max_support_bytes - support.live_bytes()) / 4,
        ..limits
    };
    let expansion = worker_expansion(2, budget.usage());
    let before_work = coordinator.accounting.work;
    let results = completed_pair(&plan, &relations, &append, &grants, expansion);
    let actual_work: u64 = results.iter().map(|(_, work, _, _)| *work).sum();
    let bounded = FormulaLimits {
        max_work: before_work + actual_work,
        max_support_bytes: limits.max_support_bytes
            - 2 * (grants.max_support_bytes - relations.current_bytes()),
        ..limits
    };
    coordinator
        .absorb(u128::from(actual_work), &bounded, rule.location)
        .unwrap();
    for (_, _, substitutions, usage) in &results {
        coordinator.accounting.substitutions = zetesis_algebra::Meter::check_receipt(
            coordinator.accounting.substitutions,
            u128::from(*substitutions),
            limits.max_substitutions,
        )
        .unwrap();
        settle_expansion(&mut budget, *usage, rule.location);
    }
    assert_eq!(coordinator.accounting.work, bounded.max_work);
    let result = {
        let mut computation = Computation::new(&mut append, &support);
        results[0]
            .0
            .publish(rule, &mut computation, &bounded, &mut coordinator)
    };
    assert!(matches!(
        result,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            ..
        })
    ));
    assert_eq!(coordinator.accounting.work, bounded.max_work);
    assert!(
        append.is_empty(),
        "neither block published a head before refusal"
    );
}
