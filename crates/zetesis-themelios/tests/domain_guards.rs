//! Optional domains preserve complete theories while avoiding impossible prefixes.

use std::cell::{Cell, RefCell};
use std::fmt::Write;

use themelios_base::span::Location;
use zetesis_domain::Status;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, DomainLimits, DomainObservation, ExpansionFailure,
    ExpansionLimits, FormulaFailure, FormulaLimits, GroundingObserver, GroundingOptions,
    GroundingOutcome, GroundingPhase, GroundingWork, JoinStrategy, prepare_formula,
};

#[derive(Default)]
struct Observation {
    status: Cell<Option<Status>>,
    disabled: Cell<bool>,
    declined: Cell<bool>,
    work: Cell<GroundingWork>,
    support: Cell<GroundingWork>,
    rules: RefCell<Vec<GroundingWork>>,
}
impl GroundingObserver for Observation {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn domain_analysis(&self, observation: DomainObservation<'_, '_>) {
        match observation {
            DomainObservation::Disabled => self.disabled.set(true),
            DomainObservation::Inapplicable => self.declined.set(true),
            DomainObservation::Analyzed(analysis) => {
                assert!(analysis.belongs_to(analysis.program()));
                self.status.set(Some(analysis.status()));
            }
        }
    }
    fn phase_exit(
        &self,
        phase: GroundingPhase,
        _: Option<Location>,
        _: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.work.set(self.work.get().checked_sum(work));
        if phase == GroundingPhase::SupportCompletion {
            self.support.set(self.support.get().checked_sum(work));
        }
        if phase == GroundingPhase::RuleInstantiation {
            self.rules.borrow_mut().push(work);
        }
    }
}

