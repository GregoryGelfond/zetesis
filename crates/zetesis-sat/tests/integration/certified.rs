//! Optional specialization preserves full reduct membership and cumulative limits.

use std::collections::BTreeSet;
use std::convert::Infallible;
use std::num::NonZeroUsize;

use crate::support::batching::batch as batch_limits;
use crate::support::choice_theories::three_choices;
use crate::support::clause_search::by_clauses;
use zetesis_ferraris::{Interpretation, Node, Theory, TightError, TightPlanLimits, TightResource};
use zetesis_sat::{
    BatchError, BatchVerdict, Cancellation, CertificateError, CompletionExecutor, Incomplete,
    Limits, StableModels,
};
use zetesis_theory_support::theories::theory;

fn masks(models: impl Iterator<Item = Result<Interpretation, Incomplete>>) -> BTreeSet<Vec<usize>> {
    models.map(|m| m.unwrap().atoms().collect()).collect()
}
fn expected(theory: &Theory) -> BTreeSet<Vec<usize>> {
    (0..1usize << theory.atom_count())
        .filter_map(|mask| {
            let m = Interpretation::new(
                theory,
                (0..theory.atom_count()).filter(|a| mask & (1 << a) != 0),
            )
            .unwrap();
            zetesis_ferraris::check(
                theory,
                &m,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .accepted()
            .then(|| m.atoms().collect())
        })
        .collect()
}

#[test]
fn external_preparation_does_not_activate_cpu_checking() {
    let original = three_choices();
    let mut stream = by_clauses(&original, Limits::default(), Cancellation::default()).unwrap();
    let plan = stream
        .prepare_tight_certificate(TightPlanLimits::default())
        .unwrap()
        .unwrap();
    assert!(plan.theory().same_instance(&original));
    let work = stream.statistics().search.work;
    let again = stream
        .prepare_tight_certificate(TightPlanLimits::default())
        .unwrap()
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&plan, &again));
    assert_eq!(stream.statistics().search.work, work);
    assert_eq!(masks(stream.by_ref()), expected(&original));
    assert!(stream.exhausted());
    assert_eq!(stream.statistics().certified.unwrap().checks, 0);
    assert!(stream.statistics().countermodel_queries > 0);
}

#[test]
fn cpu_activation_reuses_external_preparation() {
    let original = three_choices();
    let mut stream = by_clauses(&original, Limits::default(), Cancellation::default()).unwrap();
    let plan = stream
        .prepare_tight_certificate(TightPlanLimits::default())
        .unwrap()
        .unwrap();
    let work = stream.statistics().search.work;
    assert!(
        stream
            .enable_certified_checking(TightPlanLimits::default())
            .unwrap()
    );
    assert!(std::sync::Arc::ptr_eq(
        &plan,
        &stream.prepared_tight_certificate().unwrap()
    ));
    assert_eq!(stream.statistics().search.work, work);
    assert_eq!(masks(stream.by_ref()), expected(&original));
    assert!(stream.statistics().certified.unwrap().checks > 0);
    assert_eq!(stream.statistics().countermodel_queries, 0);
}

#[test]
fn external_preparation_retains_optional_refusal() {
    let original = three_choices();
    let mut stream = by_clauses(&original, Limits::default(), Cancellation::default()).unwrap();
    assert!(
        stream
            .prepare_tight_certificate(TightPlanLimits {
                max_work: 1,
                ..Default::default()
            })
            .unwrap()
            .is_none()
    );
    let work = stream.statistics().search.work;
    assert!(stream.statistics().certified.unwrap().construction_work > 0);
    assert!(
        stream
            .prepare_tight_certificate(TightPlanLimits::default())
            .unwrap()
            .is_none()
    );
    assert_eq!(stream.statistics().search.work, work);
    assert_eq!(masks(stream.by_ref()), expected(&original));
    assert!(stream.exhausted());
}

#[test]
fn external_preparation_obeys_cumulative_work() {
    let original = three_choices();
    let initial = by_clauses(&original, Limits::default(), Cancellation::default())
        .unwrap()
        .statistics()
        .search
        .work;
    let mut limits = Limits::default();
    limits.search.max_work = initial + 1;
    let mut stream = by_clauses(&original, limits, Cancellation::default()).unwrap();
    assert!(matches!(
        stream.prepare_tight_certificate(TightPlanLimits::default()),
        Err(Incomplete::WorkLimit)
    ));
    assert_eq!(stream.statistics().search.work, initial + 1);
    assert!(stream.prepared_tight_certificate().is_none());
    assert!(!stream.exhausted());
}

