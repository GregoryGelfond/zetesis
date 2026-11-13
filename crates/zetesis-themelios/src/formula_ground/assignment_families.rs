//! Family sharing preserves the original theory and every frozen reduct.
use super::*;
use crate::formula_support::testing;
use crate::test_support::interpretation;
use zetesis_algebra::Cancellation;
use zetesis_ferraris::{Limits, models, models_reduct};

/// Reproduce the prior cache boundary without changing the source or its joins.
fn full_binding_keys(preparation: &mut crate::formula::Preparation) {
    for rule in preparation.program.rules_without_facts_mut() {
        if let Some(plan) = &mut rule.bindings {
            plan.retain_required_for_test(&rule.body);
        }
        for literal in &mut rule.body {
            if let LiteralIr::Aggregate(aggregate) = literal
                && let Some(target) = aggregate.binding
            {
                aggregate.family_inputs = (0..rule.body_variables)
                    .filter(|&slot| slot != target)
                    .collect();
            }
        }
    }
}

#[test]
fn unrelated_bindings_share_bounded_assignment_families() {
    let source = "u(1..8). {p(1..3)}. q(X,A,B) :- u(X), A=#count{Z:p(Z)}, B=#count{Z:p(Z)}.";
    let shared = ground_retained(testing::prepare(source), None, false).unwrap();
    let mut prior = testing::prepare(source);
    full_binding_keys(&mut prior);
    let prior = ground_retained(prior, None, false).unwrap();
    assert!(shared.accounting.work < prior.accounting.work);
    assert_eq!(shared.compiled.atoms.atoms(), prior.compiled.atoms.atoms());
    // Two source aggregates each retain one family with four equality roots.
    // The unrelated X and the other proposal do not multiply either owner.
    for limits in [
        FormulaLimits {
            max_aggregate_cache_rows: 2,
            ..Default::default()
        },
        FormulaLimits {
            max_aggregate_cache_roots: 8,
            ..Default::default()
        },
    ] {
        let mut preparation = testing::prepare(source);
        preparation.limits = limits;
        ground(preparation, None, None).unwrap();
        let mut prior = testing::prepare(source);
        prior.limits = limits;
        full_binding_keys(&mut prior);
        assert!(matches!(
            ground(prior, None, None),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::AggregateCacheRows
                    | FormulaResource::AggregateCacheRoots,
                ..
            })
        ));
    }
}

fn same_truth(source: &str) {
    let shared = ground(testing::prepare(source), None, None).unwrap();
    let mut prior = testing::prepare(source);
    full_binding_keys(&mut prior);
    let prior = ground(prior, None, None).unwrap();
    assert_eq!(shared.atoms.atoms(), prior.atoms.atoms(), "{source}");
    // Complete interpretations, including candidates that violate source facts,
    // and arbitrary tested interpretations, not only proper candidate subsets.
    assert!(
        shared.theory.atom_count() <= 7,
        "keep the complete truth table finite"
    );
    let count = 1 << shared.theory.atom_count();
    let limits = Limits::default();
    let cancellation = Cancellation::default();
    for candidate in 0..count {
        let left = interpretation(&shared.theory, candidate);
        let right = interpretation(&prior.theory, candidate);
        assert_eq!(
            models(&shared.theory, &left, limits, &cancellation).unwrap(),
            models(&prior.theory, &right, limits, &cancellation).unwrap(),
            "original candidate {candidate}: {source}",
        );
        for tested in 0..count {
            assert_eq!(
                models_reduct(
                    &shared.theory,
                    &left,
                    &interpretation(&shared.theory, tested),
                    limits,
                    &cancellation,
                )
                .unwrap(),
                models_reduct(
                    &prior.theory,
                    &right,
                    &interpretation(&prior.theory, tested),
                    limits,
                    &cancellation,
                )
                .unwrap(),
                "candidate {candidate}, tested {tested}: {source}",
            );
        }
    }
}

