//! Real source occurrences, cached schedules and bounded preparation.

mod schedule;

use super::*;
use crate::expansion::Budget;
use crate::formula_support::{SupportCatalog, complete, row_values};
use crate::{AdmissionOptions, ExpansionLimits, FormulaResource, ParsedSource};
use zetesis_core::{Atom, Value};

fn prepare(source: &str) -> Prepared {
    let parsed = ParsedSource::new(source.into(), AdmissionOptions::default()).unwrap();
    let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
    let mut choices = crate::formula_choice_source::Catalog::default();
    let raised = crate::formula_choice_source::raise(
        parsed.parsed(),
        &mut crate::metadata::Builder::default(),
        &mut budget,
        &mut choices,
    )
    .unwrap();
    crate::formula_ir::prepare(
        &raised,
        &choices,
        AdmissionOptions::default(),
        &FormulaLimits::default(),
        &mut budget,
        Location {
            source: parsed.source().id(),
            span: parsed.source().span(),
        },
    )
    .unwrap()
}

fn location(prepared: &Prepared) -> Location {
    prepared.rules.first().unwrap().location
}

fn plan(prepared: &Prepared) -> ProducerPlan<'_> {
    ProducerPlan::prepare(
        prepared,
        &FormulaLimits::default(),
        &mut Counters::default(),
        location(prepared),
    )
    .unwrap()
    .unwrap()
}

fn atom(name: &str, sign: zetesis_core::Sign, values: &[i32]) -> Atom {
    Atom::new(
        Predicate::with_sign(name, values.len(), sign).unwrap(),
        values.iter().copied().map(Value::Number).collect(),
    )
    .unwrap()
}

fn atoms(prepared: &Prepared, selected: Option<ProducerPlan<'_>>) -> Vec<Atom> {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let catalog = complete(
        prepared,
        selected,
        None,
        &limits,
        &mut Budget::new(ExpansionLimits::default(), usize::MAX),
        &mut counters,
        location(prepared),
    )
    .unwrap();
    let snapshot = catalog
        .snapshot(&limits, &mut counters, location(prepared))
        .unwrap();
    let mut atoms = Vec::new();
    for predicate in snapshot.relations.predicates() {
        for row in snapshot.relations.rows(predicate) {
            atoms.push(Atom::new(predicate.clone(), row_values(row).cloned().collect()).unwrap());
        }
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
    let prepared = prepare("d(0).p(1;1):-d(0).");
    let plan = plan(&prepared);
    assert_eq!(prepared.rules.len(), 3);
    assert_eq!(prepared.analyzed.statements().count(), 2);
    assert_eq!(plan.rules.len(), 3);
    for (index, rule) in prepared.rules.iter().enumerate() {
        assert!(plan.source.contains(index, rule));
        let occurrences = plan.rules[index].as_ref().unwrap();
        assert_eq!(
            &plan.inputs[occurrences.clone()],
            &(0..rule.body.len()).collect::<Vec<_>>()
        );
    }
    assert_eq!(plan.rules[1], Some(0..1));
    assert_eq!(plan.rules[2], Some(1..2));
    assert!(!prepared.rules[1].origins.is_empty());
    assert_eq!(prepared.rules[1].origins, prepared.rules[2].origins);
    assert_eq!(prepared.rules[1].location, prepared.rules[2].location);
}

