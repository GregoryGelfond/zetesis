//! Real source occurrences, cached schedules and bounded preparation.

mod schedule;

use super::*;
use crate::FormulaResource;
use crate::formula::Preparation;
use crate::formula_support::{GroundingWork, SupportCatalog, complete};
use zetesis_core::Atom;
use zetesis_test_support::programs::signed_numbered as atom;

fn prepare(source: &str) -> Preparation {
    crate::formula_support::testing::prepare(source)
}

fn location(prepared: &Prepared) -> ProgramSite {
    prepared.rules_without_facts().first().unwrap().location
}

fn plan<'a>(prepared: &'a Prepared, catalog: &SupportCatalog) -> ProducerPlan<'a> {
    ProducerPlan::prepare(
        prepared,
        catalog,
        &FormulaLimits::default(),
        &mut Counters::default(),
        location(prepared),
    )
    .unwrap()
    .unwrap()
}

fn atoms(prepared: Preparation, scheduled: bool) -> Vec<Atom> {
    let Preparation {
        catalog,
        accounting,
        program: prepared,
        mut budget,
        ..
    } = prepared;
    let limits = FormulaLimits::default();
    let mut counters = Counters::resume(accounting, crate::grounding_observer::Work::default());
    let selected = if scheduled {
        Producers::Planned(plan(&prepared, &catalog))
    } else {
        Producers::registered(
            &prepared,
            &catalog,
            &limits,
            &mut counters,
            location(&prepared),
        )
        .unwrap()
    };
    let catalog = complete(
        catalog,
        &prepared,
        selected,
        None,
        &mut budget,
        GroundingWork::new(&limits, &mut counters, location(&prepared)),
    )
    .unwrap();
    let snapshot = catalog
        .snapshot(&limits, &mut counters, location(&prepared))
        .unwrap();
    let mut atoms = Vec::new();
    for (_, source) in snapshot.relations.source_atoms() {
        // Explicit test-oracle export, outside measured execution.
        atoms.extend(
            source
                .iter()
                .map(|atom| atom.to_atom(zetesis_core::ValueLimits::default()).unwrap()),
        );
    }
    atoms.sort();
    atoms
}

#[test]
fn pool_alternatives_keep_distinct_plan_occurrences() {
    // Equal textual statements may already share a carrier and its origins.
    // Admitted head pool alternatives instead create two distinct IR occurrences
    // before the separate analyzed program coalesces their equal rules. Body
    // atom pools are outside the current source pool profile.
    // The fact d(0) is a fact block, which has no plan slot.
    let owner = prepare("d(0).p(1;1):-d(0).");
    let prepared = &owner.program;
    let plan = plan(prepared, &owner.catalog);
    let rules = prepared.rules_without_facts();
    assert_eq!(rules.len(), 2);
    assert_eq!(prepared.analyzed.full_program().statements().count(), 2);
    assert_eq!(plan.rules.len(), 2);
    for (index, rule) in rules.iter().enumerate() {
        assert!(plan.source.contains(index, rule));
        let occurrences = plan.rules[index].as_ref().unwrap();
        assert_eq!(
            &plan.inputs[occurrences.clone()],
            &(0..rule.body.len()).collect::<Vec<_>>()
        );
    }
    assert_eq!(plan.rules[0], Some(0..1));
    assert_eq!(plan.rules[1], Some(1..2));
    assert!(!rules[0].origins.is_empty());
    assert_eq!(rules[0].origins, rules[1].origins);
    assert_eq!(rules[0].location, rules[1].location);
}

#[test]
fn planned_completion_preserves_the_full_support_carrier() {
    let source =
        "p(1).p(1).-p(2).p(3,4).r(X):-p(X).s(X):-r(X).r(X):-s(X).t(X,Y):-p(X),p(Y).:-s(7).";
    let actual = atoms(prepare(source), true);
    let reference = atoms(prepare(source), false);
    let mut expected = vec![
        atom("p", zetesis_core::Sign::Positive, &[1]),
        atom("p", zetesis_core::Sign::Negative, &[2]),
        atom("p", zetesis_core::Sign::Positive, &[3, 4]),
        atom("r", zetesis_core::Sign::Positive, &[1]),
        atom("s", zetesis_core::Sign::Positive, &[1]),
        atom("t", zetesis_core::Sign::Positive, &[1, 1]),
    ];
    expected.sort();
    assert_eq!(actual, expected);
    assert_eq!(actual, reference);
}

