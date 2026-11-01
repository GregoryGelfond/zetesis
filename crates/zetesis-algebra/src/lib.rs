//! Shared execution primitives: cancellation, regions, walks and packed words.
//!
//! These operations carry no program semantics. Their callers establish
//! narrowing soundness and decide answer-set membership through the reduct.
#![forbid(unsafe_code)]

mod cancellation;
pub mod class;
pub mod region;
pub mod walk;
pub mod words;

pub use class::{AnswerSetsSought, Class, Countermodels};

pub use cancellation::{
    Cancellation, CancellationPoll, CancellationRun, CancellationSlot, CancellationSlotError,
    ChargeError, Failure, LocalQuota, Meter, Metered, POLL_INTERVAL, Quota, Refusal, Stop,
    poll_due,
};
