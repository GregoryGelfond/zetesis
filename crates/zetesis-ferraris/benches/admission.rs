//! Admission of fixed finite formula DAGs, including structural refusals.
//!
//! Fixture construction, input cloning and correctness assertions are outside
//! timers. Each timed call consumes its input vectors. Successful admission's
//! Arc allocation is included; its returned theory is dropped after timing.
//! Refused input vectors are dropped inside `Theory::new` and remain timed.
//! Per-iteration batching bounds retention to one input and one returned result.

use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use zetesis_ferraris::{AdmissionError, AdmissionLimits, Node, Theory};

const ATOMS: usize = 256;
const LARGE: usize = 32_768;

struct Fixture {
    nodes: Vec<Node>,
    roots: Vec<usize>,
    refused: Option<AdmissionError>,
}

impl Fixture {
    fn atoms(count: usize) -> Self {
        Self {
            nodes: (0..count).map(|atom| Node::Atom(atom % ATOMS)).collect(),
            roots: vec![count - 1],
            refused: None,
        }
    }

    fn falsum(count: usize) -> Self {
        Self {
            nodes: vec![Node::False; count],
            roots: vec![count - 1],
            refused: None,
        }
    }

    fn mixed(count: usize) -> Self {
        let mut nodes = vec![Node::False, Node::Atom(0)];
        for index in 2..count {
            nodes.push(match index % 4 {
                0 => Node::Atom(index % ATOMS),
                1 => Node::And(index - 1, index / 2),
                2 => Node::Or(index / 2, index - 1),
                _ => Node::Implies(index - 1, 0),
            });
        }
        Self {
            nodes,
            roots: vec![count - 1],
            refused: None,
        }
    }

    fn early_atom() -> Self {
        let mut fixture = Self::mixed(LARGE);
        fixture.nodes[0] = Node::Atom(ATOMS);
        fixture.refused = Some(AdmissionError::Atom);
        fixture
    }

    fn late_edge() -> Self {
        let mut fixture = Self::mixed(LARGE);
        fixture.nodes[LARGE - 1] = Node::Implies(0, LARGE - 1);
        fixture.refused = Some(AdmissionError::Edge);
        fixture
    }

    fn roots(refused: bool) -> Self {
        let mut fixture = Self::mixed(32);
        fixture.roots = (0..LARGE).map(|root| root % 32).collect();
        if refused {
            fixture.roots[LARGE - 1] = fixture.nodes.len();
            fixture.refused = Some(AdmissionError::Root);
        }
        fixture
    }

    fn input(&self) -> (Vec<Node>, Vec<usize>) {
        (self.nodes.clone(), self.roots.clone())
    }

    fn admit((nodes, roots): (Vec<Node>, Vec<usize>)) -> Result<Theory, AdmissionError> {
        Theory::new(
            black_box(ATOMS),
            nodes,
            roots,
            black_box(AdmissionLimits::default()),
        )
    }

    fn verify(&self) {
        let result = Self::admit(self.input());
        if let Some(expected) = self.refused {
            assert_eq!(result.unwrap_err(), expected);
        } else {
            let theory = result.unwrap();
            assert_eq!(theory.atom_count(), ATOMS);
            assert_eq!(theory.nodes(), self.nodes);
            assert_eq!(theory.roots(), self.roots);
        }
    }
}

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    let mut benchmark = criterion.benchmark_group("admission");
    benchmark.throughput(Throughput::Elements(1));
    let fixtures = [
        (BenchmarkId::new("atoms", 32), Fixture::atoms(32)),
        (BenchmarkId::new("atoms", LARGE), Fixture::atoms(LARGE)),
        (BenchmarkId::new("falsum", LARGE), Fixture::falsum(LARGE)),
        (BenchmarkId::new("mixed", 32), Fixture::mixed(32)),
        (BenchmarkId::new("mixed", LARGE), Fixture::mixed(LARGE)),
        (
            BenchmarkId::new("refused", "early_atom"),
            Fixture::early_atom(),
        ),
        (
            BenchmarkId::new("refused", "late_edge"),
            Fixture::late_edge(),
        ),
        (BenchmarkId::new("roots", "repeated"), Fixture::roots(false)),
        (
            BenchmarkId::new("roots", "late_invalid"),
            Fixture::roots(true),
        ),
    ];
    for (id, fixture) in fixtures {
        fixture.verify();
        benchmark.bench_function(id, |bencher| {
            bencher.iter_batched(
                || fixture.input(),
                |input| black_box(Fixture::admit(input)),
                BatchSize::PerIteration,
            );
        });
    }
    benchmark.finish();
    criterion.final_summary();
}
