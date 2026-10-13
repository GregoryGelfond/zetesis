//! Decoding keeps allocation refusal distinct from invalid admitted data. A
//! closure's resource refusal between metered boundaries reports itself without
//! observing a requested stop; settled where a metered operation returns, as the
//! check's own refusals are, it becomes the stop.

use super::{StaticStatistics, Work, close, decoding_stop};
use crate::{Cancellation, Limits, Meter, Stop};
use zetesis_core::{GroundProgram, ModelError, StaticLimits, WordError};
use zetesis_test_support::programs::{fact, program};

/// A checker whose meter has admitted `charged` operations, which is not a
/// metered boundary for the counts these tests use.
fn work<'m, 'c>(meter: &'m mut Meter<'c>, charged: u64, limits: Limits) -> Work<'m, 'c> {
    for _ in 0..charged {
        meter.charge().unwrap();
    }
    Work {
        limits,
        meter,
        statistics: StaticStatistics::default(),
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
    // The tick's own report, returned as a value so the exit leaves it alone.
    let tick = |stop: bool| {
        let cancellation = Cancellation::default();
        cancellation
            .metered(limits.max_work, |meter| {
                let mut work = work(meter, 7, limits);
                if stop {
                    cancellation.cancel();
                }
                Ok::<_, Stop>(work.tick())
            })
            .result
            .unwrap()
    };
    assert_eq!(tick(false), Err(Stop::WorkLimit));
    assert_eq!(tick(true), Err(Stop::Cancelled));
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
    // stop requested; the metered exit reports the stop.
    let cancellation = Cancellation::default();
    let mut closure = vec![0; graph.word_count()];
    let settled = cancellation.metered(limits.max_work, |meter| {
        let mut work = work(meter, 1, limits);
        cancellation.cancel();
        let refusal = close(&rules, &mut closure, &mut work);
        assert_eq!(refusal, Err(Stop::DerivedAtomLimit));
        refusal
    });
    assert_eq!(settled.result, Err(Stop::Cancelled));
}
