//! Actual optional-domain profiling and its independent reference boundary.

use zetesis_experiments::grounding::{Configuration, Error, Mode, Report, profile};
use zetesis_themelios::{
    DomainLimits, FormulaFailure, FormulaLimits, FormulaResource, GroundingOutcome, GroundingPhase,
    GroundingWork, JoinStrategy, SourceBundle, prepare_bundle_formula,
};

use super::{configuration, source};

fn work(report: &Report, selected: GroundingPhase) -> GroundingWork {
    report
        .samples
        .iter()
        .find(|sample| sample.mode == Mode::Detailed)
        .unwrap()
        .phases
        .iter()
        .filter(|phase| phase.phase == selected)
        .fold(GroundingWork::default(), |sum, phase| {
            sum.checked_sum(phase.work)
        })
}

fn paired(joins: JoinStrategy) -> (Report, Report) {
    let mut config = configuration();
    config.grounding.joins = joins;
    let before = profile(source("domain-prefixes.lp"), config).unwrap();
    assert!(before.complete, "{:?}", before.failure);
    config.domain_analysis = Some(DomainLimits::default());
    let after = profile(source("domain-prefixes.lp"), config).unwrap();
    assert!(after.complete, "{:?}", after.failure);
    (before, after)
}

#[test]
fn domains_preserve_the_qualified_subject() {
    for joins in [JoinStrategy::Indexed, JoinStrategy::Table] {
        let (before, after) = paired(joins);
        assert_eq!(after.subject_fingerprint, before.subject_fingerprint);
        assert_eq!(after.atoms, before.atoms);
        assert!(!after.reference_domain_analysis);
        let qualified = after.qualification.as_ref().unwrap();
        assert!(qualified.exhausted);
        // Every source fact and exactly r(5..8,5..8) belong to the one least model.
        assert_eq!(qualified.interpretations, [(0_usize..96).collect::<Vec<_>>()]);
        let actual: Vec<_> = after
            .atoms
            .iter()
            .filter(|atom| atom.starts_with("r("))
            .cloned()
            .collect();
        let expected: Vec<_> = (5..=8)
            .flat_map(|left| (5..=8).map(move |right| format!("r({left},{right})")))
            .collect();
        assert_eq!(actual, expected);
        for sample in &after.samples {
            assert_eq!(sample.subject_equal, Some(true));
            assert_eq!(sample.subject_fingerprint, after.subject_fingerprint);
            let models = sample.models.as_ref().unwrap();
            assert!(models.exhausted);
            assert_eq!(models.interpretations, qualified.interpretations);
        }
    }
}

#[test]
fn domain_receipts_describe_actual_guard_work() {
    for joins in [JoinStrategy::Indexed, JoinStrategy::Table] {
        let (before, after) = paired(joins);
        assert_eq!(
            work(&before, GroundingPhase::SupportCompletion),
            work(&after, GroundingPhase::SupportCompletion)
        );
        let off = work(&before, GroundingPhase::RuleInstantiation);
        let on = work(&after, GroundingPhase::RuleInstantiation);
        assert_eq!(off.join_probes, Some(73));
        assert_eq!(on.join_probes, Some(21));
        // Indexed exposes a shortest posting: each of the sixteen viable c
        // queries visits eight rows. Table already intersects both equalities.
        let (before_rows, after_rows, rejected) = match joins {
            JoinStrategy::Indexed => (200, 168, 84),
            JoinStrategy::Table => (88, 56, 20),
        };
        assert_eq!(off.join_rows, Some(before_rows));
        assert_eq!(on.join_rows, Some(after_rows));
        assert_eq!(on.domain_rejected_rows, Some(rejected));
        assert_eq!(on.binding_snapshots, off.binding_snapshots);
        if joins == JoinStrategy::Table {
            assert!(on.table_probes.unwrap() > 0);
        }
        let analysis = work(&after, GroundingPhase::DomainAnalysis);
        assert!(analysis.domain_prepare_work.unwrap() > 0);
        assert!(on.domain_prepare_work.unwrap() > 0);
        assert!(on.domain_guard_rows.unwrap() > 0);
        assert!(on.domain_guard_checks.unwrap() > 0);
        let detailed = after
            .samples
            .iter()
            .find(|sample| sample.mode == Mode::Detailed)
            .unwrap();
        for phase in &detailed.phases {
            let encoded = serde_json::to_value(phase).unwrap();
            assert_eq!(encoded["phase"], phase.phase.label());
            for (name, count) in [
                ("domain_prepare_work", phase.work.domain_prepare_work),
                ("domain_guard_rows", phase.work.domain_guard_rows),
                ("domain_guard_checks", phase.work.domain_guard_checks),
                ("domain_rejected_rows", phase.work.domain_rejected_rows),
            ] {
                assert_eq!(encoded["work"][name].as_u64(), count);
            }
        }
    }
}