#[test]
fn projected_families_preserve_formula_semantics() {
    for source in [
        // X is read only by the tuple key; at zero the duplicate collapses.
        "d(0;1). {a}. ok :- d(X), d(Y), N=#count{X:a;0:a}, N=1.",
        // A local witness reads X through an arithmetic condition.
        "d(0;1). p(1). {a}. ok :- d(X), d(Y), N=#count{Z:p(Z),Z=X,not a}, N=1.",
        // Whole typed values and strong negation remain part of eligibility.
        "d(a;f(a)). {-p(a);-p(f(a))}. ok :- d(X), N=#count{1: -p(X)}, N=1.",
        // A guard-only read belongs to the separate nonbinding aggregate.
        "d(0;1). {a}. ok :- d(X), N=#count{1:a}, #count{1:a}!=X.",
        // Correlated proposals, signed weights and default negation retain
        // equality in the original formula and in every frozen reduct.
        "{a}. ok :- A=#count{1:a}, B=#sum{-1,A:not a;1,0:a}, B=A.",
        // Extrema retain typed empty sentinels and value-dependent tuple keys.
        "d(0;1). {a}. ok :- d(X), d(Y), N=#min{X:a}, N=0.",
    ] {
        same_truth(source);
    }
}

#[test]
fn projected_families_keep_outer_arithmetic_diagnostics() {
    let source = "d(0;1). {a}. ok :- d(X), N=#count{1:a,1/(1-X)>0}.";
    let shared = ground(testing::prepare(source), None, None).unwrap_err();
    let mut prior = testing::prepare(source);
    full_binding_keys(&mut prior);
    let prior = ground(prior, None, None).unwrap_err();
    assert!(matches!(
        shared,
        FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation { .. })
    ));
    assert_eq!(shared.to_string(), prior.to_string());
}

fn family_population<'source>(
    aggregate: &AggregateIr,
    support: &Support<'source>,
    budget: &mut Budget,
    context: Context<'_, &mut Computation<'_, 'source>>,
) -> (usize, usize) {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    let target = aggregate.binding.unwrap();
    let mut binding = Binding::new(computation, limits, counters, location).unwrap();
    binding
        .extend_scope(target + 1, computation, limits, counters, location)
        .unwrap();
    let zero = computation.number(0, limits, counters, location).unwrap();
    binding
        .set(target, &zero, limits, counters, location)
        .unwrap();
    let mut builder = Builder::empty(
        computation,
        limits,
        budget,
        counters,
        Purpose::Validation,
        None,
        location,
    )
    .unwrap();
    builder.initialize(location).unwrap();
    builder
        .assignment_aggregate(aggregate.into(), target, &binding, support, location, None)
        .unwrap();
    (
        builder.families.cached_elements.get(),
        builder.families.cached_roots.get(),
    )
}

#[test]
fn aggregate_families_do_not_cross_support_snapshots() {
    // A chosen atom: a fact's would be certain, and its tuple required.
    let mut preparation = testing::prepare("{p(1)}. n(N) :- N=#count{X:p(X)}.");
    let aggregate = preparation
        .program
        .rules_without_facts()
        .iter()
        .flat_map(|rule| &rule.body)
        .find_map(|literal| match literal {
            LiteralIr::Aggregate(aggregate) => Some(aggregate),
            _ => None,
        })
        .unwrap();
    assert!(aggregate.family_inputs.is_empty());
    let limits = preparation.limits;
    let location = preparation.location;
    let mut counters = Counters::resume(
        preparation.accounting,
        crate::grounding_observer::Work::default(),
    );
    // Reuse this exact admitted IR and source authority, first over an empty
    // relation snapshot, then over the fixed point that contains p(1).
    let before = {
        let (relations, mut append) = preparation
            .catalog
            .split(&limits, &mut counters, location)
            .unwrap();
        let support = Support::indexed(&relations, &limits, &counters, location).unwrap();
        let mut computation = Computation::new(&mut append, &support);
        family_population(
            aggregate,
            &support,
            &mut preparation.budget,
            Context::new(&mut computation, &limits, &mut counters, location),
        )
    };
    let mut completed = formula_support::build(
        preparation.catalog,
        &preparation.program,
        None,
        &limits,
        &mut preparation.budget,
        &mut counters,
        location,
    )
    .unwrap();
    let after = {
        let (relations, mut append) = completed.split(&limits, &mut counters, location).unwrap();
        let queries = relations
            .queries(crate::JoinStrategy::Indexed, &limits, &counters, location)
            .unwrap();
        let mut computation = Computation::new(&mut append, queries.support());
        family_population(
            aggregate,
            queries.support(),
            &mut preparation.budget,
            Context::new(&mut computation, &limits, &mut counters, location),
        )
    };
    assert_eq!(before, (0, 1));
    assert_eq!(after, (1, 2));
}

