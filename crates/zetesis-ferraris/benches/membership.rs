//! Exact reference membership, including its per-call allocation and controls.
//!
//! Admission, interpretations, and verdict/work assertions are outside timers.
//! Unlike `frozen_reduct`, these fixtures call the exhaustive reference checker:
//! failed original satisfaction, the first empty countermodel, and stable
//! fact theories requiring every proper subset. This is not ordinary region or
//! clause search, and its measurements do not describe those solver routes.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, Check, Interpretation, Limits, Node, Statistics, Theory, Verdict, check,
};

#[derive(Clone, Copy)]
enum Expected {
    Stable,
    NotModel(usize),
    EmptyCountermodel,
}

struct Fixture {
    candidate: Interpretation,
    cancellation: Cancellation,
    expected: Expected,
    statistics: Statistics,
}

impl Fixture {
    fn new(theory: &Theory, atoms: &[usize], expected: Expected, statistics: Statistics) -> Self {
        Self {
            candidate: Interpretation::new(theory, atoms.iter().copied()).unwrap(),
            cancellation: Cancellation::default(),
            expected,
            statistics,
        }
    }

    fn stable(universe: usize, atoms: &[usize]) -> Self {
        assert!(!atoms.is_empty());
        let nodes = atoms.iter().copied().map(Node::Atom).collect();
        let roots = (0..atoms.len()).collect();
        let theory = Theory::new(universe, nodes, roots, AdmissionLimits::default()).unwrap();
        let count = u64::try_from(atoms.len()).unwrap();
        let states = 1_u64 << count;
        // Roots follow the counter's selected-atom order. Proper subsets make
        // 2^(k+1)-k-2 root probes and the same number of carry-bit updates;
        // their node evaluations cost k*(2^k-1), plus the original 2*k visits.
        let statistics = Statistics {
            work: u64::try_from(universe).unwrap() + count * (states - 1) + 4 * states - 4,
            subsets: states - 1,
        };
        Self::new(&theory, atoms, Expected::Stable, statistics)
    }

    fn not_model() -> Self {
        let theory =
            Theory::new(130, vec![Node::False], vec![0], AdmissionLimits::default()).unwrap();
        Self::new(
            &theory,
            &[63, 64, 65, 129],
            Expected::NotModel(0),
            Statistics {
                work: 2,
                subsets: 0,
            },
        )
    }

    fn empty_countermodel() -> Self {
        let theory = Theory::new(130, vec![], vec![], AdmissionLimits::default()).unwrap();
        Self::new(
            &theory,
            &[63, 64, 65, 129],
            Expected::EmptyCountermodel,
            Statistics {
                work: 130,
                subsets: 1,
            },
        )
    }

    fn run(&self) -> Check {
        check(
            self.candidate.theory(),
            &self.candidate,
            Limits::default(),
            &self.cancellation,
        )
        .unwrap()
    }

    fn verify(&self) {
        let result = self.run();
        assert_eq!(result.statistics(), self.statistics);
        match (self.expected, result.verdict()) {
            (Expected::Stable, Verdict::Stable) => {}
            (Expected::NotModel(expected), Verdict::NotModel { root }) => {
                assert_eq!(*root, expected);
            }
            (Expected::EmptyCountermodel, Verdict::NonMinimal { witness }) => {
                assert!(witness.atoms().next().is_none());
                assert!(witness.theory().same_instance(self.candidate.theory()));
            }
            _ => panic!(
                "unexpected reference membership verdict: {:?}",
                result.verdict()
            ),
        }
    }
}

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    let mut benchmark = criterion.benchmark_group("membership");
    benchmark.throughput(Throughput::Elements(1));
    let mut fixtures = vec![
        (
            BenchmarkId::new("not_model", "falsum"),
            Fixture::not_model(),
        ),
        (
            BenchmarkId::new("nonminimal", "empty_witness"),
            Fixture::empty_countermodel(),
        ),
        (
            BenchmarkId::new("stable", "sparse-4-of-130"),
            Fixture::stable(130, &[63, 64, 65, 129]),
        ),
    ];
    for count in [4, 8, 12] {
        fixtures.push((
            BenchmarkId::new("stable", format!("dense-{count}")),
            Fixture::stable(count, &(0..count).collect::<Vec<_>>()),
        ));
    }
    for (id, fixture) in fixtures {
        fixture.verify();
        benchmark.bench_function(id, |bencher| {
            bencher.iter(|| black_box(fixture.run()));
        });
    }
    benchmark.finish();
    criterion.final_summary();
}