fn pivots(mut variants: Variants<'_, '_>, counters: &mut Counters) -> Vec<Option<usize>> {
    let mut result = Vec::new();
    while let Some(variant) = variants.next(&FormulaLimits::default(), counters).unwrap() {
        result.push(match variant {
            delta::Variant::Full => None,
            delta::Variant::Delta(pivot) => Some(pivot),
        });
    }
    result
}

#[test]
fn repeated_schedules_reuse_original_positive_occurrences() {
    let Preparation {
        mut catalog,
        program: prepared,
        accounting,
        ..
    } = prepare("p(1).p(2).r(X,Y):-p(X),p(Y).");
    let plan = plan(&prepared, &catalog);
    let limits = FormulaLimits::default();
    let mut counters = Counters::resume(accounting, crate::grounding_observer::Work::default());
    for value in [1, 2] {
        catalog = catalog
            .insert(
                &atom("p", zetesis_core::Sign::Positive, &[value]),
                &limits,
                &mut counters,
                location(&prepared),
            )
            .unwrap();
    }
    let snapshot = catalog
        .snapshot(&limits, &mut counters, location(&prepared))
        .unwrap();
    let support = Support::indexed(&snapshot, &limits, &counters, location(&prepared)).unwrap();
    let index = prepared.rules_without_facts().len() - 1;
    let rule = &prepared.rules_without_facts()[index];
    let storage = plan.inputs.as_ptr();
    let registration = Registration::prepare(
        prepared.rules_without_facts(),
        support.components(),
        0,
        &limits,
        &mut Counters::default(),
        location(&prepared),
    )
    .unwrap();
    let mut planned = Counters::default();
    let mut registered = Counters::default();
    for _ in 0..8 {
        let cached = plan
            .variants(index, rule, &support, false, &limits, &mut planned)
            .unwrap();
        let recorded = registration
            .variants(index, rule, &support, false, &limits, &mut registered)
            .unwrap();
        assert_eq!(pivots(cached, &mut planned), vec![Some(0), Some(1)]);
        assert_eq!(pivots(recorded, &mut registered), vec![Some(0), Some(1)]);
        assert_eq!(plan.inputs.as_ptr(), storage);
    }
    // Construction was separate. Each repeated call takes the same recorded
    // occurrences; the plan charges its checked original-owner lookup, while
    // the registration's owner check is a pointer comparison covered by the
    // visit's own unit.
    assert_eq!(planned.accounting.work - registered.accounting.work, 8);
}

#[test]
fn foreign_rule_owners_cannot_use_cached_occurrences() {
    let source = "p(1).r(X):-p(X).";
    let owner = prepare(source);
    let prepared = &owner.program;
    let foreign_owner = prepare(source);
    let foreign = &foreign_owner.program;
    let plan = plan(prepared, &owner.catalog);
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let snapshot = owner
        .catalog
        .snapshot(&limits, &mut counters, location(prepared))
        .unwrap();
    let support = Support::indexed(&snapshot, &limits, &counters, location(prepared)).unwrap();
    let index = foreign.rules_without_facts().len() - 1;
    let rule = &foreign.rules_without_facts()[index];
    let Err(failure) = plan.variants(index, rule, &support, true, &limits, &mut counters) else {
        panic!("an equal rule from another preparation is not this occurrence");
    };
    assert!(
        matches!(failure, FormulaFailure::SupportRelation { error: Failure::Owner, location } if location == rule.location)
    );
}

#[test]
fn richer_sources_retain_the_existing_schedule() {
    for source in [
        "p(1).r(X):-p(X),not absent(X).",
        "p(1).r(X+1):-p(X).",
        "p(1).{r(X)}:-p(X).",
    ] {
        let owner = prepare(source);
        let prepared = &owner.program;
        assert!(
            ProducerPlan::prepare(
                prepared,
                &owner.catalog,
                &FormulaLimits::default(),
                &mut Counters::default(),
                location(prepared)
            )
            .unwrap()
            .is_none(),
            "{source}"
        );
    }
}

