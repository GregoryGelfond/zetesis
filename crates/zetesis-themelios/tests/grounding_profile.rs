//! Public phase observation describes work without changing admitted formulas.

use std::cell::{Cell, RefCell};

use themelios_base::span::Location;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaFailure, FormulaLimits,
    FormulaResource, GroundingObserver, GroundingOutcome, GroundingPhase, GroundingWork,
    admit_formula_with_grounding_observer,
};

#[derive(Clone, Copy, Debug)]
struct Record {
    phase: GroundingPhase,
    location: Option<Location>,
    outcome: GroundingOutcome,
    work: GroundingWork,
}

#[derive(Default)]
struct Observer {
    active: Cell<bool>,
    phase: Cell<Option<(GroundingPhase, Option<Location>)>>,
    records: RefCell<Vec<Record>>,
}

impl GroundingObserver for Observer {
    fn enter(&self) {
        assert!(!self.active.replace(true));
    }
    fn exit(&self) {
        assert!(self.active.replace(false));
        assert!(self.phase.get().is_none());
    }
    fn details_enabled(&self) -> bool {
        assert!(self.active.get());
        true
    }
    fn phase_enter(&self, phase: GroundingPhase, location: Option<Location>) {
        assert!(self.active.get());
        assert!(self.phase.replace(Some((phase, location))).is_none());
    }
    fn phase_exit(
        &self,
        phase: GroundingPhase,
        location: Option<Location>,
        outcome: GroundingOutcome,
        work: GroundingWork,
    ) {
        assert_eq!(self.phase.take(), Some((phase, location)));
        self.records.borrow_mut().push(Record {
            phase,
            location,
            outcome,
            work,
        });
    }
}

fn compile(
    source: &str,
    limits: &FormulaLimits,
    observer: Option<&dyn GroundingObserver>,
) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula_with_grounding_observer(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        *limits,
        observer,
    )
}

#[test]
fn profile_preserves_the_compiled_subject() {
    let source = "digit(0..3). 1 {p(X):digit(X)} 1. :- p(X), X+1>2. #minimize{X:p(X)}. #show p/1.";
    let observer = Observer::default();
    let measured = compile(source, &FormulaLimits::default(), Some(&observer)).unwrap();
    let plain = compile(source, &FormulaLimits::default(), None).unwrap();
    assert_eq!(measured.atoms(), plain.atoms());
    assert_eq!(measured.theory().nodes(), plain.theory().nodes());
    assert_eq!(measured.theory().roots(), plain.theory().roots());
    assert_eq!(measured.formula_origins(), plain.formula_origins());
    assert_eq!(measured.objective_origins(), plain.objective_origins());
    assert_eq!(
        measured.objective_declarations(),
        plain.objective_declarations()
    );
    assert_eq!(
        measured.objectives().templates(),
        plain.objectives().templates()
    );
    assert!(!observer.active.get());
}

#[test]
fn phase_counts_reconcile_with_materialized_storage() {
    let observer = Observer::default();
    let admitted = compile(
        "p(1). 1 {q(X):p(X)} 1. -q(1).",
        &FormulaLimits::default(),
        Some(&observer),
    )
    .unwrap();
    let records = observer.records.borrow();
    let sum = |field: fn(GroundingWork) -> Option<u64>| {
        records
            .iter()
            .map(|record| field(record.work).unwrap())
            .sum::<u64>()
    };
    assert_eq!(
        sum(|work| work.atoms_inserted),
        admitted.atoms().len() as u64
    );
    assert_eq!(
        sum(|work| work.nodes_inserted),
        admitted.theory().nodes().len() as u64
    );
    assert_eq!(
        sum(|work| work.roots),
        admitted.theory().roots().len() as u64
    );
}

#[test]
fn support_counts_describe_completed_rounds() {
    let observer = Observer::default();
    compile("p(1).", &FormulaLimits::default(), Some(&observer)).unwrap();
    let records = observer.records.borrow();
    let support = records
        .iter()
        .find(|r| r.phase == GroundingPhase::SupportCompletion)
        .unwrap();
    assert_eq!(support.work.support_rounds, Some(2));
    assert_eq!(support.work.support_atoms, Some(1));
    assert_eq!(support.work.support_index_entries, Some(1));
}

