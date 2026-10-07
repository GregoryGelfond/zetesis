//! Optional domains preserve complete theories while avoiding impossible prefixes.

use std::cell::{Cell, RefCell};
use std::fmt::Write;

use zetesis_domain::Status;
use zetesis_themelios::ProgramSite;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, DomainLimits, DomainObservation, ExpansionFailure,
    ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource, GroundingObserver,
    GroundingOptions, GroundingOutcome, GroundingPhase, GroundingWork, JoinStrategy,
    prepare_formula,
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
        _: Option<ProgramSite>,
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
    assert_eq!(
        (left.theory().nodes(), left.theory().operands()),
        (right.theory().nodes(), right.theory().operands())
    );
    assert_eq!(left.theory().roots(), right.theory().roots());
    assert_eq!(left.formula_origins(), right.formula_origins());
    assert_eq!(
        left.objectives().templates().iter().collect::<Vec<_>>(),
        right.objectives().templates().iter().collect::<Vec<_>>()
    );
    assert_eq!(left.objective_origins(), right.objective_origins());
    assert_eq!(
        left.objectives().priorities(),
        right.objectives().priorities()
    );
    assert_eq!(left.metadata(), right.metadata());
    assert_eq!(left.projection(), right.projection());
    assert_eq!(left.warnings(), right.warnings());
}

fn selective() -> String {
    // The shared constant selects a real, maximally skewed posting in a/b.
    // Both remain preferred to the larger c relation, so domain meets must
    // remove impossible prefixes independently of connected join ordering.
    selective_with_key(true)
}

fn selective_with_key(shared_key: bool) -> String {
    let key = if shared_key { "0," } else { "" };
    let mut source = String::new();
    for value in 1..=8 {
        write!(source, "a({key}{value}).b({key}{value}).").unwrap();
    }
    for left in 5..=12 {
        for right in 5..=12 {
            write!(source, "c({left},{right}).").unwrap();
        }
    }
    write!(source, "r(X,Y):-a({key}X),b({key}Y),c(X,Y).").unwrap();
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
            .map(|atom| atom.values().iter().collect::<Vec<_>>())
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
        // Completion reads the same narrowed rows as the final instantiation.
        assert!(on.support.get().domain_rejected_rows.unwrap() > 0);
        assert!(on.support.get().join_rows < off.support.get().join_rows);
        let before = *off.rules.borrow().last().unwrap();
        let after = *on.rules.borrow().last().unwrap();
        // The constant-key postings retain the a, b, c order: eight a/b
        // values, but only four occur in the corresponding c columns.
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
        // Complete rows are lent to formula emission, with the same 16 roots.
        assert_eq!(before.binding_snapshots, Some(0));
        assert_eq!(after.binding_snapshots, Some(0));
        assert_eq!(before.roots, Some(16));
        assert_eq!(after.roots, Some(16));
        assert!(after.domain_guard_checks.unwrap() > 0);
        assert!(on.work.get().domain_prepare_work.unwrap() > after.domain_prepare_work.unwrap());
        if strategy == JoinStrategy::Table {
            assert!(after.table_probes.unwrap() > 0);
        }
    }
}

#[test]
fn finite_meets_avoid_empty_connected_probes() {
    let source = selective_with_key(false);
    for strategy in [JoinStrategy::Indexed, JoinStrategy::Table] {
        let off = Observation::default();
        let on = Observation::default();
        let complete = ground(&source, strategy, None, &off).unwrap();
        let narrowed = ground(&source, strategy, Some(DomainLimits::default()), &on).unwrap();
        equal(&complete, &narrowed);
        assert!(off.disabled.get());
        assert_eq!(on.status.get(), Some(Status::FixedPoint));
        let before = *off.rules.borrow().last().unwrap();
        let after = *on.rules.borrow().last().unwrap();
        // a, c, b: four a values have empty c postings; each remaining c
        // posting has four Y values with empty b postings. Guards skip those
        // probes, which would offer no rows under either physical strategy.
        assert_eq!(before.join_rows, Some(8 + 4 * 8 + 4 * 4));
        assert_eq!(after.join_rows, before.join_rows);
        assert_eq!(before.join_probes, Some(1 + 8 + 4 * 8));
        assert_eq!(after.join_probes, Some(1 + 4 + 4 * 4));
        assert_eq!(after.domain_rejected_rows, Some(4 + 4 * 4));
        assert_eq!(before.roots, Some(16));
        assert_eq!(after.roots, Some(16));
        assert!(after.domain_guard_checks.unwrap() > 0);
        if strategy == JoinStrategy::Table {
            assert!(after.table_probes.unwrap() > 0);
        }
    }
}