#[test]
fn a_body_comparison_is_no_producer_input() {
    // The comparison offers no rows and no relation wakes it, so the plan
    // schedules the rule on its one atom occurrence and completes the same
    // possible support as the unplanned rounds, in which X = 2 excludes
    // the one substitution.
    for (source, expected) in [
        (
            "p(1).r(X):-p(X),X=1.",
            vec![
                atom("p", zetesis_core::Sign::Positive, &[1]),
                atom("r", zetesis_core::Sign::Positive, &[1]),
            ],
        ),
        (
            "p(1).r(X):-p(X),X=2.",
            vec![atom("p", zetesis_core::Sign::Positive, &[1])],
        ),
    ] {
        let owner = prepare(source);
        let prepared = &owner.program;
        let planned = plan(prepared, &owner.catalog);
        // The fact p(1) has no slot; the rule's one input is its atom.
        assert_eq!(planned.rules[0], Some(0..1), "{source}");
        assert_eq!(atoms(prepare(source), true), expected, "{source}");
        assert_eq!(atoms(prepare(source), false), expected, "{source}");
    }
}

#[test]
fn work_refusals_do_not_publish_partial_plans() {
    let owner = prepare("p(1).r(X):-p(X).s(X):-r(X).r(X):-s(X).");
    let prepared = &owner.program;
    let mut complete = Counters::default();
    ProducerPlan::prepare(
        prepared,
        &owner.catalog,
        &FormulaLimits::default(),
        &mut complete,
        location(prepared),
    )
    .unwrap()
    .unwrap();
    assert!(complete.accounting.work > 1);
    for maximum in 0..complete.accounting.work {
        let limits = FormulaLimits {
            max_work: maximum,
            ..FormulaLimits::default()
        };
        let mut counters = Counters::default();
        let Err(error) = ProducerPlan::prepare(
            prepared,
            &owner.catalog,
            &limits,
            &mut counters,
            location(prepared),
        ) else {
            panic!("a proper charged prefix cannot publish this plan");
        };
        assert!(
            matches!(error, FormulaFailure::Limit { resource: FormulaResource::Work, observed, limit, .. } if observed > limit && limit == u128::from(maximum))
        );
        assert!(counters.accounting.work <= maximum);
    }
}

#[test]
fn producer_storage_is_admitted_before_reservation() {
    let owner = prepare("p(1).r(X):-p(X).");
    let prepared = &owner.program;
    let required = owner.catalog.bytes(location(prepared)).unwrap() + size_of::<ProducerPlan<'_>>();
    let limits = FormulaLimits {
        max_support_bytes: required - 1,
        ..FormulaLimits::default()
    };
    let mut counters = Counters::default();
    let Err(error) = ProducerPlan::prepare(
        prepared,
        &owner.catalog,
        &limits,
        &mut counters,
        location(prepared),
    ) else {
        panic!("the catalog and producer header exceed this allowance");
    };
    assert!(
        matches!(error, FormulaFailure::Limit { resource: FormulaResource::SupportBytes, observed, limit, .. } if observed == required as u128 && limit == limits.max_support_bytes as u128)
    );
    assert!(counters.accounting.work > 0);
}

#[test]
fn objective_observations_keep_the_existing_schedule() {
    for objective in [
        "#minimize{X:p(X)}.",
        ":~p(X).[X]",
        "#minimize{}.",
        "#maximize{}.",
    ] {
        let owner = prepare(&format!("p(1).r(X):-p(X).{objective}"));
        let prepared = &owner.program;
        let limits = FormulaLimits::default();
        let mut counters = Counters::default();
        let components = owner
            .catalog
            .component_view(&limits, &mut counters, location(prepared))
            .unwrap();
        let source = PositiveSource::check(
            prepared,
            components,
            &limits,
            &mut counters,
            location(prepared),
        )
        .unwrap()
        .unwrap();
        assert!(source.has_objectives(), "{objective}");
        assert!(
            ProducerPlan::prepare(
                prepared,
                &owner.catalog,
                &limits,
                &mut counters,
                location(prepared),
            )
            .unwrap()
            .is_none(),
            "{objective}"
        );
    }
}
