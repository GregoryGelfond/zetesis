//! Typed reconstruction receipts retain bounded serialization and error identity.

use super::{Buffer, reconstruction_kind, terminal_statistics};
use zetesis_core::{ModelError, ModelFailure};
use zetesis_solve::TerminalExecutionStatistics;
use zetesis_themelios::{ReconstructionError, ReconstructionStatistics};

#[test]
fn reconstruction_limits_do_not_reclassify_allocator_or_identity_failures() {
    assert_eq!(
        reconstruction_kind(&ReconstructionError::Model(ModelFailure::Model(
            ModelError::Bytes {
                required: 8,
                limit: 7
            }
        ),)),
        "answer_reconstruction_limit"
    );
    for error in [
        ReconstructionError::ForeignInput,
        ReconstructionError::Failed,
        ReconstructionError::Model(ModelFailure::Model(ModelError::Allocation)),
    ] {
        assert_eq!(reconstruction_kind(&error), "answer_reconstruction");
    }
}

#[test]
fn terminal_counters_serialize_zero_and_full_width_without_losing_the_bound() {
    for count in [0, u64::MAX] {
        let statistics = TerminalExecutionStatistics {
            base_answers: count,
            reconstructed: count,
            pending: 0,
            reconstruction: ReconstructionStatistics {
                attempts: count,
                completed: count,
                work: count,
                substitutions: count,
                latest_work: count,
                latest_substitutions: count,
            },
        };
        let mut complete = Buffer::new(1024);
        terminal_statistics(&mut complete, Some(&statistics)).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&complete.bytes).unwrap();
        assert_eq!(
            value,
            serde_json::json!({"base_answers":count,"reconstructed":count,
            "pending":0,"reconstruction":{"attempts":count,"completed":count,
                "work":count,"substitutions":count}})
        );
        for maximum in 0..complete.bytes.len() {
            let mut refused = Buffer::new(maximum);
            assert!(terminal_statistics(&mut refused, Some(&statistics)).is_err());
            assert!(refused.bytes.len() <= maximum);
        }
    }
    let mut absent = Buffer::new(4);
    terminal_statistics(&mut absent, None).unwrap();
    assert_eq!(absent.bytes, b"null");
}