#[test]
fn eligible_domain_guards_preserve_complete_grounding() {
    // The same complete-theory and provenance contract covers typed and signed
    // values, aliases and repeated rule occurrences, seeded and unseeded cycles,
    // swapped columns, and equal argument domains.
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
        assert!(on.support.get().join_rows <= off.support.get().join_rows);
    }
}

#[test]
fn producer_meets_prune_downstream_empty_joins() {
    // The producer of p bounds X by a intersect b. Propagating that bound
    // makes q's domain empty before it probes p, rather than rediscovering
    // the disjoint domains for each offered c row.
    let source = include_str!("../fixtures/domain-producer-meets.lp");
    for strategy in [JoinStrategy::Indexed, JoinStrategy::Table] {
        let off = Observation::default();
        let on = Observation::default();
        let complete = ground(source, strategy, None, &off).unwrap();
        let narrowed = ground(source, strategy, Some(DomainLimits::default()), &on).unwrap();
        equal(&complete, &narrowed);
        assert_eq!(on.status.get(), Some(Status::FixedPoint));
        assert!(
            narrowed
                .atoms()
                .iter()
                .all(|atom| atom.predicate().name() != "q")
        );
        let before = *off.rules.borrow().last().unwrap();
        let after = *on.rules.borrow().last().unwrap();
        assert!(after.domain_rejected_rows.unwrap() > 0);
        assert!(after.join_probes < before.join_probes);
        assert_eq!(after.roots, Some(0));
    }
}

#[test]
fn completion_reads_the_narrowed_rows() {
    // The candidates are prepared once, with the analysis; the first
    // completion round rejects the 198 rows X < 3 excludes, and the delta
    // round offers d/1 no new row.
    let source = "d(1..200). p(X) :- d(X), X < 3. q(X) :- p(X).";
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
    let (before, after) = (off.support.get(), on.support.get());
    assert_eq!(after.domain_rejected_rows, Some(198));
    assert_eq!(after.join_rows, before.join_rows);
    // Without the analysis X < 3 is evaluated on both sides of every offered
    // row; with it, on the two rows the guard admits.
    assert_eq!(before.expression_evaluations, Some(400));
    assert_eq!(after.expression_evaluations, Some(4));
    assert_eq!(on.work.get().domain_excluded_values, Some(198));
}

#[test]
fn widened_arguments_leave_grounding_unrestricted() {
    let source = selective();
    let ordinary = ground(
        &source,
        JoinStrategy::Indexed,
        None,
        &Observation::default(),
    )
    .unwrap();
    let limits = DomainLimits {
        max_values_per_argument: 0,
        ..DomainLimits::default()
    };
    let observation = Observation::default();
    equal(
        &ordinary,
        &ground(&source, JoinStrategy::Indexed, Some(limits), &observation).unwrap(),
    );
    assert_eq!(observation.status.get(), Some(Status::FixedPoint));
    assert_eq!(observation.work.get().domain_guard_rows, Some(0));
    assert_eq!(observation.work.get().domain_rejected_rows, Some(0));
}

#[test]
fn stopped_domain_analysis_preserves_complete_fallback() {
    let source = selective();
    let ordinary = ground(
        &source,
        JoinStrategy::Indexed,
        None,
        &Observation::default(),
    )
    .unwrap();
    let limits = DomainLimits {
        max_work: 7,
        ..DomainLimits::default()
    };
    let observation = Observation::default();
    equal(
        &ordinary,
        &ground(&source, JoinStrategy::Indexed, Some(limits), &observation).unwrap(),
    );
    assert!(
        matches!(observation.status.get(), Some(Status::Stopped(stop))
        if stop.resource == zetesis_domain::Resource::Work && stop.limit == 7 && stop.observed == 8)
    );
    assert!(observation.work.get().domain_prepare_work.unwrap() >= 7);
    assert_eq!(observation.work.get().domain_guard_rows, Some(0));
    assert_eq!(observation.work.get().domain_rejected_rows, Some(0));
}