#[test]
fn cpu_activation_refuses_pending_external_candidates() {
    let original = three_choices();
    let mut stream = by_clauses(&original, Limits::default(), Cancellation::default()).unwrap();
    stream
        .prepare_tight_certificate(TightPlanLimits::default())
        .unwrap()
        .unwrap();
    assert!(matches!(
        stream.next_batch(batch_limits(3), |_, _| Err::<Vec<BatchVerdict>, ()>(())),
        Err(BatchError::Checker(()))
    ));
    assert_eq!(stream.batch_statistics().pending, 3);
    let work = stream.statistics().search.work;
    assert!(matches!(
        stream.enable_certified_checking(TightPlanLimits::default()),
        Err(Incomplete::LateCertificate)
    ));
    assert_eq!(stream.statistics().search.work, work);
    assert_eq!(stream.statistics().certified.unwrap().checks, 0);
    assert_eq!(stream.batch_statistics().pending, 3);
    assert!(!stream.exhausted());
}

#[test]
fn scalar_and_rayon_batches_match_independent_reduct_with_support_refutations_and_refusals() {
    let cases = [
        three_choices(),
        // Unsupported carrier atoms are refuted by the support law: every
        // present atom lacks a producer, and no countermodel query is needed.
        theory(3, vec![], vec![]),
        theory(
            2,
            vec![Node::Atom(0), Node::Atom(1), Node::Implies(0, 1)],
            vec![0, 2],
        ),
        // Positive cycle and general disjunction are certificate refusals.
        theory(1, vec![Node::Atom(0), Node::Implies(0, 0)], vec![1]),
        theory(
            2,
            vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1)],
            vec![2],
        ),
        // A frozen requirement cannot give an unsupported atom a producer.
        theory(
            1,
            vec![
                Node::Atom(0),
                Node::False,
                Node::Implies(0, 1),
                Node::Implies(2, 1),
            ],
            vec![3],
        ),
    ];
    for (index, theory) in cases.iter().enumerate() {
        let wanted = expected(theory);
        let mut scalar = by_clauses(theory, Limits::default(), Cancellation::default()).unwrap();
        let eligible = scalar
            .enable_certified_checking(TightPlanLimits::default())
            .unwrap();
        assert_eq!(eligible, ![3, 4].contains(&index));
        assert_eq!(masks(scalar.by_ref()), wanted);
        assert!(scalar.exhausted());
        let stats = scalar.statistics();
        let certificate = stats.certified.unwrap();
        assert!(stats.search.work >= certificate.construction_work + certificate.checking_work);
        if index == 0 {
            assert_eq!(stats.countermodel_queries, 0);
            assert_eq!(certificate.stable, 8);
        }
        if index == 1 {
            assert_eq!(stats.countermodel_queries, 0);
            assert_eq!(stats.countermodels, 0);
            assert_eq!(certificate.refuted, 7);
        }
        for workers in [1, 4] {
            for size in [1, 2, 5] {
                let mut stream =
                    by_clauses(theory, Limits::default(), Cancellation::default()).unwrap();
                stream.enable_phase_timing();
                assert_eq!(
                    stream
                        .enable_certified_checking(TightPlanLimits::default())
                        .unwrap(),
                    eligible
                );
                let mut executor =
                    CompletionExecutor::new(NonZeroUsize::new(workers).unwrap()).unwrap();
                let mut actual = BTreeSet::new();
                while !stream.exhausted() {
                    let batch = stream
                        .next_batch_with_completion(
                            batch_limits(size),
                            &mut executor,
                            |original, candidates| {
                                assert!(original.same_instance(theory));
                                Ok::<_, Infallible>(vec![BatchVerdict::Residual; candidates.len()])
                            },
                        )
                        .unwrap();
                    for m in batch {
                        assert!(actual.insert(m.atoms().collect()));
                    }
                }
                assert_eq!(actual, wanted);
                assert_eq!(stream.batch_statistics().pending, 0);
                assert_eq!(
                    stream.batch_statistics().committed,
                    stream.statistics().candidates
                );
                if eligible {
                    assert!(stream.statistics().phase_timings.unwrap().certified.calls > 0);
                }
                if index == 0 {
                    assert_eq!(stream.statistics().countermodel_queries, 0);
                }
            }
        }
    }
}

