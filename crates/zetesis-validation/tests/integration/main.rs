//! Integration tests of `zetesis-validation`, compiled as one test binary.

mod support;
mod authored_examples;
mod authored_workloads;
mod cli_contracts;
mod comparison_reports;
mod corpus_comparison;
mod curated_corpus;
mod example_corpus;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod example_parity;
mod matrix_schedule;
mod native_invocation;
mod native_json_answers;
mod performance_families;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod process_capture;
mod process_executable;
mod reported_answers;
mod scalability_limits;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod selected_campaign;
mod series_cells;
mod series_view;
mod workload_admission;
