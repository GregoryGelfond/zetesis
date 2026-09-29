//! Sparse and dense leading-key joins through the public lazy oracle.
//!
//! Timings include complete reduct closure, work accounting and result creation.
//! Program construction and result validation are outside the measured interval.
//! Repeated empty seeds deliberately isolate membership work; this is neither
//! an enumeration benchmark nor an end-to-end source/CLI comparison.

mod shared_source;

use std::{
    hint::black_box,
    num::NonZeroUsize,
    time::{Duration, Instant},
};

use criterion::{BenchmarkId, Criterion, Throughput};
use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Model, Predicate, Program, Seed, Template, Term, Value,
};
use zetesis_cpu::{BatchOracle, Cancellation, Limits, check};

struct Case {
    program: Program,
    seeds: Vec<Seed>,
    expected: Model,
}

fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms).unwrap()
}

fn number(value: i32) -> Term {
    Term::Constant(Value::Number(value))
}

impl Case {
    fn new(rows: i32, distinct_keys: bool, batch: usize) -> Self {
        let mut expected = Vec::new();
        let mut rules = Vec::new();
        let mut fact = |name: &str, values: Vec<i32>| {
            expected.push(
                Atom::new(
                    Predicate::new(name, values.len()).unwrap(),
                    values.iter().copied().map(Value::Number).collect(),
                )
                .unwrap(),
            );
            rules.push(Template::new(
                Some(pattern(name, values.into_iter().map(number).collect())),
                vec![],
                vec![],
                vec![],
                vec![],
            ));
        };
        for key in 0..if distinct_keys { rows } else { 1 } {
            fact("selected", vec![key]);
        }
        for row in 0..rows {
            fact("relation", vec![if distinct_keys { row } else { 0 }, row]);
        }
        let x = Term::Variable(0);
        let y = Term::Variable(1);
        rules.push(Template::new(
            Some(pattern("result", vec![x.clone(), y.clone()])),
            vec![
                pattern("selected", vec![x.clone()]),
                pattern("relation", vec![x, y]),
            ],
            vec![],
            vec![],
            vec![],
        ));
        for row in 0..rows {
            expected.push(
                Atom::new(
                    Predicate::new("result", 2).unwrap(),
                    vec![
                        Value::Number(if distinct_keys { row } else { 0 }),
                        Value::Number(row),
                    ],
                )
                .unwrap(),
            );
        }
        let program = Program::new(rules, AdmissionLimits::default()).unwrap();
        let seed = Seed::new(&program, []).unwrap();
        Self {
            program,
            seeds: vec![seed; batch],
            expected: Model::new(expected).unwrap(),
        }
    }

    fn timed(
        &self,
        iterations: u64,
        pool: Option<&BatchOracle>,
        cancellation: &Cancellation,
    ) -> Duration {
        let mut elapsed = Duration::ZERO;
        for _ in 0..iterations {
            let started = Instant::now();
            let results = if let Some(pool) = pool {
                pool.check_batch(
                    black_box(&self.program),
                    black_box(&self.seeds),
                    Limits::default(),
                    cancellation,
                )
                .unwrap()
            } else {
                self.seeds
                    .iter()
                    .map(|seed| {
                        check(
                            black_box(&self.program),
                            black_box(seed),
                            Limits::default(),
                            cancellation,
                        )
                    })
                    .collect()
            };
            elapsed += started.elapsed();
            assert_eq!(results.len(), self.seeds.len());
            for result in results {
                let checked = result.unwrap();
                assert!(checked.accepted());
                assert_eq!(checked.closure(), &self.expected);
            }
        }
        elapsed
    }
}

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    let cancellation = Cancellation::default();
    let pool =
        BatchOracle::new(NonZeroUsize::new(4).unwrap(), NonZeroUsize::new(8).unwrap()).unwrap();
    for (name, distinct_keys) in [("sparse-keys", true), ("dense-key", false)] {
        let mut group = criterion.benchmark_group(name);
        group
            .sample_size(20)
            .warm_up_time(Duration::from_secs(1))
            .measurement_time(Duration::from_secs(3));
        for rows in [32, 256] {
            for batch in [1, 8] {
                let case = Case::new(rows, distinct_keys, batch);
                group.throughput(Throughput::Elements(u64::try_from(batch).unwrap()));
                let size = format!("{rows}-rows-{batch}-worlds");
                for (backend, pool) in [("scalar", None), ("rayon-4", Some(&pool))] {
                    group.bench_with_input(
                        BenchmarkId::new(backend, &size),
                        &case,
                        |bencher, case| {
                            bencher.iter_custom(|iterations| {
                                case.timed(iterations, pool, &cancellation)
                            });
                        },
                    );
                }
            }
        }
        group.finish();
    }
    shared_source::benchmarks(&mut criterion);
    criterion.final_summary();
}
