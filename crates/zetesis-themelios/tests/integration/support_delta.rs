//! Delta support coverage agrees with complete finite source substitutions.

use crate::support::source_oracle;
use crate::support::source_records;

use std::cell::Cell;

use themelios_base::span::Location;
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource,
    GroundingObserver, GroundingOutcome, GroundingPhase, GroundingWork,
    admit_formula_with_grounding_observer,
};

const CASES: &[(&str, &str)] = &[
    (
        "p(0). q(0). p(1):-q(0). q(1):-p(0). r(X,Y):-p(X),q(Y).",
        "p(0..1). q(0..1). r(0,0). r(0,1). r(1,0). r(1,1).",
    ),
    (
        "p(0). p(1):-p(0). r(X,Y):-p(X),p(Y).",
        "p(0..1). r(0,0). r(0,1). r(1,0). r(1,1).",
    ),
    (
        "p(0..2). q(0). q(1):-q(0). r(X,Y):-p(X),q(Y).",
        "p(0..2). q(0..1). r(0,0). r(0,1). r(1,0). r(1,1). r(2,0). r(2,1).",
    ),
    ("d(0). d(1):-d(0). {p(X):d(X)}.", "d(0..1). {p(0);p(1)}."),
    (
        "p(0). q(0):-not nope. p(1):-q(0). r(X):-p(X),not z(X).",
        "p(0..1). q(0). r(0..1).",
    ),
    (
        "d(0). d(1):-d(0). p(N):-N=#count{X:d(X)}.",
        "d(0..1). p(2).",
    ),
    (
        "-p(0). -p(1):--p(0). q(X,Y):--p(X),-p(Y).",
        "-p(0..1). q(0,0). q(0,1). q(1,0). q(1,1).",
    ),
    (
        "p(0). p(1):-p(0). blocked(1). picked(0). r(X,Y):-not blocked(X),p(X),not not picked(Y),p(Y).",
        "p(0). p(1). blocked(1). picked(0). r(0,0).",
    ),
    ("p:-not q. q:-not p.", "1{p;q}1."),
    ("p:-not not p.", "{p}."),
    (
        "p(1). p(\"1\"):-p(1). blocked(1). -q(X):-p(X),not blocked(X).",
        "p(1). p(\"1\"). blocked(1). -q(\"1\").",
    ),
];

#[test]
fn delta_support_matches_complete_finite_substitutions() {
    for &(source, expanded) in CASES {
        let admitted = source_records::admit(source, &FormulaLimits::default()).unwrap();
        let expanded = source_records::admit(expanded, &FormulaLimits::default()).unwrap();
        assert_eq!(
            source_records::exhaustive(&admitted),
            source_records::exhaustive(&expanded),
            "{source}"
        );
    }
}

#[test]
fn delta_completion_excludes_what_a_false_comparison_excludes() {
    // p already has support before the later positive row d(0) appears.
    // 1=2 is defined and false over every substitution of the rule, so the
    // division over X is never reached, in the first round or the delta
    // round, and the rule adds nothing to the program without it.
    let expanded =
        source_records::admit("p. d(1). d(0):-d(1).", &FormulaLimits::default()).unwrap();
    for source in [
        "p. d(1). d(0):-d(1). p:-d(X),1=2,1/X=1.",
        "p. d(1). d(0):-d(1). p:-d(X),1/X=1,1=2.",
        "p. d(1). d(0):-d(1). p:-d(X),1=2,not q(1/X).",
        "p. d(1). d(0):-d(1). p:-d(X),not not q(1/X),1=2.",
    ] {
        let admitted = source_records::admit(source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        assert_eq!(
            source_records::exhaustive(&admitted),
            source_records::exhaustive(&expanded),
            "{source}"
        );
    }
}

#[test]
fn support_completion_requires_the_final_empty_round() {
    let source = "p(0). p(X+1):-p(X),X<3,not stop(X).";
    let exact = FormulaLimits {
        max_support_rounds: 5,
        ..Default::default()
    };
    assert!(source_records::admit(source, &exact).is_ok());
    let short = FormulaLimits {
        max_support_rounds: 4,
        ..exact
    };
    assert!(matches!(
        source_records::admit(source, &short),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportRounds,
            observed: 5,
            limit: 4,
            ..
        })
    ));
}

