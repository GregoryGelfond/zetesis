//! Statistical regression measurements of the static oracles; every returned
//! closure is checked against the scalar reference.

mod fixture;

use criterion::{BenchmarkId, Criterion, Throughput};
use fixture::Family;
use std::hint::black_box;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};
use zetesis_core::{GroundProgram, Seed};
use zetesis_cpu::{BatchOracle, Cancellation, Limits, StaticCheck, check_static};

struct Case {
    program: GroundProgram,
    worlds: Vec<Vec<Seed>>,
    expected: Vec<Vec<StaticCheck>>,
}
impl Case {
    fn new(family: Family, atoms: usize, batch: usize, cancellation: &Cancellation) -> Self {
        let program = fixture::program(family, atoms);
        let worlds: Vec<_> = (0..4)
            .map(|salt| fixture::seeds(&program, batch, salt))
            .collect();
        let expected = worlds
            .iter()
            .map(|seeds| {
                seeds
                    .iter()
                    .map(|seed| {
                        check_static(&program, seed, Limits::default(), cancellation).unwrap()
                    })
                    .collect()
            })
            .collect();
        Self {
            program,
            worlds,
            expected,
        }
    }
    fn timed(
        &self,
        iterations: u64,
        pool: Option<&BatchOracle>,
        cancellation: &Cancellation,
    ) -> Duration {
        let mut elapsed = Duration::ZERO;
        for index in (0..self.worlds.len())
            .cycle()
            .take(usize::try_from(iterations).unwrap())
        {
            let started = Instant::now();
            let results: Vec<_> = if let Some(pool) = pool {
                pool.check_static_batch(
                    black_box(&self.program),
                    black_box(&self.worlds[index]),
                    Limits::default(),
                    cancellation,
                )
                .unwrap()
                .into_iter()
                .map(Result::unwrap)
                .collect()
            } else {
                self.worlds[index]
                    .iter()
                    .map(|seed| {
                        check_static(
                            black_box(&self.program),
                            black_box(seed),
                            Limits::default(),
                            cancellation,
                        )
                        .unwrap()
                    })
                    .collect()
            };
            elapsed += started.elapsed();
            assert_eq!(results.len(), self.expected[index].len());
            for (actual, expected) in results.iter().zip(&self.expected[index]) {
                assert_eq!(actual.closure_words(), expected.closure_words());
                assert_eq!(actual.accepted(), expected.accepted());
                assert_eq!(actual.constraint_violated(), expected.constraint_violated());
                assert_eq!(actual.seed_mismatch(), expected.seed_mismatch());
            }
        }
        elapsed
    }
}

fn oracles(criterion: &mut Criterion) {
    let cancellation = Cancellation::default();
    let pool = BatchOracle::new(
        NonZeroUsize::new(4).unwrap(),
        NonZeroUsize::new(64).unwrap(),
    )
    .unwrap();
    for (label, family) in [
        ("wide", Family::Wide),
        ("reverse-chain", Family::ReverseChain),
    ] {
        let mut group = criterion.benchmark_group(label);
        group
            .sample_size(20)
            .warm_up_time(Duration::from_secs(1))
            .measurement_time(Duration::from_secs(3));
        for atoms in [64, 256] {
            for batch in [1, 64] {
                let case = Case::new(family, atoms, batch, &cancellation);
                group.throughput(Throughput::Elements(u64::try_from(batch).unwrap()));
                let dimension = format!("{atoms}-consequences-{batch}-worlds");
                group.bench_with_input(
                    BenchmarkId::new("cpu-scalar", &dimension),
                    &case,
                    |bencher, case| {
                        bencher
                            .iter_custom(|iterations| case.timed(iterations, None, &cancellation));
                    },
                );
                group.bench_with_input(
                    BenchmarkId::new("cpu-rayon", &dimension),
                    &case,
                    |bencher, case| {
                        bencher.iter_custom(|iterations| {
                            case.timed(iterations, Some(&pool), &cancellation)
                        });
                    },
                );
            }
        }
        group.finish();
    }
}

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    oracles(&mut criterion);
    criterion.final_summary();
}