/// Compare the borrowed proposal with an independent reconstruction from final
/// coalesced tuples. This exercises the representation boundary, not merely a
/// second call to the cursor's own proposal builder.
fn borrowed_matches_final_carrier(source: &str) {
    testing::with_completed_source(source, |prepared, support, computation, counters| {
        let rule = prepared
            .rules_without_facts()
            .iter()
            .find(|rule| rule.bindings.is_some())
            .unwrap();
        let limits = FormulaLimits::default();
        let location = rule.location;
        let mut budget = testing::budget();
        let mut builder = Builder::empty(
            computation,
            &limits,
            &mut budget,
            counters,
            Purpose::Validation,
            None,
            location,
        )
        .unwrap();
        builder.initialize(location).unwrap();
        let mut rows = Join::rule(
            Ir::Source(rule),
            support,
            builder.computation,
            &limits,
            builder.budget,
            builder.counters,
        )
        .unwrap();
        let mut checks = 0;
        while let Some(row) = rows
            .next_matched_row(
                builder.budget,
                Context::new(builder.computation, &limits, builder.counters, location),
            )
            .unwrap()
        {
            if !row.passes {
                continue;
            }
            let cursor = row.carriers.expect("assignment retains its live cursor");
            for literal in &rule.body {
                let LiteralIr::Aggregate(aggregate) = literal else {
                    continue;
                };
                if aggregate.binding.is_none() {
                    continue;
                }
                let retained = cursor
                    .assignment_carrier(
                        aggregate,
                        &row.values,
                        support,
                        Context::new(builder.computation, &limits, builder.counters, location),
                    )
                    .unwrap()
                    .expect("matching completed family");
                let (elements, required) = builder
                    .aggregate_elements(Ir::Source(aggregate), &row.values, support, location)
                    .unwrap();
                let required = required.expect("assignment required flags");
                let elements = match (Kind::of(aggregate.function), &elements) {
                    (Kind::Numeric(function), GroundAggregate::Numeric(elements)) => {
                        AssignmentElements::Numeric(function, elements.slice())
                    }
                    (Kind::Extremum(function), GroundAggregate::Extrema(elements)) => {
                        AssignmentElements::Extrema(function, elements.slice())
                    }
                    _ => unreachable!("one function kind"),
                };
                let rebuilt = builder
                    .aggregate_candidates(elements, &required, location)
                    .unwrap();
                assert_eq!(retained.len(), rebuilt.len(), "{source}");
                for slot in 0..retained.len() {
                    let key = rebuilt.key(slot, location).unwrap();
                    assert_eq!(
                        retained.slots().compare_key(slot, &key).unwrap(),
                        std::cmp::Ordering::Equal,
                        "{source}, slot {slot}",
                    );
                }
                checks += 1;
            }
        }
        assert!(
            checks > 0,
            "the fixture must exercise a borrowed family: {source}"
        );
    });
}

#[test]
fn borrowed_assignment_carriers_equal_final_tuple_reductions() {
    for source in [
        // Repeated full tuples collapse; equal weights on different tuples do not.
        "{a;b}. q(N) :- N=#count{1:a;1:b;2:a}.",
        "{a;b}. q(N) :- N=#sum{2,one:a;2,two:b;2,one:b;-1,three:a}.",
        "{a;b}. q(N) :- N=#sum+{-2:a;0:b;2:a;3:b}.",
        // Final coalescing keeps even FALSUM eligibility contributions. The
        // carrier remains a possible-value family, never an extra truth filter.
        "{p}. q(N) :- N=#sum{2:p,not p;3:p}.",
        // Required tuple overlap and ignored nonnumeric sum weights.
        "{a}. q(N) :- N=#sum{2;2:a;3:a;name:a}.",
        // Empty extrema preserve the typed #sup/#inf values in the carrier.
        "{a}. q(N) :- N=#min{4:a}.",
        "{a}. q(N) :- N=#max{4:a}.",
        "{a}. q(N) :- N=#min{4;2:a;7:a}.",
        // Structured term order, explicit negation and default negation.
        "{-p(f(a));p(a)}. q(N) :- N=#max{f(a): -p(f(a));a:not p(a)}.",
        // Family inputs vary with an earlier proposal and a local witness.
        "{a}. q(A,B) :- A=#count{1:a}, B=#sum{A:a;2,A:not a}.",
        "d(0;1). p(1). {a}. q(X,N) :- d(X), N=#count{Y:p(Y),Y=X,not a}.",
    ] {
        borrowed_matches_final_carrier(source);
    }
}

mod carrier_reuse;

mod reuse;
