//! Integration tests of `zetesis-maintenance`, compiled as one test binary.

mod support;
mod book_artifacts;
mod coverage_policy;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod coverage_workflow;
mod documentation_links;
mod executable_agreement;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod process_capture;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod proof_capture;
mod proof_records;
mod source_inventory;
