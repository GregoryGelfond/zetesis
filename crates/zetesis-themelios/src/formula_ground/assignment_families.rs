//! Family sharing preserves the original theory and every frozen reduct.
use super::*;
use crate::formula_support::testing;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Limits, models, models_reduct};

/// Reproduce the prior cache boundary without changing the source or its joins.
fn full_binding_keys(preparation: &mut crate::formula::Preparation) {
    for rule in &mut preparation.program.rules {
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

fn interpretation(theory: &Theory, bits: usize) -> Interpretation {
    Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|atom| bits & (1 << atom) != 0),
    )
    .unwrap()
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

fn family_population(
    aggregate: &AggregateIr,
    support: &Support<'_>,
    budget: &mut Budget,
    context: Context<'_, &mut Computation<'_, '_>>,
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
        .assignment_aggregate(aggregate, target, &binding, support, location)
        .unwrap();
    let result = (builder.cached_elements, builder.cached_roots);
    *counters = builder.counters;
    result
}

#[test]
fn aggregate_families_do_not_cross_support_snapshots() {
    let mut preparation = testing::prepare("p(1). n(N) :- N=#count{X:p(X)}.");
    let aggregate = preparation
        .program
        .rules
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
