//! Statistical regression measurements; all returned closures are checked.

use criterion::{BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};
use zetesis_core::Seed;
use zetesis_cpu::{BatchOracle, Control, Limits, StaticCheck, check_static};
use zetesis_experiments::{BenchmarkFixture, Family};
use zetesis_wgpu::{GpuLimits, GpuOptions, GpuOracle};

struct Case {
    fixture: BenchmarkFixture,
    worlds: Vec<Vec<Seed>>,
    expected: Vec<Vec<StaticCheck>>,
}
impl Case {
    fn new(family: Family, atoms: usize, batch: usize, control: &Control) -> Self {
        let fixture = BenchmarkFixture::new(family, atoms).unwrap();
        let worlds: Vec<_> = (0..4)
            .map(|salt| fixture.seeds(batch, salt).unwrap())
            .collect();
        let expected = worlds
            .iter()
            .map(|seeds| {
                seeds
                    .iter()
                    .map(|seed| {
                        check_static(fixture.graph(), seed, Limits::default(), control).unwrap()
                    })
                    .collect()
            })
            .collect();
        Self {
            fixture,
            worlds,
            expected,
        }
    }
    fn timed_cpu(
        &self,
        iterations: u64,
        pool: Option<&BatchOracle>,
        control: &Control,
    ) -> Duration {
        let mut elapsed = Duration::ZERO;
        for index in (0..self.worlds.len())
            .cycle()
            .take(usize::try_from(iterations).unwrap())
        {
            let started = Instant::now();
            let results: Vec<_> = if let Some(pool) = pool {
                pool.check_static_batch(
                    black_box(self.fixture.graph()),
                    black_box(&self.worlds[index]),
                    Limits::default(),
                    control,
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
                            black_box(self.fixture.graph()),
                            black_box(seed),
                            Limits::default(),
                            control,
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
    fn timed_metal(&self, iterations: u64, oracle: &mut GpuOracle) -> Duration {
        let mut elapsed = Duration::ZERO;
        for index in (0..self.worlds.len())
            .cycle()
            .take(usize::try_from(iterations).unwrap())
        {
            let started = Instant::now();
            let actual = oracle
                .check_batch(
                    black_box(self.fixture.graph()),
                    black_box(&self.worlds[index]),
                    GpuLimits::default(),
                )
                .unwrap();
            elapsed += started.elapsed();
            assert_eq!(actual.len(), self.expected[index].len());
            for (value, expected) in actual.iter().zip(&self.expected[index]) {
                assert_eq!(value.closure_words(), expected.closure_words());
                assert_eq!(value.accepted(), expected.accepted());
                assert_eq!(value.constraint_violated(), expected.constraint_violated());
                assert_eq!(value.seed_mismatch(), expected.seed_mismatch());
            }
        }
        elapsed
    }
}

fn oracles(criterion: &mut Criterion) {
    let control = Control::default();
    let pool = BatchOracle::new(
        NonZeroUsize::new(4).unwrap(),
        NonZeroUsize::new(64).unwrap(),
    )
    .unwrap();
    let mut metal = std::env::var_os("ZETESIS_BENCH_METAL").map(|value| {
        assert_eq!(
            value, "1",
            "ZETESIS_BENCH_METAL must be exactly 1, or unset"
        );
        let oracle = GpuOracle::new_metal(GpuOptions::default())
            .expect("explicit physical Metal qualification");
        eprintln!("Criterion physical adapter: {:?}", oracle.info());
        oracle
    });
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
                let case = Case::new(family, atoms, batch, &control);
                group.throughput(Throughput::Elements(u64::try_from(batch).unwrap()));
                let dimension = format!("{atoms}-consequences-{batch}-worlds");
                group.bench_with_input(
                    BenchmarkId::new("cpu-scalar", &dimension),
                    &case,
                    |bencher, case| {
                        bencher
                            .iter_custom(|iterations| case.timed_cpu(iterations, None, &control));
                    },
                );
                group.bench_with_input(
                    BenchmarkId::new("cpu-rayon", &dimension),
                    &case,
                    |bencher, case| {
                        bencher.iter_custom(|iterations| {
                            case.timed_cpu(iterations, Some(&pool), &control)
                        });
                    },
                );
                if let Some(oracle) = &mut metal {
                    // Qualify and establish residency before measuring warm calls.
                    case.timed_metal(1, oracle);
                    group.bench_with_input(
                        BenchmarkId::new("metal", &dimension),
                        &case,
                        |bencher, case| {
                            bencher.iter_custom(|iterations| case.timed_metal(iterations, oracle));
                        },
                    );
                }
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
