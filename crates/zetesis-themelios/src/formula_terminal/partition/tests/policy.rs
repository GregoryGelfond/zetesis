//! Physical selection refines the mathematical certificate without changing it.

use super::{Partition, partition, prepare};
use crate::formula_support::Counters;
use crate::formula_terminal::partition::{policy, selection, workspace::Context};
use crate::{FormulaFailure, FormulaResource};

#[path = "policy_refusal.rs"]
mod refusal;

#[test]
fn ground_only_policy_preserves_the_preparation_owner() {
    let mut prepared = prepare("{seed}. d:-seed. hidden(7).");
    let rules = prepared.program.rules.as_ptr();
    let carriers: Vec<_> = prepared
        .program
        .analyzed
        .statements()
        .map(std::ptr::from_ref)
        .collect();
    let expansion = prepared.budget.usage();
    let work = prepared.accounting.work;
    let counters = Counters::resume(
        std::mem::take(&mut prepared.accounting),
        crate::grounding_observer::Work::default(),
    );
    let workspace = counters.workspace_bytes();
    prepared.accounting = counters.into_accounting();
    let Partition {
        base,
        terminal: None,
    } = partition(prepared).unwrap()
    else {
        panic!("ground-only definitions remain ordinary rules")
    };
    assert_eq!(base.program.rules.as_ptr(), rules);
    assert_eq!(
        base.program
            .analyzed
            .statements()
            .map(std::ptr::from_ref)
            .collect::<Vec<_>>(),
        carriers
    );
    assert_eq!(base.budget.usage(), expansion);
    assert!(base.accounting.work > work);
    let counters = Counters::resume(base.accounting, crate::grounding_observer::Work::default());
    assert_eq!(counters.workspace_bytes(), workspace);
}

#[test]
fn variable_producers_defer_all_facts_of_their_signature() {
    let prepared = prepare("p(1). q(2). d(2;3). d(X):-p(X). d(X):-q(X).");
    let Partition {
        base,
        terminal: Some(definitions),
    } = partition(prepared).unwrap()
    else {
        panic!("variable-bearing signature")
    };
    assert_eq!(definitions.deferred.len(), 4);
    assert_eq!(
        definitions
            .deferred
            .iter()
            .filter(|rule| rule.variables == 0)
            .count(),
        2
    );
    assert_eq!(base.program.rules.len(), 2);
    let names: Vec<_> = base
        .program
        .analyzed
        .statements()
        .map(|source| {
            super::super::matching::head(super::super::reads::source(source))
                .unwrap()
                .name
                .as_str()
        })
        .collect();
    assert_eq!(names, ["p", "q"]);
}

#[test]
fn different_arities_keep_independent_materialization_policies() {
    let prepared = prepare("p(1). d(7,8). d(2). d(X):-p(X).");
    let Partition {
        base,
        terminal: Some(definitions),
    } = partition(prepared).unwrap()
    else {
        panic!("only d/1 has a variable-bearing producer")
    };
    assert_eq!(definitions.deferred.len(), 2);
    assert_eq!(base.program.rules.len(), 2);
    let expected = prepare("p(1). d(7,8).");
    assert_eq!(base.program.analysis, expected.program.analysis);
    assert_eq!(
        base.program.analysis,
        themelios_analysis::Analysis::of(&base.program.analyzed)
    );
    assert_ne!(base.program.analysis, definitions.original.analysis);
    let terminal = zetesis_domain::terminal::analyze(
        &base.program.analyzed,
        zetesis_domain::Limits::default(),
    );
    // Both remaining facts are still mathematically terminal. Reanalysis must
    // describe the retained ground rules, not the original d/1 definition.
    assert_eq!(terminal.definitions().len(), 2);
}

#[test]
fn body_only_bindings_are_deferred() {
    for source in ["p(1). d:-p(X).", "p(1). d:-p(_)."] {
        let Partition {
            terminal: Some(definitions),
            ..
        } = partition(prepare(source)).unwrap()
        else {
            panic!("{source}")
        };
        assert_eq!(definitions.deferred.len(), 1);
        assert_eq!(definitions.deferred[0].variables, 1);
    }
}

#[test]
fn policy_refusals_preserve_the_resource_boundary() {
    for resource in [FormulaResource::Work, FormulaResource::SupportBytes] {
        let mut prepared = prepare("p(1). d(2). d(X):-p(X).");
        let analysis = zetesis_domain::terminal::analyze(
            &prepared.program.analyzed,
            zetesis_domain::Limits::default(),
        );
        let mut counters = Counters::resume(
            std::mem::take(&mut prepared.accounting),
            crate::grounding_observer::Work::default(),
        );
        let admission = prepared
            .catalog
            .component_admission(&prepared.limits, &mut counters, prepared.location)
            .unwrap();
        let components = admission
            .components(&prepared.limits, &mut counters, prepared.location)
            .unwrap();
        let mut context = Context {
            admission: &admission,
            components,
            limits: &prepared.limits,
            counters: &mut counters,
            location: prepared.location,
        };
        let mut selected =
            selection::select(&prepared.program, analysis.definitions(), &mut context)
                .unwrap()
                .expect("complete source/IR certificate precedes the policy");
        let workspace = context.counters.workspace_bytes();
        let work = context.counters.accounting.work;
        let mut limits = prepared.limits;
        let limit = match resource {
            FormulaResource::Work => {
                limits.max_work = work;
                u128::from(work)
            }
            FormulaResource::SupportBytes => {
                limits.max_support_bytes = 0;
                0
            }
            _ => unreachable!("two policy resource boundaries"),
        };
        context.limits = &limits;
        let result = policy::select(
            &prepared.program,
            analysis.definitions(),
            &mut selected.values,
            &mut context,
        );
        assert!(matches!(result, Err(FormulaFailure::Limit {
            resource: actual, observed, limit: actual_limit, ..
        }) if actual == resource && actual_limit == limit && observed > limit));
        assert_eq!(context.counters.accounting.work, work);
        assert_eq!(context.counters.workspace_bytes(), workspace);
    }
}