#[test]
fn stopped_domain_analysis_retains_complete_fallback() {
    let mut config = configuration();
    config.domain_analysis = Some(DomainLimits {
        max_work: 7,
        ..DomainLimits::default()
    });
    let report = profile(source("domain-prefixes.lp"), config).unwrap();
    assert!(report.complete, "{:?}", report.failure);
    let qualified = report.qualification.as_ref().unwrap();
    assert_eq!(qualified.interpretations, [(0_usize..96).collect::<Vec<_>>()]);
    for sample in &report.samples {
        assert_eq!(sample.subject_equal, Some(true));
        let models = sample.models.as_ref().unwrap();
        assert!(models.exhausted);
        assert_eq!(models.interpretations, qualified.interpretations);
    }
    let detailed = report
        .samples
        .iter()
        .find(|sample| sample.mode == Mode::Detailed)
        .unwrap();
    let phase = detailed
        .phases
        .iter()
        .find(|phase| phase.phase == GroundingPhase::DomainAnalysis)
        .unwrap();
    // A completed optional attempt can retain an analysis stop and use the full
    // join. Completion of that attempt does not establish an analysis fixed point.
    assert_eq!(phase.outcome, GroundingOutcome::Completed);
    assert!(phase.work.domain_prepare_work.unwrap() >= 7);
    let rules = work(&report, GroundingPhase::RuleInstantiation);
    assert_eq!(rules.domain_guard_rows, Some(0));
    assert_eq!(rules.domain_rejected_rows, Some(0));
    assert_eq!(rules.join_probes, Some(73));
}

fn fact_admission(
    maximum: u64,
) -> Result<zetesis_themelios::AdmittedFormulaBundle, zetesis_themelios::FormulaBundleFailure> {
    let config = configuration();
    let bundle = SourceBundle::load(source("domain-fact.lp"), config.bundle).unwrap();
    prepare_bundle_formula(
        bundle,
        config.admission,
        config.expansion,
        FormulaLimits {
            max_work: maximum,
            ..config.formula
        },
    )?
    .ground()
}

#[test]
fn domain_refusal_keeps_the_unmodified_reference() {
    // Find this real fact's inclusive admission boundary. Only typed Work
    // refusals move the lower bound; no fixed implementation cost is assumed.
    let mut upper = FormulaLimits::default().max_work;
    assert!(fact_admission(upper).is_ok());
    let mut lower = 0;
    while lower + 1 < upper {
        let middle = lower + (upper - lower) / 2;
        match fact_admission(middle) {
            Ok(_) => upper = middle,
            Err(error) => {
                assert!(
                    matches!(error.error(), FormulaFailure::Limit {
                    resource: FormulaResource::Work, limit, observed, ..
                } if *limit == u128::from(middle) && observed > limit),
                    "{error:?}"
                );
                lower = middle;
            }
        }
    }
    let mut config = configuration();
    config.formula.max_work = upper;
    config.domain_analysis = Some(DomainLimits::default());
    let report = profile(source("domain-fact.lp"), config).unwrap();
    assert!(!report.complete);
    let qualified = report
        .qualification
        .as_ref()
        .expect("unmodified reference qualifies");
    assert!(qualified.exhausted);
    assert_eq!(report.atoms, ["p(1)"]);
    assert_eq!(qualified.interpretations, [vec![0]]);
    assert_eq!(report.samples.len(), 1);
    assert!(!report.samples[0].admitted);
    assert!(report.samples[0].models.is_none());
    let Some(Error::Admission(error)) = report.failure else {
        panic!("expected measured work refusal");
    };
    assert!(matches!(error.error(), FormulaFailure::Limit {
        resource: FormulaResource::Work, limit, observed, ..
    } if *limit == u128::from(upper) && observed > limit));
}

#[test]
fn requested_domain_limits_are_serialized() {
    let config = Configuration {
        domain_analysis: Some(DomainLimits {
            max_work: 101,
            max_predicates: 102,
            max_positions: 103,
            max_links: 104,
            max_values_per_argument: 105,
            max_value_entries: 106,
            max_rounds: 107,
            max_inspected_bytes: 108,
            max_symbol_nodes: 109,
            max_symbol_depth: 110,
            max_symbol_bytes: 111,
        }),
        ..configuration()
    };
    assert_eq!(
        serde_json::to_value(config).unwrap()["domain_analysis"],
        serde_json::json!({
            "max_work": 101, "max_predicates": 102, "max_positions": 103, "max_links": 104,
            "max_values_per_argument": 105, "max_value_entries": 106, "max_rounds": 107,
            "max_inspected_bytes": 108, "max_symbol_nodes": 109, "max_symbol_depth": 110,
            "max_symbol_bytes": 111,
        })
    );
    assert!(serde_json::to_value(configuration()).unwrap()["domain_analysis"].is_null());
}

#[test]
fn projection_limits_are_serialized() {
    let mut config = configuration();
    config.search.projections = zetesis_sat::ProjectionLimits {
        max_entries: 19,
        max_nodes: 71,
        max_bytes: 997,
    };
    assert_eq!(
        serde_json::to_value(config).unwrap()["search"]["projections"],
        serde_json::json!({ "max_entries": 19, "max_nodes": 71, "max_bytes": 997 })
    );
}