#[test]
fn failed_construction_is_charged_and_cannot_restart_or_reset_search_limits() {
    let original = three_choices();
    let mut baseline =
        StableModels::new(&original, Limits::default(), Cancellation::default()).unwrap();
    let before = baseline.statistics().search.work;
    let local = TightPlanLimits {
        max_work: 3,
        ..Default::default()
    };
    assert!(!baseline.enable_certified_checking(local).unwrap());
    assert_eq!(baseline.statistics().search.work, before + 3);
    assert_eq!(
        baseline.statistics().certified.unwrap().refusal,
        Some(CertificateError::Tight(TightError::Limit(
            TightResource::Work
        )))
    );
    assert!(
        !baseline
            .enable_certified_checking(TightPlanLimits::default())
            .unwrap()
    );
    assert_eq!(baseline.statistics().search.work, before + 3);
    assert_eq!(masks(baseline.by_ref()), expected(&original));
    assert_eq!(baseline.statistics().countermodel_queries, 8);

    let mut limits = Limits::default();
    limits.search.max_work = before + 3;
    let mut stopped = StableModels::new(&original, limits, Cancellation::default()).unwrap();
    assert_eq!(
        stopped.enable_certified_checking(TightPlanLimits::default()),
        Err(Incomplete::WorkLimit)
    );
    assert_eq!(stopped.statistics().search.work, limits.search.max_work);
    assert!(!stopped.exhausted());
    assert!(stopped.next().is_none());
}

#[test]
fn exact_scalar_work_boundary_includes_certification_and_residual_completion() {
    for original in [three_choices(), theory(3, vec![], vec![])] {
        let run = |ceiling| {
            let mut limits = Limits::default();
            limits.search.max_work = ceiling;
            let mut stream = StableModels::new(&original, limits, Cancellation::default()).unwrap();
            stream
                .enable_certified_checking(TightPlanLimits::default())
                .unwrap();
            let results = stream.by_ref().collect::<Vec<_>>();
            (results, stream)
        };
        let (_, full) = run(u64::MAX);
        let exact = full.statistics().search.work;
        let (values, at) = run(exact);
        assert!(at.exhausted());
        assert!(values.iter().all(Result::is_ok));
        let (values, below) = run(exact - 1);
        assert!(!below.exhausted());
        assert_eq!(
            values.last().unwrap().as_ref().unwrap_err(),
            &Incomplete::WorkLimit
        );
        assert_eq!(below.statistics().search.work, exact - 1);
    }
}

#[test]
fn certificate_failure_retains_pending_candidates_without_reentering_completion() {
    let original = three_choices();
    // Construction fits, but the per-candidate plan plus scratch does not.
    let probe = zetesis_ferraris::TightPlan::compile(
        &original,
        TightPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let plan_bytes = probe.statistics().construction_bytes;
    // A work refusal is independently reproducible without conflating bytes.
    let limits = Limits {
        max_verification_work: original.nodes().len() as u64 + original.roots().len() as u64,
        ..Default::default()
    };
    // Initial proposal validation fits this exact amount; ranked support needs more.
    let mut stream = StableModels::new(&original, limits, Cancellation::default()).unwrap();
    assert!(
        stream
            .enable_certified_checking(TightPlanLimits {
                max_bytes: plan_bytes,
                ..Default::default()
            })
            .unwrap()
    );
    let mut pool = CompletionExecutor::default();
    let result = stream.next_batch_with_completion(batch_limits(2), &mut pool, |_, rows| {
        Ok::<_, Infallible>(vec![BatchVerdict::Residual; rows.len()])
    });
    assert!(matches!(
        result,
        Err(BatchError::Search(Incomplete::Certificate(
            CertificateError::Tight(TightError::Limit(TightResource::Work))
        )))
    ));
    assert_eq!(stream.batch_statistics().completion_calls, 0);
    assert_eq!(stream.batch_statistics().pending, 2);
    assert_eq!(stream.statistics().stable_models, 0);
    assert_eq!(stream.statistics().certified.unwrap().failed, 1);
    assert_eq!(pool.last_statistics(), None);
    assert!(!stream.exhausted());
}

#[test]
fn restrictions_keep_original_certificate_and_late_configuration_is_explicit() {
    let original = three_choices();
    let restriction = theory(3, vec![Node::Atom(0)], vec![0]);
    let mut stream = by_clauses(&original, Limits::default(), Cancellation::default()).unwrap();
    stream
        .enable_certified_checking(TightPlanLimits::default())
        .unwrap();
    stream.restrict_candidates(&restriction).unwrap();
    let results = masks(stream.by_ref());
    assert_eq!(results.len(), 4);
    assert!(results.iter().all(|m| m.contains(&0)));
    assert_eq!(stream.statistics().countermodel_queries, 0);
    assert_eq!(stream.statistics().candidate_restrictions, 1);
    let mut late = by_clauses(&original, Limits::default(), Cancellation::default()).unwrap();
    late.next().unwrap().unwrap();
    assert_eq!(
        late.enable_certified_checking(TightPlanLimits::default()),
        Err(Incomplete::LateCertificate)
    );
    assert_eq!(late.statistics().certified, None);
}
