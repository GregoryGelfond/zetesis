//! One fixed candidate and repeated tested interpretations; no membership search.
//!
//! `one_shot` includes a fresh freeze per query, `freeze_then_test` includes one
//! freeze per batch, and `retained_freeze` excludes freezing. All paths include
//! temporary tested-workspace allocation. Admission, interpretations and parity
//! checks are outside timers. This does not benchmark the reference checker,
//! which already freezes its candidate once per membership search.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput};
use zetesis_cpu::Control;
use zetesis_ferraris::{
    AdmissionLimits, FrozenReduct, Interpretation, Limits, Node, Theory, models_reduct,
};

const ATOMS: usize = 6;

struct Fixture {
    candidate: Interpretation,
    tested: Vec<Interpretation>,
    control: Control,
}

impl Fixture {
    fn new(depth: usize, queries: usize) -> Self {
        let mut nodes: Vec<_> = (0..ATOMS).map(Node::Atom).collect();
        for level in 0..depth {
            nodes.push(Node::Implies(nodes.len() - 1, level % ATOMS));
        }
        let roots = vec![nodes.len() - 1];
        let theory = Theory::new(ATOMS, nodes, roots, AdmissionLimits::default()).unwrap();
        Self {
            candidate: Interpretation::new(&theory, 0..ATOMS).unwrap(),
            tested: (0..queries)
                .map(|world| {
                    Interpretation::new(&theory, (0..ATOMS).filter(|atom| world & (1 << atom) != 0))
                        .unwrap()
                })
                .collect(),
            control: Control::default(),
        }
    }

    fn one_shot(&self) -> usize {
        self.tested
            .iter()
            .filter(|tested| {
                models_reduct(
                    self.candidate.theory(),
                    &self.candidate,
                    tested,
                    Limits::default(),
                    &self.control,
                )
                .unwrap()
            })
            .count()
    }

    fn freeze_then_test(&self) -> usize {
        let reduct = FrozenReduct::new(&self.candidate, Limits::default(), &self.control).unwrap();
        self.retained_freeze(&reduct)
    }

    fn retained_freeze(&self, reduct: &FrozenReduct<'_>) -> usize {
        self.tested
            .iter()
            .filter(|tested| {
                reduct
                    .is_satisfied_by(tested, Limits::default(), &self.control)
                    .unwrap()
            })
            .count()
    }

    fn verify(&self, reduct: &FrozenReduct<'_>) {
        for tested in &self.tested {
            assert_eq!(
                reduct
                    .is_satisfied_by(tested, Limits::default(), &self.control)
                    .unwrap(),
                models_reduct(
                    self.candidate.theory(),
                    &self.candidate,
                    tested,
                    Limits::default(),
                    &self.control,
                )
                .unwrap()
            );
        }
    }
}

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    let mut benchmark = criterion.benchmark_group("frozen_reduct");
    for depth in [4, 64, 256] {
        for queries in [1, 8, 32, 128] {
            let fixture = Fixture::new(depth, queries);
            let reduct =
                FrozenReduct::new(&fixture.candidate, Limits::default(), &fixture.control).unwrap();
            fixture.verify(&reduct);
            assert_eq!(fixture.one_shot(), fixture.freeze_then_test());
            assert_eq!(fixture.one_shot(), fixture.retained_freeze(&reduct));
            benchmark.throughput(Throughput::Elements(u64::try_from(queries).unwrap()));
            let label = format!("depth-{depth}-queries-{queries}");
            benchmark.bench_function(BenchmarkId::new("one_shot", &label), |bencher| {
                bencher.iter(|| black_box(fixture.one_shot()));
            });
            benchmark.bench_function(BenchmarkId::new("freeze_then_test", &label), |bencher| {
                bencher.iter(|| black_box(fixture.freeze_then_test()));
            });
            benchmark.bench_function(BenchmarkId::new("retained_freeze", &label), |bencher| {
                bencher.iter(|| black_box(fixture.retained_freeze(&reduct)));
            });
        }
    }
    benchmark.finish();
    criterion.final_summary();
}
