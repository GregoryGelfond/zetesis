//! Optional clocks must not change answers, work, retry ownership or exhaustion.

use std::num::NonZeroUsize;
use std::time::Duration;

use crate::support::clause_search::by_clauses;
use zetesis_ferraris::{AdmissionLimits, Node, Theory};
use zetesis_sat::{
    BatchError, BatchLimits, BatchVerdict, Cancellation, Incomplete, Limits, PhaseMeasurement,
    SearchLimits, StableModels,
};

fn choice() -> Theory {
    // a OR NOT a has both {} and {a} as stable models.
    Theory::new(
        1,
        zetesis_ferraris::FormulaParts::new(
            vec![
                Node::falsum(),
                Node::atom(0),
                Node::implies(1, 0),
                Node::or_pair([1, 2]),
            ],
            vec![],
        )
        .unwrap(),
        vec![3],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn batch() -> BatchLimits {
    BatchLimits {
        max_candidates: NonZeroUsize::new(2).unwrap(),
        max_pending_bytes: 4096,
    }
}

#[test]
fn enabling_clocks_preserves_models_and_all_deterministic_counters() {
    for theory in [
        choice(),
        // a <- a has only the empty stable model, though {a} models the formula.
        Theory::new(
            1,
            zetesis_ferraris::FormulaParts::new(vec![Node::atom(0), Node::implies(0, 0)], vec![])
                .unwrap(),
            vec![1],
            AdmissionLimits::default(),
        )
        .unwrap(),
    ] {
        let mut expected = None;
        for enabled in [false, true] {
            let mut search =
                StableModels::new(&theory, Limits::default(), Cancellation::default()).unwrap();
            if enabled {
                search.enable_phase_timing();
            }
            let mut models = Vec::new();
            while let Some(model) = search.next() {
                models.push(model.unwrap().atoms().collect::<Vec<_>>());
                let before = search.statistics();
                if enabled {
                    search.enable_phase_timing();
                    assert_eq!(search.statistics(), before, "enabling is idempotent");
                }
            }
            assert!(search.exhausted());
            let mut stats = search.statistics();
            if let Some(timing) = stats.phase_timings.take() {
                assert!(timing.candidates.calls >= stats.candidate_queries);
                assert_eq!(timing.original_validation.calls, stats.candidates);
                assert_eq!(timing.reduct.calls, stats.candidates);
                assert!(!timing.candidates.overflowed);
            } else {
                assert!(!enabled);
            }
            if let Some((old_models, old_stats)) = &expected {
                assert_eq!(&models, old_models);
                assert_eq!(&stats, old_stats);
            } else {
                expected = Some((models, stats));
            }
        }
    }
}

#[test]
fn failed_checker_keeps_proposals_and_retry_measures_only_actual_native_residuals() {
    let theory = choice();
    let mut search = by_clauses(&theory, Limits::default(), Cancellation::default()).unwrap();
    search.enable_phase_timing();
    assert!(matches!(
        search.next_batch(batch(), |_, candidates| {
            assert_eq!(candidates.len(), 2);
            Err::<Vec<BatchVerdict>, _>("temporarily unavailable")
        }),
        Err(BatchError::Checker("temporarily unavailable"))
    ));
    let before = search.statistics();
    let timing = before.phase_timings.unwrap();
    assert!(timing.candidates.calls > 0);
    assert_eq!(timing.original_validation.calls, 2);
    assert_eq!(
        timing.reduct.calls, 0,
        "checker failure is not a reduct query"
    );
    assert_eq!(search.batch_statistics().pending, 2);
    assert!(!search.exhausted());
    let models = search
        .next_batch(batch(), |_, candidates| {
            Ok::<_, &'static str>(vec![BatchVerdict::Residual; candidates.len()])
        })
        .unwrap();
    assert_eq!(models.len(), 2);
    let after = search.statistics();
    assert_eq!(after.phase_timings.unwrap().candidates, timing.candidates);
    assert_eq!(after.candidates, before.candidates);
    assert_eq!(after.phase_timings.unwrap().reduct.calls, 2);
    assert_eq!(search.batch_statistics().pending, 0);
    assert!(
        search
            .next_batch(batch(), |_, _| -> Result<_, &'static str> {
                panic!("all semantic candidates already checked")
            })
            .unwrap()
            .is_empty()
    );
    assert!(search.exhausted());
}

#[test]
fn failed_outer_and_original_attempts_are_recorded_without_false_completion() {
    for verification in [false, true] {
        let theory = choice();
        let limits = Limits {
            search: SearchLimits {
                max_decisions: if verification { u64::MAX } else { 0 },
                ..SearchLimits::default()
            },
            max_verification_work: if verification { 0 } else { u64::MAX },
            ..Limits::default()
        };
        let mut search = StableModels::new(&theory, limits, Cancellation::default()).unwrap();
        search.enable_phase_timing();
        let error = search.next().unwrap().unwrap_err();
        assert!(matches!(
            error,
            Incomplete::DecisionLimit | Incomplete::Verification(zetesis_cpu::Stop::WorkLimit)
        ));
        let timing = search.statistics().phase_timings.unwrap();
        // One call builds the original index when the walk starts; one is
        // the proposal that stopped or was refused.
        assert_eq!(timing.candidates.calls, 2);
        assert_eq!(timing.original_validation.calls, u64::from(verification));
        assert_eq!(timing.reduct.calls, 0);
        assert_eq!(search.statistics().stable_models, 0);
        assert!(search.next().is_none());
        assert!(!search.exhausted());
    }
}

#[test]
fn measurement_overflow_preserves_an_explicit_incomplete_prefix() {
    for mut measurement in [
        PhaseMeasurement {
            calls: u64::MAX,
            elapsed: Duration::ZERO,
            overflowed: false,
        },
        PhaseMeasurement {
            calls: 1,
            elapsed: Duration::MAX,
            overflowed: false,
        },
    ] {
        let before = measurement;
        measurement.record(Duration::from_nanos(1));
        assert!(measurement.overflowed);
        assert_eq!(measurement.calls, before.calls);
        assert_eq!(measurement.elapsed, before.elapsed);
        let stopped = measurement;
        measurement.record(Duration::ZERO);
        assert_eq!(measurement, stopped, "never resume a partial measurement");
    }
}

#[test]
fn failed_cold_preparation_is_recorded_before_worker_entry() {
    let theory = choice();
    let mut probe = by_clauses(&theory, Limits::default(), Cancellation::default()).unwrap();
    assert!(matches!(
        probe.next_batch(batch(), |_, _| Err::<Vec<BatchVerdict>, _>("hold")),
        Err(BatchError::Checker("hold"))
    ));
    let mut search = by_clauses(
        &theory,
        Limits {
            search: SearchLimits {
                max_work: probe.statistics().search.work,
                ..SearchLimits::default()
            },
            ..Limits::default()
        },
        Cancellation::default(),
    )
    .unwrap();
    search.enable_phase_timing();
    assert!(matches!(
        search.next_batch(batch(), |_, candidates| Ok::<_, &'static str>(
            vec![BatchVerdict::Residual; candidates.len()]
        )),
        Err(BatchError::Search(Incomplete::WorkLimit))
    ));
    let stats = search.statistics();
    assert_eq!(
        stats.countermodel_queries, 0,
        "cold preparation failed before query entry"
    );
    let timing = stats.phase_timings.unwrap();
    assert_eq!(timing.reduct_preparation.calls, 1);
    assert_eq!(timing.reduct.calls, 0);
    assert_eq!(
        timing.original_validation.calls, 2,
        "only proposals were validated"
    );
    let preparation = stats.reduct.preparation.unwrap();
    assert_eq!(
        preparation.work, 0,
        "the proposal exhausted the work allowance"
    );
    assert_eq!(preparation.retained_bytes, 0);
    assert_eq!(stats.stable_models, 0);
    assert_eq!(search.batch_statistics().pending, 2);
    assert!(!search.exhausted());
}
