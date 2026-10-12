//! Decoding keeps allocation refusal distinct from invalid admitted data, and
//! a requested stop keeps its precedence over the checker's resource refusals.

use super::{StaticStatistics, Work, close, decoding_stop};
use crate::{Cancellation, Limits, Stop};
use zetesis_core::{GroundProgram, ModelError, StaticLimits, WordError};
use zetesis_test_support::programs::{fact, program};

/// A checker charged `work` operations so far, which is not a metered
/// boundary for the counts these tests use.
fn work(cancellation: &Cancellation, work: u64, limits: Limits) -> Work<'_> {
    Work {
        limits,
        cancellation,
        statistics: StaticStatistics {
            work,
            ..StaticStatistics::default()
        },
    }
}

#[test]
fn selection_allocation_remains_a_resource_stop() {
    // Inject the typed allocator outcome at the conversion boundary; this does
    // not force process exhaustion or claim to test the allocator itself.
    assert_eq!(
        decoding_stop(&WordError::Model(ModelError::Allocation)),
        Stop::Allocation
    );
}

#[test]
fn malformed_closures_remain_invariant_failures() {
    for error in [
        WordError::Length {
            expected: 1,
            actual: 0,
        },
        WordError::TailBits,
        WordError::Model(ModelError::Position {
            position: 1,
            atoms: 1,
        }),
    ] {
        assert_eq!(decoding_stop(&error), Stop::InvalidProgram);
    }
}

#[test]
fn a_requested_stop_precedes_the_work_limit_between_boundaries() {
    let limits = Limits {
        max_work: 7,
        ..Limits::default()
    };
    let cancellation = Cancellation::default();
    assert_eq!(work(&cancellation, 7, limits).tick(), Err(Stop::WorkLimit));
    cancellation.cancel();
    assert_eq!(work(&cancellation, 7, limits).tick(), Err(Stop::Cancelled));
}

#[test]
fn the_derived_atom_refusal_settles_to_a_requested_stop() {
    let graph =
        GroundProgram::compile(&program(vec![fact("a", vec![])]), StaticLimits::default()).unwrap();
    let rules: Vec<_> = graph.rules().iter().collect();
    let limits = Limits {
        max_derived_atoms: 0,
        ..Limits::default()
    };
    // One charged operation leaves every later charge of this short closure
    // between boundaries, so the closure reports the refusal itself even with a
    // stop requested; the check settles it at its exit.
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut closure = vec![0; graph.word_count()];
    let refusal = close(&rules, &mut closure, &mut work(&cancellation, 1, limits));
    assert_eq!(refusal, Err(Stop::DerivedAtomLimit));
    assert_eq!(cancellation.settle(refusal), Err(Stop::Cancelled));
}