#[test]
fn arithmetic_counts_describe_the_joined_rule() {
    let observer = Observer::default();
    compile(
        "p(1). :- p(X), X+1<3.",
        &FormulaLimits::default(),
        Some(&observer),
    )
    .unwrap();
    let records = observer.records.borrow();
    let arithmetic = records
        .iter()
        .find(|r| r.work.expression_nodes == Some(4))
        .unwrap();
    assert_eq!(arithmetic.phase, GroundingPhase::RuleInstantiation);
    assert_eq!(arithmetic.work.expression_evaluations, Some(2));
    assert_eq!(arithmetic.work.readiness_nodes, Some(4));
    assert_eq!(arithmetic.work.join_probes, Some(1));
    assert_eq!(arithmetic.work.join_rows, Some(1));
    assert_eq!(arithmetic.work.binding_snapshots, Some(1));
    assert_eq!(arithmetic.work.roots, Some(1));
}

#[test]
fn only_rule_phases_claim_a_source_location() {
    let observer = Observer::default();
    compile("p(1).", &FormulaLimits::default(), Some(&observer)).unwrap();
    let records = observer.records.borrow();
    assert_eq!(
        records.first().unwrap().phase,
        GroundingPhase::SupportCompletion
    );
    assert_eq!(
        records.last().unwrap().phase,
        GroundingPhase::TheoryValidation
    );
    for record in records.iter() {
        assert_eq!(
            record.location.is_some(),
            record.phase == GroundingPhase::RuleInstantiation
        );
        assert_eq!(record.outcome, GroundingOutcome::Completed);
    }
}

#[test]
fn undefined_arithmetic_retains_its_failed_phase() {
    let observer = Observer::default();
    let source = "p(0). :- p(X), 1/X=0.";
    let measured = compile(source, &FormulaLimits::default(), Some(&observer)).unwrap_err();
    let plain = compile(source, &FormulaLimits::default(), None).unwrap_err();
    assert_eq!(measured.to_string(), plain.to_string());
    let records = observer.records.borrow();
    let last = records.last().unwrap();
    assert_eq!(last.phase, GroundingPhase::RuleInstantiation);
    assert_eq!(last.outcome, GroundingOutcome::Failed);
    assert!(last.work.expression_nodes.unwrap() > 0);
    assert_eq!(last.work.roots, Some(0));
    assert!(!observer.active.get());
}

#[test]
fn profiling_preserves_inclusive_support_limits() {
    for (rounds, succeeds) in [(1, false), (2, true)] {
        let limits = FormulaLimits {
            max_support_rounds: rounds,
            ..FormulaLimits::default()
        };
        let observer = Observer::default();
        let measured = compile("p(1).", &limits, Some(&observer));
        let plain = compile("p(1).", &limits, None);
        assert_eq!(measured.is_ok(), succeeds);
        assert_eq!(plain.is_ok(), succeeds);
        if let Err(error) = measured {
            assert_eq!(error.to_string(), plain.unwrap_err().to_string());
            let records = observer.records.borrow();
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].outcome, GroundingOutcome::Failed);
            assert_eq!(records[0].work.support_rounds, Some(1));
            assert_eq!(records[0].work.support_atoms, Some(1));
        }
    }
}

#[test]
fn node_refusal_retains_attempted_interning() {
    let limits = FormulaLimits {
        theory: zetesis_ferraris::AdmissionLimits {
            max_nodes: 1,
            ..zetesis_ferraris::AdmissionLimits::default()
        },
        ..FormulaLimits::default()
    };
    let observer = Observer::default();
    let error = compile("p(1).", &limits, Some(&observer)).unwrap_err();
    assert_eq!(
        error.to_string(),
        compile("p(1).", &limits, None).unwrap_err().to_string()
    );
    let records = observer.records.borrow();
    let last = records.last().unwrap();
    assert_eq!(last.phase, GroundingPhase::FormulaInitialization);
    assert_eq!(last.outcome, GroundingOutcome::Failed);
    assert_eq!(last.work.node_lookups, Some(2));
    assert_eq!(last.work.nodes_inserted, Some(1));
}

