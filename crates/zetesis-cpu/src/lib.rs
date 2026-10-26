//! CPU primitives shared by the solver's routes: cooperative cancellation and
//! deadlines with typed incomplete outcomes, the region traversal that covers a
//! candidate space with a caller-supplied narrowing, and exact finite-table
//! selection and projection over an immutable typed relation. None of them
//! decides membership; the reduct's check stays with its callers.
#![forbid(unsafe_code)]

mod cancellation;
pub mod regions;
pub mod table;

pub use cancellation::{
    Cancellation, CancellationPoll, CancellationRun, CancellationSlot, CancellationSlotError,
    ChargeError, Meter, Metered, POLL_INTERVAL, Refusal, Stop, poll_due,
};
