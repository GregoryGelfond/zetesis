//! Distinct sparse/dense frozen seeds through three concrete CPU routes.
//! Construction and exact ordered closure validation are outside each timer.
//! All routes reuse one owned Rayon pool; each call creates fresh source state.

use criterion::{BenchmarkId, Criterion, Throughput};
use std::{
    hint::black_box,
    num::NonZeroUsize,
    time::{Duration, Instant},
};
use zetesis_core::{AdmissionLimits, Model, Program, Seed, Template, Term, Value};
use zetesis_cpu::lazy::{SourceSelection, shared};
use zetesis_cpu::{BatchOracle, Cancellation, Limits};
use zetesis_test_support::programs::{numbered as atom, pattern};

struct Case {
    program: Program,
    seeds: Vec<Seed>,
    expected: Vec<(Model, bool)>,
}

impl Case {
    fn new(width: i32, dense: bool) -> Self {
        let facts: Vec<_> = (0..width).map(|value| atom("d", &[value])).collect();
        let mut rules: Vec<_> = (0..width)
            .map(|value| {
                Template::new(
                    Some(pattern("d", vec![Term::Constant(Value::Number(value))])),
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                )
            })
            .collect();
        let p = pattern("p", vec![Term::Variable(0)]);
        rules.push(Template::new(
            Some(p.clone()),
            vec![pattern("d", vec![Term::Variable(0)])],
            vec![p.clone()],
            vec![],
            vec![],
        ));
        rules.push(Template::new(
            Some(pattern("pair", vec![Term::Variable(0), Term::Variable(1)])),
            vec![p, pattern("p", vec![Term::Variable(1)])],
            vec![],
            vec![],
            vec![],
        ));
        rules.push(Template::new(
            None,
            vec![pattern(
                "pair",
                vec![
                    Term::Constant(Value::Number(0)),
                    Term::Constant(Value::Number(1)),
                ],
            )],
            vec![],
            vec![],
            vec![],
        ));
        let program = Program::new(rules, AdmissionLimits::default()).unwrap();
        let mut seeds = Vec::new();
        let mut expected = Vec::new();
        for omitted in 0..width {
            let selected: Vec<_> = (0..width)
                .filter(|value| (*value == omitted) != dense)
                .collect();
            let seed =
                Seed::new(&program, selected.iter().map(|value| atom("p", &[*value]))).unwrap();
            let mut closure = facts.clone();
            closure.extend(selected.iter().map(|value| atom("p", &[*value])));
            for left in &selected {
                for right in &selected {
                    closure.push(atom("pair", &[*left, *right]));
                }
            }
            let violated = selected.contains(&0) && selected.contains(&1);
            seeds.push(seed);
            expected.push((Model::new(closure).unwrap(), violated));
        }
        Self {
            program,
            seeds,
            expected,
        }
    }

    fn check(&self, pool: &BatchOracle, route: Route, cancellation: &Cancellation) -> Results {
        match route {
            Route::Independent => Results::Independent(
                pool.check_batch(
                    black_box(&self.program),
                    black_box(&self.seeds),
                    Limits::default(),
                    cancellation,
                )
                .unwrap(),
            ),
            Route::Shared(selection) => Results::Shared(
                pool.check_shared(
                    black_box(&self.program),
                    black_box(&self.seeds),
                    shared::Limits::default(),
                    selection,
                    cancellation,
                )
                .unwrap(),
            ),
        }
    }

    fn validate(&self, results: &Results) {
        match results {
            Results::Independent(checks) => {
                assert_eq!(checks.len(), self.expected.len());
                for (check, (closure, violated)) in checks.iter().zip(&self.expected) {
                    let check = check.as_ref().unwrap();
                    assert_eq!(check.closure(), closure);
                    assert_eq!(check.constraint_violated(), *violated);
                    assert!(!check.seed_mismatch());
                    assert_eq!(check.accepted(), !violated);
                }
            }
            Results::Shared(batch) => {
                assert_eq!(batch.checks.len(), self.expected.len());
                assert_eq!(batch.statistics.worlds.len(), self.expected.len());
                for (check, (closure, violated)) in batch.checks.iter().zip(&self.expected) {
                    assert!(check.program().same_instance(&self.program));
                    assert_eq!(check.closure(), closure);
                    assert_eq!(check.constraint_violated(), *violated);
                    assert!(!check.seed_mismatch());
                    assert_eq!(check.accepted(), !violated);
                }
            }
        }
    }

    fn timed(
        &self,
        iterations: u64,
        pool: &BatchOracle,
        route: Route,
        cancellation: &Cancellation,
    ) -> Duration {
        let mut elapsed = Duration::ZERO;
        for _ in 0..iterations {
            let start = Instant::now();
            let results = self.check(pool, route, cancellation);
            elapsed += start.elapsed();
            self.validate(&results);
        }
        elapsed
    }
}

#[derive(Clone, Copy)]
enum Route {
    Independent,
    Shared(SourceSelection),
}
enum Results {
    Independent(Vec<Result<zetesis_cpu::Check, zetesis_cpu::Stop>>),
    Shared(shared::Batch),
}

pub(super) fn benchmarks(criterion: &mut Criterion) {
    let cancellation = Cancellation::default();
    let pool = BatchOracle::new(
        NonZeroUsize::new(4).unwrap(),
        NonZeroUsize::new(32).unwrap(),
    )
    .unwrap();
    let routes = [
        ("independent-rayon-4", Route::Independent),
        ("union-rayon-4", Route::Shared(SourceSelection::Union)),
        ("worlds-rayon-4", Route::Shared(SourceSelection::Worlds)),
    ];
    let mut group = criterion.benchmark_group("shared-source-distinct");
    group
        .sample_size(20)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    for width in [8, 32] {
        for (density, dense) in [("sparse", false), ("dense", true)] {
            let case = Case::new(width, dense);
            let size = format!("{density}-{width}-domain-{}-worlds", case.seeds.len());
            group.throughput(Throughput::Elements(
                u64::try_from(case.seeds.len()).unwrap(),
            ));
            for (name, route) in routes {
                let qualified = case.check(&pool, route, &cancellation);
                case.validate(&qualified);
                if let Results::Independent(checks) = &qualified {
                    // At most 32 complete checks, each bounded by 10 million
                    // operations; these sums cannot exceed the u64 carrier.
                    eprintln!(
                        "independent qualification {name}/{size}: occurrences={}; join_copy_work={}; enabled_bindings={}",
                        checks.len(),
                        checks
                            .iter()
                            .map(|check| check.as_ref().unwrap().statistics().work)
                            .sum::<u64>(),
                        checks
                            .iter()
                            .map(|check| check.as_ref().unwrap().statistics().bindings)
                            .sum::<u64>()
                    );
                }
                if let Results::Shared(batch) = &qualified {
                    eprintln!(
                        "shared qualification {name}/{size}: occurrences={}; source={:?}; world_work={}; world_instances={}",
                        batch.checks.len(),
                        batch.statistics.source,
                        batch
                            .statistics
                            .worlds
                            .iter()
                            .map(|world| world.work)
                            .sum::<u64>(),
                        batch
                            .statistics
                            .worlds
                            .iter()
                            .map(|world| world.instances)
                            .sum::<u64>()
                    );
                }
            }
            // Establish all three concrete routes before timing this fixture.
            for (name, route) in routes {
                group.bench_with_input(BenchmarkId::new(name, &size), &case, |bencher, case| {
                    bencher.iter_custom(|iterations| {
                        case.timed(iterations, &pool, route, &cancellation)
                    });
                });
            }
        }
    }
    group.finish();
}