fn ground(
    source: &str,
    strategy: JoinStrategy,
    domains: Option<DomainLimits>,
    observation: &Observation,
) -> Result<AdmittedFormula, FormulaFailure> {
    prepare_formula(
        source.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?
    .with_grounding_options(GroundingOptions { joins: strategy })
    .with_domain_analysis(domains)
    .ground_with_observer(Some(observation))
}

fn equal(left: &AdmittedFormula, right: &AdmittedFormula) {
    assert_eq!(left.atoms(), right.atoms());
    assert_eq!(left.theory().nodes(), right.theory().nodes());
    assert_eq!(left.theory().roots(), right.theory().roots());
    assert_eq!(left.formula_origins(), right.formula_origins());
    assert_eq!(
        left.objectives().templates(),
        right.objectives().templates()
    );
    assert_eq!(left.objective_origins(), right.objective_origins());
}

fn selective() -> String {
    let mut source = String::new();
    for value in 1..=8 {
        write!(source, "a({value}).b({value}).").unwrap();
    }
    for left in 5..=12 {
        for right in 5..=12 {
            write!(source, "c({left},{right}).").unwrap();
        }
    }
    source.push_str("r(X,Y):-a(X),b(Y),c(X,Y).");
    source
}

#[test]
fn finite_meets_avoid_real_prefixes_and_probes() {
    let source = selective();
    for strategy in [JoinStrategy::Indexed, JoinStrategy::Table] {
        let off = Observation::default();
        let on = Observation::default();
        let complete = ground(&source, strategy, None, &off).unwrap();
        let narrowed = ground(&source, strategy, Some(DomainLimits::default()), &on).unwrap();
        equal(&complete, &narrowed);
        let actual: Vec<_> = narrowed
            .atoms()
            .iter()
            .filter(|atom| atom.predicate().name() == "r")
            .map(|atom| atom.values().to_vec())
            .collect();
        let expected: Vec<_> = (5..=8)
            .flat_map(|left| {
                (5..=8).map(move |right| {
                    vec![
                        zetesis_core::Value::Number(left),
                        zetesis_core::Value::Number(right),
                    ]
                })
            })
            .collect();
        assert_eq!(actual, expected);
        assert!(off.disabled.get());
        assert_eq!(on.status.get(), Some(Status::FixedPoint));
        assert_eq!(on.support.get(), off.support.get());
        let before = *off.rules.borrow().last().unwrap();
        let after = *on.rules.borrow().last().unwrap();
        // Eight a/b values, but only four occur in the corresponding c columns.
        // Only 16 c probes have nonempty postings. Indexed offers one eight-row
        // posting per probe, then checks the full row; Table intersects both
        // equalities before offering its one row. A complete c match is not an
        // Indexed offered-row count. Its guard rejects four marginal rows per
        // posting; three other locally permitted rows still fail the matcher.
        let (c_rows, c_rejected) = match strategy {
            JoinStrategy::Indexed => (16 * 8, 16 * 4),
            JoinStrategy::Table => (16, 0),
        };
        assert_eq!(before.join_rows, Some(8 + 8 * 8 + c_rows));
        assert_eq!(after.join_rows, Some(8 + 4 * 8 + c_rows));
        assert_eq!(before.join_probes, Some(1 + 8 + 8 * 8));
        assert_eq!(after.join_probes, Some(1 + 4 + 4 * 4));
        assert_eq!(after.domain_rejected_rows, Some(4 + 4 * 4 + c_rejected));
        assert_eq!(after.domain_guard_rows, after.join_rows);
        assert_eq!(before.binding_snapshots, Some(16));
        assert_eq!(after.binding_snapshots, Some(16));
        assert!(after.domain_guard_checks.unwrap() > 0);
        assert!(on.work.get().domain_prepare_work.unwrap() > after.domain_prepare_work.unwrap());
        if strategy == JoinStrategy::Table {
            assert!(after.table_probes.unwrap() > 0);
        }
    }
}

#[test]
fn typed_aliases_cycles_and_provenance_survive() {
    for source in [
        "p(1).p(\"1\").p(a).q(\"1\").-q(1).r(X):-p(X),q(X).s(X):-p(X),-q(X).",
        "p(1,1).p(1,2).q(1).r(X):-p(X,X),q(X).r(X):-p(X,X),q(X).",
        "p(1).q(X):-p(X).p(X):-q(X).r(X,Y):-p(X),q(Y).",
        "p(X):-q(X).q(X):-p(X).r(X,Y):-p(X),q(Y).",
        "p(1,2).q(2,1).r(X,Y):-p(X,Y),q(X,Y).",
        "p(1).q(1).r(X):-p(X),q(X).",
    ] {
        let off = Observation::default();
        let on = Observation::default();
        equal(
            &ground(source, JoinStrategy::Indexed, None, &off).unwrap(),
            &ground(
                source,
                JoinStrategy::Indexed,
                Some(DomainLimits::default()),
                &on,
            )
            .unwrap(),
        );
        assert_eq!(on.status.get(), Some(Status::FixedPoint), "{source}");
        assert_eq!(on.support.get(), off.support.get());
    }
}

#[test]
fn widened_arguments_and_stopped_analysis_preserve_fallback() {
    let source = selective();
    let ordinary = ground(
        &source,
        JoinStrategy::Indexed,
        None,
        &Observation::default(),
    )
    .unwrap();
    for limits in [
        DomainLimits {
            max_values_per_argument: 0,
            ..DomainLimits::default()
        },
        DomainLimits {
            max_work: 7,
            ..DomainLimits::default()
        },
    ] {
        let observation = Observation::default();
        equal(
            &ordinary,
            &ground(&source, JoinStrategy::Indexed, Some(limits), &observation).unwrap(),
        );
        if limits.max_work == 7 {
            assert!(
                matches!(observation.status.get(), Some(Status::Stopped(stop))
                if stop.resource == zetesis_domain::Resource::Work && stop.limit == 7 && stop.observed == 8)
            );
            assert!(observation.work.get().domain_prepare_work.unwrap() >= 7);
        } else {
            assert_eq!(observation.status.get(), Some(Status::FixedPoint));
        }
        assert_eq!(observation.work.get().domain_guard_rows, Some(0));
        assert_eq!(observation.work.get().domain_rejected_rows, Some(0));
    }
}

#[test]
fn richer_source_keeps_the_complete_join() {
    for source in [
        "p(1).q(X):-p(X),not z(X).",
        "d(1).{p(X):d(X)}.",
        "p(f(1)).q(X):-p(X).",
        "p(1).q(X+1):-p(X).",
        "p(1).q(N):-N=#count{X:p(X)}.",
    ] {
        let off = Observation::default();
        let on = Observation::default();
        equal(
            &ground(source, JoinStrategy::Indexed, None, &off).unwrap(),
            &ground(
                source,
                JoinStrategy::Indexed,
                Some(DomainLimits::default()),
                &on,
            )
            .unwrap(),
        );
        assert!(on.declined.get(), "{source}");
        assert_eq!(on.work.get().domain_guard_rows, Some(0));
    }
}

#[test]
fn optional_analysis_never_suppresses_authored_arithmetic() {
    for source in ["d(0).p:-d(X),1=2,1/X=1.", "d(0).p:-d(X),1=2,not q(1/X)."] {
        for options in [None, Some(DomainLimits::default())] {
            assert!(
                matches!(
                    ground(
                        source,
                        JoinStrategy::Indexed,
                        options,
                        &Observation::default()
                    ),
                    Err(FormulaFailure::Expansion(
                        ExpansionFailure::Evaluation { .. }
                    ))
                ),
                "{source}"
            );
        }
    }
}
