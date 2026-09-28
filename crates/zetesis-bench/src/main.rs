//! Process entry point for zetesis-bench.
#![forbid(unsafe_code)]

use std::process::ExitCode;
use zetesis_validation::process::memory;

fn main() -> ExitCode {
    let mut arguments = std::env::args_os();
    // Memory rounds launch this executable as their fresh measurement helper.
    if arguments
        .nth(1)
        .is_some_and(|first| first == memory::HELPER_COMMAND)
    {
        return memory::run_helper(arguments, &mut std::io::stderr().lock());
    }
    zetesis_bench::entry()
}
