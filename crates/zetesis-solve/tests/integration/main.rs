//! Integration tests of `zetesis-solve`, compiled as one test binary.

mod support;
mod candidate_restrictions;
mod closure_receipts;
mod closure_reservation;
mod failures;
mod hybrid;
mod language_consumers;
mod measurements;
mod memory_allowance;
mod model_construction;
mod positive_sessions;
mod projected_reference;
mod projected_sessions;
mod region_workers;
mod relational_programs;
mod retained_models;
mod retention_limits;
mod selection_sessions;
mod session_builder;
#[cfg(feature = "gpu")]
mod session_resources_gpu;
mod streaming_defaults;
mod terminal_sessions;
mod world_views;
#[cfg(feature = "gpu")]
mod world_views_gpu;
