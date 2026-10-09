//! Completed-source structural selection preserves original body consequences.

use super::*;
use crate::formula_support::testing::budget;
use crate::formula_support::{Accounting, CompletedQueries, Computation, Join, PreparedRule};
use crate::grounding_observer::Profile;
use crate::test_support::Observer;
use crate::{
    AdmissionOptions, ConstraintCheckLimits, ExpansionLimits, FormulaLimits, FormulaWarning,
    GroundingPhase, HybridFormula,
};
use zetesis_core::ValueNodeRef;

const SOURCE: &str = include_str!("../../../tests/fixtures/pattern-rows/partial-signed-body.lp");

fn preparation() -> crate::PreparedFormula {
    crate::prepare_formula(
        SOURCE.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}

fn unprepared_rows(
    rule: &crate::formula_ir::RuleIr,
    queries: &CompletedQueries<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    mut visit: impl FnMut(&Binding<'_>, bool, &Computation<'_, '_>, &mut Counters),
) {
    let mut computation = queries.computation(rule.location).unwrap();
    let mut rows = Join::rule(
        rule,
        queries.support(),
        &computation,
        limits,
        &mut budget(),
        counters,
    )
    .unwrap();
    while let Some(row) = rows
        .next_row(
            &mut computation,
            limits,
            &mut budget(),
            counters,
            rule.location,
        )
        .unwrap()
    {
        visit(&row.values, row.passes, &computation, counters);
    }
}

/// The same captured carrier supplies both traversals. Only the prepared
/// traversal can use its structural row selection; neither is given truth.
fn scan(
    owner: &HybridFormula,
    region: &Region,
    selected: bool,
) -> (Vec<ConstraintConsequence>, u64) {
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let mut counters = Counters::resume(Accounting::default(), profile.work());
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    assert_eq!(prepared.source.rules.len(), 1);
    let rule = &prepared.source.rules[0];
    let plan = selected.then(|| {
        PreparedRule::new(
            rule,
            &mut prepared.completed,
            &prepared.limits,
            &mut budget(),
            &mut counters,
        )
        .unwrap()
    });
    let queries = prepared
        .completed
        .queries(
            crate::JoinStrategy::Indexed,
            &prepared.limits,
            &counters,
            rule.location,
        )
        .unwrap();
    let outcomes = profile
        .phase(
            GroundingPhase::RuleInstantiation,
            Some(rule.location),
            || {
                let mut outcomes = Vec::new();
                let mut visit = |values: &Binding<'_>,
                                 passes: bool,
                                 computation: &Computation<'_, '_>,
                                 counters: &mut Counters| {
                    if passes {
                        outcomes.push(
                            body(
                                &rule.body,
                                values,
                                None,
                                region,
                                prepared.index.unwrap().lookup(),
                                None,
                                Context::new(
                                    computation,
                                    &prepared.limits,
                                    counters,
                                    rule.location,
                                ),
                            )
                            .unwrap(),
                        );
                    }
                };
                if let Some(plan) = &plan {
                    let mut computation = queries.computation(rule.location).unwrap();
                    let mut rows = plan
                        .rows(
                            &queries,
                            None,
                            &computation,
                            &prepared.limits,
                            &mut budget(),
                            &mut counters,
                        )
                        .unwrap();
                    while let Some(row) = rows
                        .next_row(
                            &mut computation,
                            &prepared.limits,
                            &mut budget(),
                            &mut counters,
                            rule.location,
                        )
                        .unwrap()
                    {
                        assert!(row.positives.is_none());
                        visit(&row.values, row.passes, &computation, &mut counters);
                    }
                } else {
                    unprepared_rows(rule, &queries, &prepared.limits, &mut counters, visit);
                }
                Ok::<_, FormulaFailure>(outcomes)
            },
        )
        .unwrap();
    (outcomes, observer.0.get().join_rows.unwrap())
}

fn relevant_atoms(owner: &HybridFormula) -> [usize; 3] {
    let mut relevant = [None; 3];
    for (index, atom) in owner.atom_catalog().atoms().iter().enumerate() {
        let slot = match atom.predicate().name() {
            "q" => Some(1),
            "r" => Some(2),
            "p" if atom.arguments().get(0).is_some_and(|value| {
                matches!(
                    value.descriptor(),
                    ValueNodeRef::Function {
                        name: "f",
                        arity: 1,
                        ..
                    }
                ) && value
                    .child(0)
                    .is_some_and(|child| child.descriptor() == ValueNodeRef::Number(1))
            }) =>
            {
                Some(0)
            }
            _ => None,
        };
        if let Some(slot) = slot {
            assert!(relevant[slot].replace(index).is_none());
        }
    }
    relevant.map(Option::unwrap)
}

#[test]
fn prepared_structures_preserve_partial_signed_bodies() {
    let owner = preparation().ground_hybrid().unwrap();
    let eager = preparation().ground().unwrap();
    assert!(matches!(
        owner.warnings(),
        [FormulaWarning::ZeroDivisor { .. }]
    ));
    assert_eq!(owner.warnings(), eager.warnings());
    let atoms = relevant_atoms(&owner);
    let mut refuted = false;
    let mut held = false;
    let mut cut = false;
    let mut unchanged = false;
    for encoding in 0..27 {
        let mut region = Region::all_open(owner.atom_catalog().atoms().len());
        let mut digits = encoding;
        for atom in atoms {
            match digits % 3 {
                1 => assert!(region.hold(atom)),
                2 => assert!(region.cut(atom)),
                _ => {}
            }
            digits /= 3;
        }
        let (selected, selected_rows) = scan(&owner, &region, true);
        let (reference, reference_rows) = scan(&owner, &region, false);
        assert_eq!(selected, reference, "signed region {encoding}");
        // This is route evidence: retaining the prepared order alone cannot
        // remove the unrelated p(other(...)) rows offered by the same source.
        assert!(
            selected_rows < reference_rows,
            "{selected_rows} versus {reference_rows}"
        );
        for consequence in selected {
            match consequence {
                ConstraintConsequence::Refuted { .. } => refuted = true,
                ConstraintConsequence::Hold { .. } => held = true,
                ConstraintConsequence::Cut { .. } => cut = true,
                ConstraintConsequence::NoConsequence => unchanged = true,
            }
        }
    }
    assert!(refuted && held && cut && unchanged);
    assert_eq!(owner.warnings(), eager.warnings());
}
