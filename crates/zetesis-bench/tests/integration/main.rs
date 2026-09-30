//! Integration tests of `zetesis-bench`, compiled as one test binary.

mod benchmark_commands;
mod benchmark_comparison;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod matrix_campaign;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod performance_campaign;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod process_memory;
mod scalability_commands;