#[test]
fn planned_completion_preserves_the_full_support_carrier() {
    let prepared = prepare(
        "p(1).p(1).-p(2).p(3,4).r(X):-p(X).s(X):-r(X).r(X):-s(X).t(X,Y):-p(X),p(Y).:-s(7).",
    );
    let actual = atoms(&prepared, Some(plan(&prepared)));
    let reference = atoms(&prepared, None);
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
    let prepared = prepare("p(1).p(2).r(X,Y):-p(X),p(Y).");
    let plan = plan(&prepared);
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    for value in [1, 2] {
        if value == 2 {
            catalog
                .advance(&limits, &mut counters, location(&prepared))
                .unwrap();
        }
        catalog = catalog
            .insert(
                atom("p", zetesis_core::Sign::Positive, &[value]),
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
    let index = prepared.rules.len() - 1;
    let rule = &prepared.rules[index];
    let storage = plan.inputs.as_ptr();
    let mut planned = Counters::default();
    let mut reference = Counters::default();
    for _ in 0..8 {
        let cached = plan
            .variants(index, rule, &support, false, &limits, &mut planned)
            .unwrap();
        let full = delta::variants(rule, &support, false, &limits, &mut reference).unwrap();
        assert_eq!(pivots(cached, &mut planned), vec![Some(0), Some(1)]);
        assert_eq!(pivots(full, &mut reference), vec![Some(0), Some(1)]);
        assert_eq!(plan.inputs.as_ptr(), storage);
    }
    // Construction was separate. Each repeated call replaces the two-literal
    // applicability scan with one checked original-owner lookup.
    assert_eq!(reference.work - planned.work, 8);
}

#[test]
fn foreign_rule_owners_cannot_use_cached_occurrences() {
    let source = "p(1).r(X):-p(X).";
    let prepared = prepare(source);
    let foreign = prepare(source);
    let plan = plan(&prepared);
    let limits = FormulaLimits::default();
    let catalog = SupportCatalog::default();
    let mut counters = Counters::default();
    let snapshot = catalog
        .snapshot(&limits, &mut counters, location(&prepared))
        .unwrap();
    let support = Support::indexed(&snapshot, &limits, &counters, location(&prepared)).unwrap();
    let index = foreign.rules.len() - 1;
    let rule = &foreign.rules[index];
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
        let prepared = prepare(source);
        assert!(
            ProducerPlan::prepare(
                &prepared,
                &FormulaLimits::default(),
                &mut Counters::default(),
                location(&prepared)
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
        let prepared = prepare(source);
        let planned = plan(&prepared);
        assert_eq!(planned.rules[1], Some(0..1), "{source}");
        assert_eq!(atoms(&prepared, Some(planned)), expected, "{source}");
        assert_eq!(atoms(&prepared, None), expected, "{source}");
    }
}

#[test]
fn work_refusals_do_not_publish_partial_plans() {
    let prepared = prepare("p(1).r(X):-p(X).s(X):-r(X).r(X):-s(X).");
    let mut complete = Counters::default();
    ProducerPlan::prepare(
        &prepared,
        &FormulaLimits::default(),
        &mut complete,
        location(&prepared),
    )
    .unwrap()
    .unwrap();
    assert!(complete.work > 1);
    for maximum in 0..complete.work {
        let limits = FormulaLimits {
            max_work: maximum,
            ..FormulaLimits::default()
        };
        let mut counters = Counters::default();
        let Err(error) =
            ProducerPlan::prepare(&prepared, &limits, &mut counters, location(&prepared))
        else {
            panic!("a proper charged prefix cannot publish this plan");
        };
        assert!(
            matches!(error, FormulaFailure::Limit { resource: FormulaResource::Work, observed, limit, .. } if observed > limit && limit == u128::from(maximum))
        );
        assert!(counters.work <= maximum);
    }
}

#[test]
fn producer_storage_is_admitted_before_reservation() {
    let prepared = prepare("p(1).r(X):-p(X).");
    let limits = FormulaLimits {
        max_support_bytes: size_of::<ProducerPlan<'_>>() - 1,
        ..FormulaLimits::default()
    };
    let mut counters = Counters::default();
    let Err(error) = ProducerPlan::prepare(&prepared, &limits, &mut counters, location(&prepared))
    else {
        panic!("the producer header alone exceeds this allowance");
    };
    assert!(
        matches!(error, FormulaFailure::Limit { resource: FormulaResource::SupportBytes, observed, limit, .. } if observed == size_of::<ProducerPlan<'_>>() as u128 && limit == limits.max_support_bytes as u128)
    );
    assert!(counters.work > 0);
}
