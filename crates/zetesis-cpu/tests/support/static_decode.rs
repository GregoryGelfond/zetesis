//! Decoding keeps allocation refusal distinct from invalid admitted data.

use super::decoding_stop;
use crate::Stop;
use zetesis_core::{ModelError, WordError};

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
