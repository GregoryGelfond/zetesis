//! Shared execution primitives: cancellation, regions, walks, packed words,
//! and the relational operators Bind, Filter, Project, Union, and Least.
//!
//! Cancellation, regions, walks and words carry no program semantics. The
//! relational operators implement the Lean relations of the same names;
//! callers still establish that a concrete grounder or GPU kernel uses them
//! soundly, and decide answer-set membership through the reduct.
#![forbid(unsafe_code)]

mod cancellation;
pub mod class;
pub mod operators;
pub mod reduction;
pub mod region;
pub mod threads;
pub mod walk;
pub mod words;

pub use class::{AnswerSetsSought, Class, Countermodels};

pub use cancellation::{
    Cancellation, CancellationPoll, CancellationRun, CancellationSlot, CancellationSlotError,
    ChargeError, Failure, LocalQuota, Meter, MeterBytes, MeterBytesError, MeterCountError,
    MeterWideCountError, MeterWorkError, Metered, POLL_INTERVAL, PermitLease, PermitSettlement,
    Quota, Refusal, Reservation, Stop, poll_due,
};

pub use operators::{
    Atoms, Bindings, ColumnLayout, Cpu, EnumerationStats, LayoutError, RelationalOps, Schedule,
    Template, TemplateError, bind, filter, least, project, rows, union,
};
