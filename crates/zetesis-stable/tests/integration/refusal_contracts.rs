//! Typed failures preserve exact reduct enumeration and distinguish incomplete work.

use crate::support::choice_theories::choices;
use std::collections::BTreeSet;
use std::error::Error as _;
use std::time::Instant;

use zetesis_algebra::{Refusal, Stop};
use zetesis_ferraris::{
    AdmissionError, AdmissionLimits, EvaluationError, Interpretation, PositiveError,
    PositiveResource, StratifiedError, StratifiedResource, Theory, TightError, TightResource,
};
use zetesis_stable::{
    Cancellation, CertificateError, Check, Incomplete, Limits, StableModels, check,
};

fn remaining(search: &mut StableModels) -> BTreeSet<Vec<usize>> {
    let result = search
        .by_ref()
        .map(|entry| entry.unwrap().atoms().collect())
        .collect();
    assert!(search.exhausted());
    result
}

#[test]
fn mismatched_restriction_error_does_not_consume_the_live_enumerator() {
    let original = choices(2);
    let mut search =
        StableModels::new(&original, Limits::default(), Cancellation::default()).unwrap();
    let wrong = Theory::new(
        1,
        zetesis_ferraris::FormulaParts::new(vec![], vec![]).unwrap(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    let before = search.statistics();
    let error = search.restrict_candidates(&wrong).unwrap_err();
    assert_eq!(
        error,
        Incomplete::RestrictionUniverse {
            expected: 2,
            actual: 1
        }
    );
    assert!(error.to_string().contains("declares 1 atoms; expected 2"));
    assert!(error.source().is_none());
    assert_eq!(search.statistics(), before);
    assert_eq!(
        remaining(&mut search),
        BTreeSet::from([vec![], vec![0], vec![1], vec![0, 1]])
    );
    let error = search.restrict_candidates(&original).unwrap_err();
    assert_eq!(error, Incomplete::ClosedEnumerator);
    assert!(error.to_string().contains("already closed"));
    assert!(search.next().is_none());
    assert!(search.exhausted());
}

#[test]
fn independent_verification_and_foreign_candidate_failures_are_not_rejections() {
    let original = choices(2);
    let candidate = Interpretation::new(&original, [0]).unwrap();
    let Check::Inconclusive(error) = check(
        &original,
        &candidate,
        Limits {
            max_verification_work: 0,
            ..Default::default()
        },
        &Cancellation::default(),
    ) else {
        panic!("verification must be incomplete")
    };
    assert_eq!(error, Incomplete::Verification(Stop::WorkLimit));
    assert_eq!(
        error.source().unwrap().downcast_ref::<Stop>(),
        Some(&Stop::WorkLimit)
    );
    assert!(error.to_string().starts_with("independent verification:"));
    let other = choices(2);
    let foreign = Interpretation::new(&other, [0]).unwrap();
    let Check::Inconclusive(error) = check(
        &original,
        &foreign,
        Limits::default(),
        &Cancellation::default(),
    ) else {
        panic!("foreign candidate must be incomplete")
    };
    assert_eq!(error, Incomplete::WrongTheory);
    assert!(error.to_string().contains("different theory"));
    assert!(error.source().is_none());
    let Check::Inconclusive(error) = check(
        &original,
        &candidate,
        Limits::default(),
        &Cancellation::with_deadline(Instant::now()).unwrap(),
    ) else {
        panic!("deadline must be incomplete")
    };
    assert_eq!(error, Incomplete::Deadline);
    assert!(error.to_string().contains("deadline expired"));
    assert!(
        check(
            &original,
            &candidate,
            Limits::default(),
            &Cancellation::default()
        )
        .accepted()
    );
}

/// Exercise the common exit settlement, retaining the exact performed prefix.
/// Classification may replace an error only when control requested a stop.
fn assert_settlement(error: Incomplete, resource: bool) {
    assert_eq!(error.is_resource_refusal(), resource, "{error:?}");
    for stop in [false, true] {
        let cancellation = Cancellation::default();
        let attempt = cancellation.metered(7, |meter| {
            for _ in 0..3 {
                meter.charge().unwrap();
            }
            if stop {
                cancellation.cancel();
            }
            Err::<(), _>(error)
        });
        let expected = if stop && resource {
            Incomplete::Cancelled
        } else {
            error
        };
        assert_eq!(attempt.result, Err(expected), "{error:?}, stop={stop}");
        assert_eq!(attempt.charged, 3, "{error:?}, stop={stop}");
    }
}

#[test]
fn stable_refusals_settle_without_losing_payloads() {
    for error in [
        Incomplete::WorkLimit,
        Incomplete::DecisionLimit,
        Incomplete::CandidateLimit,
        Incomplete::PendingBytes,
        Incomplete::CompletionScratch,
        Incomplete::ReductStorage {
            required: u128::MAX,
            limit: 37,
        },
        Incomplete::NativeNarrowingStorage {
            required: u128::MAX - 1,
            limit: 41,
        },
        Incomplete::BatchCandidateLimit,
        Incomplete::Allocation,
    ] {
        assert_settlement(error, true);
    }
    for error in [
        Incomplete::Cancelled,
        Incomplete::Deadline,
        Incomplete::PendingBatch,
        Incomplete::WrongTheory,
        Incomplete::RestrictionUniverse {
            expected: 19,
            actual: 23,
        },
        Incomplete::ClosedEnumerator,
        Incomplete::LateCertificate,
        Incomplete::RegionFilter,
        Incomplete::InvalidRegionConsequence,
        Incomplete::LateRegionFilter,
        Incomplete::LateCandidateBounds,
        Incomplete::CandidateBoundPlan,
        Incomplete::CandidateBoundNotImproving,
        Incomplete::RegionFilterAlreadySet,
        Incomplete::LateLearnedStore,
        Incomplete::LearnedStoreAlreadySet,
        Incomplete::InvalidWitness,
        Incomplete::WorkerPanicked,
        Incomplete::CounterOverflow,
    ] {
        assert_settlement(error, false);
    }
}

#[test]
fn wrapped_stops_retain_their_native_classification() {
    let wrappers: [fn(Stop) -> Incomplete; 5] = [
        Incomplete::Verification,
        |stop| Incomplete::Certificate(CertificateError::Tight(TightError::Stopped(stop))),
        |stop| Incomplete::Certificate(CertificateError::Positive(PositiveError::Stopped(stop))),
        |stop| {
            Incomplete::Certificate(CertificateError::Stratified(StratifiedError::Stopped(stop)))
        },
        |stop| {
            Incomplete::Certificate(CertificateError::Evaluation(EvaluationError::Stopped(stop)))
        },
    ];
    for (stop, resource) in [
        (Stop::WorkLimit, true),
        (Stop::RoundLimit, true),
        (Stop::StorageLimit, true),
        (Stop::DerivedAtomLimit, true),
        (Stop::CandidateLimit, true),
        (Stop::CarrierLimit, true),
        (Stop::Allocation, true),
        (Stop::Cancelled, false),
        (Stop::Deadline, false),
        (Stop::WrongProgram, false),
        (Stop::InvalidProgram, false),
    ] {
        for wrap in wrappers {
            assert_settlement(wrap(stop), resource);
        }
        // The common Refusal conversion keeps the established leaf-verification
        // mapping; narrower-specific WorkLimit translation stays at its owner.
        assert_eq!(Incomplete::from_stop(stop), Incomplete::from(stop));
    }
}

#[test]
fn tight_certificate_faults_keep_their_native_class() {
    for (error, resource) in [
        (TightError::Limit(TightResource::Bytes), true),
        (TightError::UnsupportedRoot { root: 17 }, false),
        (TightError::UnsupportedBody { root: 17, body: 13 }, false),
        (TightError::PositiveCycle { atom: 11 }, false),
        (TightError::RankShape, false),
        (TightError::RankOrder { head: 7 }, false),
    ] {
        assert_settlement(
            Incomplete::Certificate(CertificateError::Tight(error)),
            resource,
        );
    }
}

#[test]
fn positive_certificate_faults_keep_their_native_class() {
    for (error, resource) in [
        (
            PositiveError::Limit {
                resource: PositiveResource::Dependencies,
                observed: u128::MAX,
                limit: 29,
            },
            true,
        ),
        (PositiveError::Overflow, true),
        (PositiveError::UnsupportedRoot { root: 19 }, false),
        (PositiveError::UnsupportedBody { root: 19, body: 13 }, false),
        (PositiveError::InvalidClosure { root: 19 }, false),
    ] {
        assert_settlement(
            Incomplete::Certificate(CertificateError::Positive(error)),
            resource,
        );
    }
}

#[test]
fn stratified_certificate_faults_keep_their_native_class() {
    for (error, resource) in [
        (
            StratifiedError::Limit {
                resource: StratifiedResource::Work,
                observed: u128::MAX - 1,
                limit: 31,
            },
            true,
        ),
        (StratifiedError::Overflow, true),
        (StratifiedError::UnsupportedRoot { root: 23 }, false),
        (
            StratifiedError::UnsupportedBody { root: 23, body: 19 },
            false,
        ),
        (StratifiedError::NegativeCycle { atom: 3, body: 19 }, false),
        (StratifiedError::InvalidClosure { root: 23 }, false),
    ] {
        assert_settlement(
            Incomplete::Certificate(CertificateError::Stratified(error)),
            resource,
        );
    }
}

#[test]
fn evaluation_certificate_storage_keeps_its_payload() {
    assert_settlement(
        Incomplete::Certificate(CertificateError::Evaluation(EvaluationError::Storage {
            required: u128::MAX - 2,
            limit: 43,
        })),
        true,
    );
}

#[test]
fn arithmetic_source_faults_keep_their_admission_class() {
    for (error, resource) in [
        (AdmissionError::Limit, true),
        (AdmissionError::Allocation, true),
        (AdmissionError::Atom, false),
        (AdmissionError::Edge, false),
        (AdmissionError::Arity, false),
        (AdmissionError::Span, false),
        (AdmissionError::ThresholdShape, false),
        (AdmissionError::NumericShape, false),
        (AdmissionError::NumericDomain, false),
        (AdmissionError::OrdinalRank, false),
        (AdmissionError::OrdinalCut, false),
        (AdmissionError::ThresholdMeasure, false),
        (AdmissionError::Root, false),
    ] {
        assert_settlement(Incomplete::ArithmeticSource(error), resource);
    }
}

#[test]
fn deadline_settlement_keeps_control_precedence() {
    for cancelled in [false, true] {
        let cancellation = Cancellation::with_deadline(Instant::now()).unwrap();
        if cancelled {
            cancellation.cancel();
        }
        let attempt = cancellation.metered(7, |_| {
            Err::<(), _>(Incomplete::ReductStorage {
                required: 47,
                limit: 43,
            })
        });
        assert_eq!(
            attempt.result,
            Err(if cancelled {
                Incomplete::Cancelled
            } else {
                Incomplete::Deadline
            })
        );
        assert_eq!(attempt.charged, 0);
    }
}

#[test]
fn completed_values_are_not_reclassified_as_refusals() {
    let cancellation = Cancellation::default();
    let retained = Incomplete::NativeNarrowingStorage {
        required: u128::MAX,
        limit: 53,
    };
    let attempt = cancellation.metered(7, |meter| {
        meter.charge().unwrap();
        cancellation.cancel();
        Ok::<_, Incomplete>(retained)
    });
    assert_eq!(attempt.result, Ok(retained));
    assert_eq!(attempt.charged, 1);
}
