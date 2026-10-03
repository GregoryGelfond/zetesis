//! Construction of packed interpretations from borrowed, owned and computed input.
//!
//! Theory admission, fixtures, input cloning and correctness checks are outside
//! timers. Each timed call includes word allocation/initialization and insertion;
//! an owned input is also destroyed within that call. The returned result is
//! dropped after timing, so successful output destruction is excluded while
//! refusal cleanup remains timed. Per-iteration batching bounds retained inputs
//! and results to one invocation.

use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use zetesis_ferraris::{AdmissionError, AdmissionLimits, Interpretation, Theory};

const LARGE: usize = 32_768;

fn theory(atoms: usize) -> Theory {
    Theory::new(atoms, vec![], vec![], AdmissionLimits::default()).unwrap()
}

fn verify(
    theory: &Theory,
    result: Result<Interpretation, AdmissionError>,
    expected: Option<&[usize]>,
) {
    if let Some(expected) = expected {
        let selected = result.unwrap();
        assert!(theory.same_instance(selected.theory()));
        assert_eq!(selected.atoms().collect::<Vec<_>>(), expected);
    } else {
        assert_eq!(result.unwrap_err(), AdmissionError::Atom);
    }
}

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    let mut benchmark = criterion.benchmark_group("interpretation");
    benchmark.throughput(Throughput::Elements(1));

    let dense: Vec<_> = (0..LARGE).collect();
    let mut early = dense.clone();
    early[0] = LARGE;
    let mut late = dense.clone();
    late[LARGE - 1] = LARGE;
    let borrowed = [
        ("small", 65, vec![64, 0, 32, 64], Some(vec![0, 32, 64])),
        ("dense", LARGE, dense.clone(), Some(dense.clone())),
        ("first_invalid", LARGE, early, None),
        ("last_invalid", LARGE, late, None),
    ];
    for (name, atoms, input, expected) in borrowed {
        let theory = theory(atoms);
        verify(
            &theory,
            Interpretation::new(&theory, input.iter().copied()),
            expected.as_deref(),
        );
        benchmark.bench_function(BenchmarkId::new("slice", name), |bencher| {
            bencher.iter_batched(
                || input.as_slice(),
                |input| {
                    black_box(Interpretation::new(
                        black_box(&theory),
                        black_box(input).iter().copied(),
                    ))
                },
                BatchSize::PerIteration,
            );
        });
    }

    let theory = theory(LARGE);
    verify(
        &theory,
        Interpretation::new(&theory, dense.clone()),
        Some(&dense),
    );
    benchmark.bench_function(BenchmarkId::new("vec", "dense"), |bencher| {
        bencher.iter_batched(
            || dense.clone(),
            |input| black_box(Interpretation::new(black_box(&theory), black_box(input))),
            BatchSize::PerIteration,
        );
    });

    let even: Vec<_> = (0..LARGE).step_by(2).collect();
    verify(
        &theory,
        Interpretation::new(&theory, (0..LARGE).filter(|atom| atom & 1 == 0)),
        Some(&even),
    );
    benchmark.bench_function(BenchmarkId::new("filter", "even"), |bencher| {
        bencher.iter_batched(
            || (0..LARGE).filter(|atom| atom & 1 == 0),
            |input| black_box(Interpretation::new(black_box(&theory), black_box(input))),
            BatchSize::PerIteration,
        );
    });

    benchmark.finish();
    criterion.final_summary();
}