fn root_refusal(source: &str, roots: usize, phase: GroundingPhase) {
    let limits = FormulaLimits {
        theory: zetesis_ferraris::AdmissionLimits {
            max_roots: roots,
            ..zetesis_ferraris::AdmissionLimits::default()
        },
        ..FormulaLimits::default()
    };
    let observer = Observer::default();
    let error = compile(source, &limits, Some(&observer)).unwrap_err();
    assert!(matches!(
        error,
        FormulaFailure::Limit {
            resource: FormulaResource::Roots,
            observed,
            limit,
            ..
        } if observed == roots as u128 + 1 && limit == roots as u128
    ));
    let records = observer.records.borrow();
    let last = records.last().unwrap();
    assert_eq!(last.phase, phase);
    assert_eq!(last.outcome, GroundingOutcome::Failed);
    assert!(
        records[..records.len() - 1]
            .iter()
            .all(|record| record.outcome == GroundingOutcome::Completed)
    );
    assert!(!observer.active.get());
}

#[test]
fn coherence_preserves_the_cumulative_root_limit() {
    // Both source facts consume the allowance before their coherence constraint.
    root_refusal("p. -p.", 2, GroundingPhase::Coherence);
}

#[test]
fn support_guards_preserve_the_cumulative_root_limit() {
    // A fact consumes the allowance before its necessary-support guard.
    root_refusal("p.", 1, GroundingPhase::SupportGuards);
}

#[test]
fn source_refusals_have_no_grounding_phases() {
    for source in ["p(.", "p(X)."] {
        let observer = Observer::default();
        assert!(compile(source, &FormulaLimits::default(), Some(&observer)).is_err());
        assert!(observer.records.borrow().is_empty());
        assert!(!observer.active.get());
    }
}

#[test]
fn bundle_rule_locations_identify_retained_sources() {
    use zetesis_themelios::{
        BundleAdmissionOptions, BundleLimits, SourceBundle,
        admit_bundle_formula_with_grounding_observer,
    };

    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/reachability.lp");
    let bundle = SourceBundle::load(path, BundleLimits::default()).unwrap();
    let observer = Observer::default();
    let admitted = admit_bundle_formula_with_grounding_observer(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
        Some(&observer),
    )
    .unwrap();
    let records = observer.records.borrow();
    let locations: Vec<_> = records
        .iter()
        .filter_map(|record| record.location)
        .collect();
    assert_eq!(locations.len(), 4);
    for location in locations {
        assert!(admitted.bundle().get(location.source).is_some());
        assert!(
            admitted
                .formula_origins()
                .iter()
                .any(|origins| origins.contains(&location))
        );
    }
}

#[test]
fn work_aggregation_preserves_field_availability() {
    let mut left = GroundingWork::default();
    left.join_rows = Some(u64::MAX);
    left.expression_nodes = None;
    left.roots = Some(3);
    let mut right = GroundingWork::default();
    right.join_rows = Some(1);
    right.expression_nodes = Some(5);
    right.roots = Some(2);
    let sum = left.checked_sum(right);
    assert_eq!(sum.join_rows, None);
    assert_eq!(sum.expression_nodes, None);
    assert_eq!(sum.roots, Some(5));
    assert_eq!(sum.atoms_inserted, Some(0));
}

#[test]
fn phase_labels_form_a_unique_complete_catalog() {
    let labels: std::collections::BTreeSet<_> = GroundingPhase::ALL
        .into_iter()
        .map(GroundingPhase::label)
        .collect();
    assert_eq!(labels.len(), GroundingPhase::ALL.len());
    assert_eq!(
        labels,
        [
            "support_completion",
            "objective_activation",
            "formula_initialization",
            "rule_instantiation",
            "coherence",
            "support_guards",
            "theory_validation"
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn outcome_labels_form_a_unique_complete_catalog() {
    let labels: std::collections::BTreeSet<_> = GroundingOutcome::ALL
        .into_iter()
        .map(GroundingOutcome::label)
        .collect();
    assert_eq!(labels.len(), GroundingOutcome::ALL.len());
    assert_eq!(
        labels,
        ["completed", "failed", "unwound"].into_iter().collect()
    );
}
