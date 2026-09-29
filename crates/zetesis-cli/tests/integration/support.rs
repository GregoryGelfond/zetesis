//! Helpers shared by the integration test modules.

pub(crate) mod bound_priority_sources;
pub(crate) mod clingo_report;
pub(crate) mod contribution_sources;
pub(crate) mod count_objective_sources;
pub(crate) mod finite_carrier_sources;
pub(crate) mod language_value_sources;
pub(crate) mod logical_extremum_sources;
#[cfg(feature = "gpu")]
pub(crate) mod physical_backend;
pub(crate) mod projected_conditional_sources;

// A helper of `zetesis-themelios`'s integration tests, compiled here too.
#[path = "../../../zetesis-themelios/tests/integration/support/source_oracle.rs"]
pub(crate) mod source_oracle;

// A helper of `zetesis-themelios`'s integration tests, compiled here too.
#[path = "../../../zetesis-themelios/tests/integration/support/source_oracle_records.rs"]
pub(crate) mod source_oracle_records;

// A helper of `zetesis-themelios`'s integration tests, compiled here too.
#[path = "../../../zetesis-themelios/tests/integration/support/source_records.rs"]
pub(crate) mod source_records;
