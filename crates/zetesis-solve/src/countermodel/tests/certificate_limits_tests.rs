//! Ordinary certificate preparation shares the search work and memory policy.

use super::super::{certificate_limits, prepare_certificate, search_limits};
use crate::SolveConfig;
use crate::execution_observation::Ignore;
use crate::phase_timing::Recorder;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, FormulaParts, Node, PositiveError, PositiveResource, Theory, TightError,
    TightPlan, TightPlanLimits, TightResource,
};
use zetesis_sat::{CertificateOrder, CertificatePlanStatistics, Incomplete, StableModels};

fn repeated_fact(roots: usize) -> Theory {
    Theory::new(
        1,
        FormulaParts::new(vec![Node::atom(0)], vec![]).unwrap(),
        vec![0; roots],
        AdmissionLimits {
            max_roots: roots,
            ..AdmissionLimits::default()
        },
    )
    .unwrap()
}

fn models(theory: &Theory, config: &SolveConfig, cancellation: Cancellation) -> StableModels {
    StableModels::with_method(
        theory,
        zetesis_sat::SearchMethod::Regions,
        search_limits(config),
        cancellation,
    )
    .unwrap()
}

fn prepare(
    models: &mut StableModels,
    config: &SolveConfig,
    order: CertificateOrder,
) -> Option<Incomplete> {
    prepare_certificate(models, order, config, &mut Ignore, &Recorder::new(false)).unwrap()
}

#[test]
fn ordinary_tight_preparation_exceeds_the_old_producer_cap() {
    // Repeated original roots remain distinct producers. One atom makes the
    // complete answer family cheap without weakening the exercised population.
    let roots = 262_145;
    let theory = repeated_fact(roots);
    assert!(matches!(
        TightPlan::compile(
            &theory,
            TightPlanLimits::default(),
            &Cancellation::default()
        ),
        Err(TightError::Limit(TightResource::Producers))
    ));
    let config = SolveConfig::default();
    let mut models = models(&theory, &config, Cancellation::default());
    assert_eq!(
        prepare(&mut models, &config, CertificateOrder::TightFirst),
        None
    );
    assert!(matches!(
        models.statistics().certified.unwrap().plan,
        Some(CertificatePlanStatistics::Tight(_))
    ));
    assert_eq!(
        models
            .prepared_tight_certificate()
            .unwrap()
            .producers()
            .len(),
        roots
    );
    assert_eq!(
        models.next().unwrap().unwrap().atoms().collect::<Vec<_>>(),
        [0]
    );
    assert!(models.next().is_none());
    assert!(models.exhausted());
    assert_eq!(models.statistics().countermodel_queries, 0);
}

#[test]
fn ordinary_certificate_counts_have_no_fixed_caps() {
    let limits = certificate_limits(&SolveConfig::default());
    // Exercise the positive dependency policy without manufacturing millions
    // of incidences solely to cross its primitive's explicit default.
    assert_eq!(limits.tight.max_producers, usize::MAX);
    assert_eq!(limits.tight.max_dependencies, usize::MAX);
    assert_eq!(limits.positive.max_dependencies, usize::MAX);
}

#[test]
fn positive_preparation_uses_the_search_work_allowance() {
    let config = SolveConfig {
        // This separate allowance governs per-candidate verification, not
        // preparation of the original theory's positive certificate.
        max_work: 0,
        ..SolveConfig::default()
    };
    let limits = certificate_limits(&config);
    assert_eq!(limits.positive.max_work, config.max_search_work);
    assert_eq!(limits.tight.max_work, config.max_search_work);
    let mut models = models(&repeated_fact(1), &config, Cancellation::default());
    assert_eq!(
        prepare(&mut models, &config, CertificateOrder::PositiveFirst),
        None
    );
    assert!(matches!(
        models.statistics().certified.unwrap().plan,
        Some(CertificatePlanStatistics::Positive(_))
    ));
}

#[test]
fn certificate_memory_refusal_preserves_general_answers() {
    let owner = super::test_harness::admitted("{p}.");
    let config = SolveConfig {
        max_completion_scratch_bytes: 0,
        ..SolveConfig::default()
    };
    for order in [
        CertificateOrder::TightFirst,
        CertificateOrder::PositiveFirst,
    ] {
        let mut models = models(owner.theory(), &config, Cancellation::default());
        assert_eq!(prepare(&mut models, &config, order), None);
        let certificate = models.statistics().certified.unwrap();
        assert_eq!(certificate.plan, None);
        assert_eq!(
            certificate.tight_refusal,
            Some(TightError::Limit(TightResource::Bytes))
        );
        assert!(matches!(
            certificate.positive_refusal,
            Some(PositiveError::Limit {
                resource: PositiveResource::Bytes,
                limit: 0,
                ..
            })
        ));
        let mut answers: Vec<_> = models
            .by_ref()
            .map(|answer| answer.unwrap().atoms().collect::<Vec<_>>())
            .collect();
        answers.sort();
        assert_eq!(answers, [vec![], vec![0]]);
        assert!(models.exhausted());
        assert!(models.statistics().countermodel_queries > 0);
    }
}

#[test]
fn cancelled_preparation_cannot_select_a_certificate() {
    let theory = repeated_fact(1);
    let config = SolveConfig::default();
    for order in [
        CertificateOrder::TightFirst,
        CertificateOrder::PositiveFirst,
    ] {
        let cancellation = Cancellation::default();
        let mut models = models(&theory, &config, cancellation.clone());
        let before = models.statistics().search.work;
        cancellation.cancel();
        assert_eq!(
            prepare(&mut models, &config, order),
            Some(Incomplete::Cancelled)
        );
        let certificate = models.statistics().certified.unwrap();
        assert_eq!(certificate.plan, None);
        assert_eq!(certificate.construction_work, 0);
        assert_eq!(models.statistics().search.work, before);
        assert!(models.next().is_none());
        assert!(!models.exhausted());
    }
}

#[test]
fn certificate_preparation_spends_only_remaining_search_work() {
    let theory = repeated_fact(1);
    let initial = models(&theory, &SolveConfig::default(), Cancellation::default())
        .statistics()
        .search
        .work;
    let config = SolveConfig {
        max_search_work: initial + 1,
        ..SolveConfig::default()
    };
    for order in [
        CertificateOrder::TightFirst,
        CertificateOrder::PositiveFirst,
    ] {
        let mut models = models(&theory, &config, Cancellation::default());
        assert_eq!(
            prepare(&mut models, &config, order),
            Some(Incomplete::WorkLimit)
        );
        let certificate = models.statistics().certified.unwrap();
        assert_eq!(certificate.plan, None);
        assert_eq!(certificate.construction_work, 1);
        assert_eq!(models.statistics().search.work, config.max_search_work);
        assert!(models.next().is_none());
        assert!(!models.exhausted());
    }
}
