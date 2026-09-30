//! Several workers walk the region tree together only under the regions
//! method on a CPU backend; otherwise one worker walks it and its leaves
//! are batched.

use zetesis_solve::{Backend, SearchMethod, SolveConfig};
use zetesis_test_support::counts::nonzero as workers;

fn parallel() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        search: SearchMethod::Regions,
        workers: workers(4),
        ..SolveConfig::DEFAULT
    }
}

#[test]
fn several_workers_walk_the_regions_on_a_cpu_backend() {
    let config = SolveConfig {
        backend: Backend::Cpu,
        ..parallel()
    };
    assert_eq!(config.region_workers(), Some(workers(4)));
}

#[test]
fn one_worker_walks_the_regions_alone() {
    let config = SolveConfig {
        workers: workers(1),
        ..parallel()
    };
    assert_eq!(config.region_workers(), None);
}

#[test]
fn the_clauses_method_walks_no_regions() {
    let config = SolveConfig {
        search: SearchMethod::Clauses,
        ..parallel()
    };
    assert_eq!(config.region_workers(), None);
}

#[test]
fn a_device_backend_keeps_the_scalar_walk() {
    let config = SolveConfig {
        backend: Backend::Gpu(None),
        ..parallel()
    };
    assert_eq!(config.region_workers(), None);
}
