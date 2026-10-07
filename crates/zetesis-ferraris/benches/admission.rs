//! Admission and immutable narrowing preparation of fixed finite formula DAGs.
//!
//! Fixture construction, input cloning and correctness assertions are outside
//! timers. Each timed call consumes its input vectors. Successful admission's
//! Arc allocation is included; its returned theory is dropped after timing.
//! Refused input vectors are dropped inside admission and remain timed.
//! `admission_topology` compares raw and retained-topology doors over identical
//! paired payloads, including wide rows. Prefix validation runs in setup; final
//! occurrence recount, atom/root checks and successful Arc publication are timed.
//! Per-iteration batching bounds retention to one input and one returned result.
//!
//! `narrower_preparation` starts from an admitted theory. It times the complete
//! public index constructor, including allocations and incidence maps, but no
//! mutable Knowledge allocation or propagation. Fixture admission and a forced
//! closure check are outside timing; each constructed index is dropped after
//! timing. Throughput counts admitted nodes, not charged propagation work.

use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionError, AdmissionLimits, AggregateFamilyLimits, FormulaNodes, FormulaParts, Narrower,
    Narrowing, NarrowingScratch, Node, NodeView, OriginalSubject, Region, RegionLimits, Theory,
    TheoryAdmission,
};

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
            nodes: (0..count).map(|atom| Node::atom(atom % ATOMS)).collect(),
            roots: vec![count - 1],
            refused: None,
        }
    }

    fn falsum(count: usize) -> Self {
        Self {
            nodes: vec![Node::falsum(); count],
            roots: vec![count - 1],
            refused: None,
        }
    }

    fn mixed(count: usize) -> Self {
        let mut nodes = vec![Node::falsum(), Node::atom(0)];
        for index in 2..count {
            nodes.push(match index % 4 {
                0 => Node::atom(index % ATOMS),
                1 => Node::and_pair([index - 1, index / 2]),
                2 => Node::or_pair([index / 2, index - 1]),
                _ => Node::implies(index - 1, 0),
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
        fixture.nodes[0] = Node::atom(ATOMS);
        fixture.refused = Some(AdmissionError::Atom);
        fixture
    }

    fn late_edge() -> Self {
        let mut fixture = Self::mixed(LARGE);
        fixture.nodes[LARGE - 1] = Node::implies(0, LARGE - 1);
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
            zetesis_ferraris::FormulaParts::new(nodes, vec![]).unwrap(),
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

/// Both setups clone identical paired storage. Only the retained route checks
/// that cloned topology before timing; it uses the existing family compiler's
/// empty request, which neither appends nodes nor changes the admitted roots.
struct TopologyFixture {
    parts: FormulaParts,
    roots: Vec<usize>,
    atoms: usize,
    refused: Option<AdmissionError>,
}

impl TopologyFixture {
    fn wide(count: usize) -> Self {
        let mut nodes = FormulaNodes::default();
        let limits = AdmissionLimits::default();
        let mut transaction = nodes.transaction();
        for index in 0..count {
            let row: Vec<_> = (0..64)
                .map(|offset| index.saturating_sub(offset + 1))
                .collect();
            let node = match index % 4 {
                0 => NodeView::Atom(index % ATOMS),
                1 => NodeView::And(&row),
                2 => NodeView::Or(&row),
                _ => NodeView::Implies(index - 1, 0),
            };
            transaction
                .push(node, limits.max_nodes, limits.max_operands)
                .unwrap();
        }
        transaction.commit();
        Self {
            parts: nodes.into_parts(),
            roots: vec![count - 1],
            atoms: ATOMS,
            refused: None,
        }
    }

    fn input(&self, retained: bool) -> TheoryAdmission {
        let mut nodes = FormulaNodes::new(
            FormulaParts::new(self.parts.nodes().to_vec(), self.parts.operands().to_vec()).unwrap(),
        );
        if retained {
            nodes
                .append_aggregate_family(
                    &[],
                    &[],
                    AggregateFamilyLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
        }
        nodes.prepare_admission(self.atoms, self.roots.clone(), AdmissionLimits::default())
    }

    fn verify(&self) {
        let raw = self.input(false);
        let checked = self.input(true);
        assert_eq!(
            raw.work() - checked.work(),
            self.parts.occurrences() as u128
        );
        let expected = raw.admit();
        let actual = checked.admit();
        if let Some(error) = self.refused {
            assert_eq!(expected.unwrap_err(), error);
            assert_eq!(actual.unwrap_err(), error);
        } else {
            for theory in [expected.unwrap(), actual.unwrap()] {
                assert_eq!(theory.atom_count(), self.atoms);
                assert_eq!(theory.nodes(), self.parts.nodes());
                assert_eq!(theory.operands(), self.parts.operands());
                assert_eq!(theory.roots(), self.roots);
            }
        }
    }
}

fn benchmark_topology(criterion: &mut Criterion) {
    let mut early_atom = TopologyFixture::wide(32);
    early_atom.atoms = 0;
    early_atom.refused = Some(AdmissionError::Atom);
    let mut late_root = TopologyFixture::wide(32);
    late_root.roots.push(32);
    late_root.refused = Some(AdmissionError::Root);
    let fixtures = [
        ("wide_32", TopologyFixture::wide(32)),
        ("wide_32768", TopologyFixture::wide(LARGE)),
        ("early_atom", early_atom),
        ("late_root", late_root),
    ];
    let mut benchmark = criterion.benchmark_group("admission_topology");
    benchmark.throughput(Throughput::Elements(1));
    for (name, fixture) in fixtures {
        fixture.verify();
        for retained in [false, true] {
            let route = if retained { "retained" } else { "raw" };
            benchmark.bench_function(BenchmarkId::new(route, name), |bencher| {
                bencher.iter_batched(
                    || fixture.input(retained),
                    |admission| black_box(admission.admit()),
                    BatchSize::PerIteration,
                );
            });
        }
    }
    benchmark.finish();
}

/// Equal-width left- and right-associated chains exercise both stack shapes.
/// Every atom has one node; admission and formula construction are not timed.
fn chain_theory(atoms: usize, conjunction: bool, reversed: bool) -> Theory {
    let mut nodes: Vec<_> = (0..atoms).map(Node::atom).collect();
    let mut root = 0;
    for atom in 1..atoms {
        let (a, b) = if reversed { (atom, root) } else { (root, atom) };
        root = nodes.len();
        nodes.push(if conjunction {
            Node::and_pair([a, b])
        } else {
            Node::or_pair([a, b])
        });
    }
    Theory::new(
        atoms,
        zetesis_ferraris::FormulaParts::new(nodes, vec![]).unwrap(),
        vec![root],
        AdmissionLimits::default(),
    )
    .unwrap()
}

/// The prepared index must force the known result, not merely allocate.
fn verify_narrower(theory: &Theory, conjunction: bool) {
    let narrower = Narrower::try_new(theory).unwrap();
    assert_eq!(
        narrower.work(),
        u64::try_from(theory.nodes().len() + theory.parts().occurrences()).unwrap()
    );
    let mut region = Region::all_open(theory.atom_count());
    if !conjunction {
        for atom in 0..theory.atom_count() - 1 {
            assert!(region.cut(atom));
        }
    }
    let (result, _) = narrower
        .narrow_known(
            OriginalSubject::new(theory, None),
            &mut region,
            &mut narrower.knowledge(),
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(result, Narrowing::Fixed { changed: true });
    assert_eq!(
        region.held().collect::<Vec<_>>(),
        if conjunction {
            (0..theory.atom_count()).collect::<Vec<_>>()
        } else {
            vec![theory.atom_count() - 1]
        }
    );
    assert_eq!(region.open().count(), 0);
}

fn benchmark_narrower(criterion: &mut Criterion) {
    let mut benchmark = criterion.benchmark_group("narrower_preparation");
    for atoms in [256, 1_024, 2_048] {
        for conjunction in [false, true] {
            for reversed in [false, true] {
                let theory = chain_theory(atoms, conjunction, reversed);
                verify_narrower(&theory, conjunction);
                let connective = if conjunction { "and" } else { "or" };
                let association = if reversed { "right" } else { "left" };
                benchmark.throughput(Throughput::Elements(
                    u64::try_from(theory.nodes().len()).unwrap(),
                ));
                let id = BenchmarkId::new(format!("{association}_{connective}"), atoms);
                benchmark.bench_function(id, |bencher| {
                    bencher.iter_batched(
                        || (),
                        |()| black_box(Narrower::try_new(black_box(&theory))),
                        BatchSize::PerIteration,
                    );
                });
            }
        }
    }
    benchmark.finish();
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
    benchmark_topology(&mut criterion);
    benchmark_narrower(&mut criterion);
    criterion.final_summary();
}