#[derive(Default)]
struct SupportObservation {
    work: Cell<Option<GroundingWork>>,
    outcome: Cell<Option<GroundingOutcome>>,
}

impl GroundingObserver for SupportObservation {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        phase: GroundingPhase,
        _: Option<Location>,
        outcome: GroundingOutcome,
        work: GroundingWork,
    ) {
        if phase == GroundingPhase::SupportCompletion {
            self.work
                .set(Some(self.work.get().unwrap_or_default().checked_sum(work)));
            self.outcome.set(Some(outcome));
        }
    }
}

fn observed(
    source: &str,
    limits: &FormulaLimits,
    observation: &SupportObservation,
) -> Result<zetesis_themelios::AdmittedFormula, FormulaFailure> {
    admit_formula_with_grounding_observer(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        *limits,
        Some(observation),
    )
}

#[test]
fn negative_noninputs_preserve_positive_delta_work() {
    let positive = SupportObservation::default();
    let negative = SupportObservation::default();
    let prefix = "p(0). p(1):-p(0). blocked(1). picked(0).";
    let full = observed(
        &format!("{prefix}r(X,Y):-p(X),p(Y)."),
        &FormulaLimits::default(),
        &positive,
    )
    .unwrap();
    let gated = observed(
        &format!("{prefix}r(X,Y):-not blocked(X),p(X),not not picked(Y),p(Y)."),
        &FormulaLimits::default(),
        &negative,
    )
    .unwrap();
    assert_eq!(negative.outcome.get(), Some(GroundingOutcome::Completed));
    let positive = positive.work.get().unwrap();
    let negative = negative.work.get().unwrap();
    assert!(positive.join_rows.unwrap() > 0);
    assert_eq!(negative.support_rounds, positive.support_rounds);
    assert_eq!(negative.support_atoms, positive.support_atoms);
    assert_eq!(negative.join_probes, positive.join_probes);
    assert_eq!(negative.join_rows, positive.join_rows);
    assert_eq!(negative.binding_snapshots, positive.binding_snapshots);
    // The same possible support does not mean the original formulas agree.
    assert_ne!(
        source_records::exhaustive(&gated),
        source_records::exhaustive(&full)
    );
}

#[test]
fn negative_delta_retains_duplicate_source_origins() {
    let authored = "r(X):-p(X),not blocked(X).";
    let source = format!("p(0). p(1):-p(0). {authored} {authored}");
    let admitted = source_records::admit(&source, &FormulaLimits::default()).unwrap();
    let starts: std::collections::BTreeSet<_> = admitted
        .formula_origins()
        .iter()
        .flatten()
        .filter_map(|location| {
            let start = usize::try_from(location.span.start().get()).unwrap();
            let end = usize::try_from(location.span.end().get()).unwrap();
            (&source[start..end] == authored).then_some(start)
        })
        .collect();
    assert_eq!(starts.len(), 2);
    let expected =
        source_records::admit("p(0).p(1).r(0).r(1).", &FormulaLimits::default()).unwrap();
    assert_eq!(
        source_records::exhaustive(&admitted),
        source_records::exhaustive(&expected)
    );
}

#[test]
fn negative_delta_cannot_publish_a_refused_support_prefix() {
    let observation = SupportObservation::default();
    let result = observed(
        "p(0).p(1):-p(0).r(X,Y):-p(X),not blocked(X),p(Y).",
        &FormulaLimits {
            theory: zetesis_ferraris::AdmissionLimits {
                max_atoms: 3,
                ..zetesis_ferraris::AdmissionLimits::default()
            },
            ..FormulaLimits::default()
        },
        &observation,
    );
    assert!(matches!(
        result,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Atoms,
            observed: 4,
            limit: 3,
            ..
        })
    ));
    assert_eq!(observation.outcome.get(), Some(GroundingOutcome::Failed));
    assert!(observation.work.get().unwrap().support_atoms.unwrap() > 0);
}

#[test]
#[ignore = "requires independently installed clingo"]
fn delta_sources_match_clingo_complete_models() {
    for &(source, _) in CASES {
        let admitted = source_records::admit(source, &FormulaLimits::default()).unwrap();
        assert_eq!(
            source_records::exhaustive(&admitted),
            source_oracle::records(source),
            "{source}"
        );
    }
}