#[test]
fn richer_source_keeps_the_complete_join() {
    for source in [
        "p(1).q(X):-p(X),not z(X).",
        "d(1).{p(X):d(X)}.",
        "p(f(1)).q(X):-p(X).",
        "p(1).q(X+1):-p(X).",
        "p(1).q(N):-N=#count{X:p(X)}.",
        "d(1).{p(X):d(X)}.#minimize{X:p(X)}.",
        "d(1).p(X):-d(X),not absent(X).#minimize{X:p(X)}.",
        "d(1).p(X):-d(X).#minimize{1:p((1;2))}.",
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
fn optional_analysis_changes_no_arithmetic_verdict() {
    // 1 = 2 excludes the only substitution, so nothing in it is reached with
    // or without the analysis; a reached operation refuses either way.
    for source in ["d(0).p:-d(X),1=2,1/X=1.", "d(0).p:-d(X),1=2,not q(1/X)."] {
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
    }
    for options in [None, Some(DomainLimits::default())] {
        assert!(matches!(
            ground(
                "d(0).p:-d(X),1/X=1.",
                JoinStrategy::Indexed,
                options,
                &Observation::default()
            ),
            Err(FormulaFailure::Expansion(
                ExpansionFailure::Evaluation { .. }
            ))
        ));
    }
}

#[test]
fn a_comparison_over_one_variable_narrows_its_candidates() {
    // X < 3 is defined and false at 198 of the 200 candidates the domain of
    // d/1 offers, so those rows are rejected before binding; the theory is
    // the one the complete join grounds.
    let source = "d(1..200). p(X) :- d(X), X < 3.";
    let off = Observation::default();
    let on = Observation::default();
    let complete = ground(source, JoinStrategy::Indexed, None, &off).unwrap();
    let narrowed = ground(
        source,
        JoinStrategy::Indexed,
        Some(DomainLimits::default()),
        &on,
    )
    .unwrap();
    equal(&complete, &narrowed);
    assert_eq!(
        narrowed
            .atoms()
            .iter()
            .filter(|atom| atom.predicate().name() == "p")
            .count(),
        2
    );
    assert_eq!(on.work.get().domain_excluded_values, Some(198));
    let rules = *on.rules.borrow().last().unwrap();
    assert_eq!(rules.domain_rejected_rows, Some(198));
    assert_eq!(rules.binding_snapshots, Some(0));
    assert_eq!(rules.roots, Some(2));
}

#[test]
fn domain_narrowing_preserves_undefined_family_evidence() {
    // 1/X = 1 is undefined at X = 0 and false at X = 2, so the analysis
    // excludes 2 alone; X != 0 then excludes 0. The same substitution is
    // reached, and omitted with a warning, when no comparison excludes it.
    let source = "d(0..2). p(X) :- d(X), 1/X = 1, X != 0.";
    let on = Observation::default();
    let narrowed = ground(
        source,
        JoinStrategy::Indexed,
        Some(DomainLimits::default()),
        &on,
    )
    .unwrap();
    equal(
        &ground(source, JoinStrategy::Indexed, None, &Observation::default()).unwrap(),
        &narrowed,
    );
    assert_eq!(on.work.get().domain_excluded_values, Some(2));
    assert!(narrowed.warnings().is_empty());
    let mixed = ground(
        "d(0..2). p(X) :- d(X), 1/X = 1.",
        JoinStrategy::Indexed,
        Some(DomainLimits::default()),
        &Observation::default(),
    )
    .unwrap();
    assert_eq!(mixed.warnings().len(), 1);
    // Different source spans are expected; emitted atoms and formulas agree.
    assert_eq!(mixed.atoms(), narrowed.atoms());
    assert_eq!(
        (mixed.theory().nodes(), mixed.theory().operands()),
        (narrowed.theory().nodes(), narrowed.theory().operands())
    );
    assert_eq!(mixed.theory().roots(), narrowed.theory().roots());
}

#[test]
fn a_comparison_over_two_variables_narrows_no_candidate() {
    // X < Y reads two variables; it is decided in the join, where the
    // exclusion rule already prunes it, and no candidate of either is
    // excluded on its own.
    let source = "d(1..3). p(X,Y) :- d(X), d(Y), X < Y.";
    let on = Observation::default();
    let narrowed = ground(
        source,
        JoinStrategy::Indexed,
        Some(DomainLimits::default()),
        &on,
    )
    .unwrap();
    equal(
        &ground(source, JoinStrategy::Indexed, None, &Observation::default()).unwrap(),
        &narrowed,
    );
    assert_eq!(on.work.get().domain_excluded_values, Some(0));
    assert_eq!(on.work.get().domain_guard_rows, Some(0));
}

#[test]
fn a_domain_that_admits_every_row_restricts_nothing() {
    // The domain of d/1 is what d/1 offers, so a guard over it could reject
    // no row and none is prepared.
    let source = "d(1..8). e(1..8). p(X,Y) :- d(X), e(Y).";
    let on = Observation::default();
    let narrowed = ground(
        source,
        JoinStrategy::Indexed,
        Some(DomainLimits::default()),
        &on,
    )
    .unwrap();
    equal(
        &ground(source, JoinStrategy::Indexed, None, &Observation::default()).unwrap(),
        &narrowed,
    );
    assert_eq!(on.status.get(), Some(Status::FixedPoint));
    let rules = *on.rules.borrow().last().unwrap();
    assert_eq!(rules.domain_guard_rows, Some(0));
    assert_eq!(rules.domain_guard_checks, Some(0));
}

/// Admit the fact `p(1).` within `max_work` units of formula work, with the
/// optional domain analysis when `domains` is given.
fn fact(max_work: u64, domains: Option<DomainLimits>) -> Result<AdmittedFormula, FormulaFailure> {
    prepare_formula(
        "p(1).".to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_work,
            ..FormulaLimits::default()
        },
    )?
    .with_domain_analysis(domains)
    .ground()
}

/// A typed refusal at the work ceiling `limit`, having observed more work.
fn exceeds_work(failure: &FormulaFailure, ceiling: u64) -> bool {
    matches!(failure, FormulaFailure::Limit {
        resource: FormulaResource::Work, limit, observed, ..
    } if *limit == u128::from(ceiling) && observed > limit)
}

// Domain analysis is charged as formula work: at the least work that admits
// the fact without it, admission with it is refused by the typed work limit.
// The boundary is found by bisection; only typed work refusals move its lower
// end, so no fixed implementation cost is assumed.
#[test]
fn domain_analysis_is_charged_beyond_the_plain_admission_boundary() {
    let mut upper = FormulaLimits::default().max_work;
    assert!(fact(upper, None).is_ok());
    let mut lower = 0;
    while lower + 1 < upper {
        let middle = lower + (upper - lower) / 2;
        match fact(middle, None) {
            Ok(_) => upper = middle,
            Err(failure) => {
                assert!(exceeds_work(&failure, middle), "{failure:?}");
                lower = middle;
            }
        }
    }
    let failure = fact(upper, Some(DomainLimits::default())).unwrap_err();
    assert!(exceeds_work(&failure, upper), "{failure:?}");
}

#[test]
fn objectives_keep_domain_guards_active() {
    let source = format!("{}#minimize{{X+Y@1,X,Y:r(X,Y)}}.", selective());
    for strategy in [JoinStrategy::Indexed, JoinStrategy::Table] {
        let off = Observation::default();
        let on = Observation::default();
        let complete = ground(&source, strategy, None, &off).unwrap();
        let narrowed = ground(&source, strategy, Some(DomainLimits::default()), &on).unwrap();
        equal(&complete, &narrowed);
        assert_eq!(on.status.get(), Some(Status::FixedPoint));
        assert!(on.support.get().domain_rejected_rows.unwrap() > 0);
        assert!(on.support.get().join_rows < off.support.get().join_rows);
        let before = *off.rules.borrow().last().unwrap();
        let after = *on.rules.borrow().last().unwrap();
        assert!(after.domain_rejected_rows.unwrap() > 0);
        assert!(after.join_rows < before.join_rows);
        assert!(after.join_probes < before.join_probes);
        assert_eq!(after.roots, before.roots);
    }
}

#[test]
fn objective_observations_preserve_scored_answers() {
    // The rules have one answer containing all six atoms. Objective-local Y
    // and aggregate variables never enter that rule carrier or its domains.
    for (objective, expected_costs) in [
        ("#minimize{X@2,X:p(X);1@1,Y:Y=1..2}.", vec![1, 2]),
        ("#maximize{X@2,X:p(X);1@1,Y:Y=1..2}.", vec![-1, -2]),
        (
            ":~p(X),1=#count{Y:p(Y),Y=X}.[X@2,X] :~p(X).[1@1,X]",
            vec![1, 2],
        ),
    ] {
        let source = format!(
            "d(0..3).p(X):-d(X),X<2.{objective}#show p/1.#show f(X):p(X).#project p(X):p(X)."
        );
        for strategy in [JoinStrategy::Indexed, JoinStrategy::Table] {
            let off = Observation::default();
            let on = Observation::default();
            let complete = ground(&source, strategy, None, &off).unwrap();
            let narrowed = ground(&source, strategy, Some(DomainLimits::default()), &on).unwrap();
            equal(&complete, &narrowed);
            assert_eq!(on.status.get(), Some(Status::FixedPoint), "{source}");
            assert!(on.support.get().domain_rejected_rows.unwrap() > 0);
            let expected = std::collections::BTreeSet::from([(
                ["d(0)", "d(1)", "d(2)", "d(3)", "p(0)", "p(1)"]
                    .map(str::to_owned)
                    .into_iter()
                    .collect(),
                Some(expected_costs.clone()),
            )]);
            assert_eq!(zetesis_reference_support::exhaustive(&complete), expected);
            assert_eq!(zetesis_reference_support::exhaustive(&narrowed), expected);
            for input in [&complete, &narrowed] {
                let model = zetesis_core::Model::from_positions(
                    input.atom_catalog(),
                    0..input.atoms().len(),
                )
                .unwrap();
                let rendered = input
                    .metadata()
                    .observations()
                    .render(
                        &model,
                        input.metadata().output(),
                        zetesis_themelios::observation::Limits::default(),
                        &zetesis_cpu::Cancellation::default(),
                    )
                    .unwrap();
                let mut shown: Vec<_> = rendered.text().split_whitespace().collect();
                shown.sort_unstable();
                assert_eq!(shown, ["f(0)", "f(1)", "p(0)", "p(1)"]);
                assert!(input.projection().is_explicit());
                assert_eq!(input.projection().atoms().len(), 2);
                assert!(
                    input
                        .projection()
                        .atoms()
                        .iter()
                        .all(|atom| atom.predicate().name() == "p")
                );
            }
        }
    }
}

#[test]
fn objective_warnings_survive_domain_guards() {
    for objective in ["#minimize{1/X@0,X:p(X)}.", ":~p(X).[1/X@0,X]"] {
        let source = format!("d(0..2).p(X):-d(X),X<2.{objective}");
        for strategy in [JoinStrategy::Indexed, JoinStrategy::Table] {
            let off = Observation::default();
            let on = Observation::default();
            let complete = ground(&source, strategy, None, &off).unwrap();
            let narrowed = ground(&source, strategy, Some(DomainLimits::default()), &on).unwrap();
            equal(&complete, &narrowed);
            assert_eq!(on.status.get(), Some(Status::FixedPoint));
            assert!(on.support.get().domain_rejected_rows.unwrap() > 0);
            assert_eq!(narrowed.warnings().len(), 1);
            assert_eq!(
                zetesis_reference_support::exhaustive(&complete),
                zetesis_reference_support::exhaustive(&narrowed)
            );
        }
    }
}

#[test]
fn objective_sources_keep_reached_arithmetic_refusals() {
    for source in [
        "d(0).p(X):-d(X).#minimize{1/X:p(X)}.",
        "d(0..1).p(X):-d(X).#minimize{1/X+((2147483647+(1-X))\\2):p(X)}.",
        "d(1;foo).p(X):-d(X),X+1>0.#minimize{1:p(X)}.",
        "d(1;foo).p(X):-d(X).#minimize{X+1:p(X)}.",
        "d(0..2).e(0).p(X):-d(X),e(X).#minimize{1/X:p(X)}.",
        "d(1;foo;2).e(1;foo).p(X):-d(X),e(X).q(X):-p(X),X+1>0.#minimize{1:q(X)}.",
    ] {
        for strategy in [JoinStrategy::Indexed, JoinStrategy::Table] {
            let off = ground(source, strategy, None, &Observation::default()).unwrap_err();
            let on = Observation::default();
            let narrowed =
                ground(source, strategy, Some(DomainLimits::default()), &on).unwrap_err();
            assert_eq!(on.status.get(), Some(Status::FixedPoint), "{source}");
            let FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                error: expected,
                location: expected_location,
            }) = off
            else {
                panic!("unexpected complete-join refusal: {off}");
            };
            assert!(
                matches!(narrowed,
                    FormulaFailure::Expansion(ExpansionFailure::Evaluation { error, location })
                    if error == expected && location == expected_location),
                "{source}"
            );
        }
    }
}

#[test]
fn objective_domains_keep_complete_fallbacks() {
    let source = "d(0..3).p(X):-d(X),X<2.#minimize{X:p(X)}.";
    let complete = ground(source, JoinStrategy::Indexed, None, &Observation::default()).unwrap();
    for limits in [
        DomainLimits {
            max_work: 7,
            ..DomainLimits::default()
        },
        DomainLimits {
            max_values_per_argument: 0,
            ..DomainLimits::default()
        },
    ] {
        let on = Observation::default();
        let fallback = ground(source, JoinStrategy::Indexed, Some(limits), &on).unwrap();
        equal(&complete, &fallback);
        assert!(on.status.get().is_some());
        assert_eq!(on.work.get().domain_guard_rows, Some(0));
        assert_eq!(on.work.get().domain_rejected_rows, Some(0));
        assert_eq!(
            zetesis_reference_support::exhaustive(&complete),
            zetesis_reference_support::exhaustive(&fallback)
        );
    }
}
