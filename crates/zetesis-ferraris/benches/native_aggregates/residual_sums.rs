//! Fixed original regions isolate residual arithmetic from candidate search.
//!
//! Theory/index preparation and region/knowledge cloning are outside timers;
//! the worker scratch is reused. Complete-valuation checks are also untimed.
//! The diagnostic reports deductions without requiring equal propagation in
//! before/after runs: a necessary check may refute earlier without changing
//! any complete model. This is not a source-grounding or solver speedup claim.

use std::{hint::black_box, time::Duration};

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, FormulaNodes, FormulaParts, Interpretation, Knowledge, Limits, Narrower,
    Narrowing, NarrowingScratch, Node, NodeView, OriginalSubject, Region, RegionLimits, Theory,
    models,
};

struct Fixture {
    name: String,
    terms: Vec<(usize, u64)>,
    bound: i128,
    theory: Theory,
    index: Narrower,
    region: Region,
    knowledge: Knowledge,
}

impl Fixture {
    fn new(
        name: String,
        atoms: usize,
        terms: Vec<(usize, u64)>,
        bound: i128,
        first: Option<bool>,
    ) -> Self {
        let mut nodes = FormulaNodes::new(
            FormulaParts::new((0..atoms).map(Node::atom).collect(), vec![]).unwrap(),
        );
        let (conditions, weights): (Vec<_>, Vec<_>) = terms.iter().copied().unzip();
        let guards = nodes
            .push_threshold_family(
                &conditions,
                &weights,
                &[bound, bound + 1],
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        let mut append = nodes.transaction();
        let falsum = append
            .push(NodeView::False, usize::MAX, usize::MAX)
            .unwrap();
        let upper = append
            .push(NodeView::Implies(guards[1], falsum), usize::MAX, usize::MAX)
            .unwrap();
        append.commit();
        let theory = Theory::new(
            atoms,
            nodes.into_parts(),
            vec![guards[0], upper],
            AdmissionLimits::default(),
        )
        .unwrap();
        let index = Narrower::new(&theory);
        let knowledge = index.knowledge();
        let mut region = Region::all_open(atoms);
        if let Some(value) = first {
            if value {
                assert!(region.hold(0));
            } else {
                assert!(region.cut(0));
            }
        }
        Self {
            name,
            terms,
            bound,
            theory,
            index,
            region,
            knowledge,
        }
    }

    fn verify(&self, control: &Cancellation) -> NarrowingScratch {
        let atoms = self.theory.atom_count();
        let mut region = self.region.clone();
        let mut scratch = NarrowingScratch::default();
        let (outcome, statistics) = self
            .index
            .narrow_known(
                OriginalSubject::new(&self.theory, None),
                &mut region,
                &mut self.knowledge.clone(),
                &mut scratch,
                RegionLimits::default(),
                control,
            )
            .unwrap();
        // Fixed complete valuations check the exact scalar weighted denotation.
        // The first three also include 17=1+6+10 for residual open/held controls.
        for pattern in 0..16 {
            let selected: Vec<_> = (0..atoms)
                .filter(|&atom| match pattern {
                    0 => false,
                    1 => true,
                    2 => atom == 0 || atom == 1 || atom == atoms.div_ceil(2),
                    _ => atom % (pattern - 1) == 0,
                })
                .collect();
            let expected = self
                .terms
                .iter()
                .filter(|&&(condition, _)| selected.binary_search(&condition).is_ok())
                .map(|&(_, weight)| i128::from(weight))
                .sum::<i128>()
                == self.bound;
            let candidate = Interpretation::new(&self.theory, selected.iter().copied()).unwrap();
            assert_eq!(
                models(&self.theory, &candidate, Limits::default(), control).unwrap(),
                expected
            );
            if expected
                && (0..atoms).all(|atom| {
                    self.region
                        .decision(atom)
                        .is_none_or(|value| value == selected.binary_search(&atom).is_ok())
                })
            {
                assert_ne!(outcome, Narrowing::Refuted);
                assert!((0..atoms).all(|atom| {
                    region
                        .decision(atom)
                        .is_none_or(|value| value == selected.binary_search(&atom).is_ok())
                }));
            }
        }
        eprintln!(
            "RESIDUAL_SUM fixture={} atoms={atoms} occurrences={} outcome={outcome:?} work={} propagations={} retained_scratch_bytes={}",
            self.name,
            self.terms.len(),
            statistics.work,
            statistics.propagations,
            scratch.retained_bytes(),
        );
        scratch
    }
}

fn fixtures() -> Vec<Fixture> {
    let mut fixtures = Vec::new();
    for size in [8, 64, 256] {
        fixtures.push(Fixture::new(
            format!("grouped_missing/{size}"),
            size,
            (0..size)
                .flat_map(|condition| [(condition, 2), (condition, 3)])
                .collect(),
            17,
            None,
        ));
        for first in [Some(false), Some(true), None] {
            let name = match first {
                Some(false) => "cut",
                Some(true) => "held",
                None => "open",
            };
            fixtures.push(Fixture::new(
                format!("residual_{name}/{size}"),
                2 * size + 1,
                std::iter::once((0, 1))
                    .chain((1..=size).map(|condition| (condition, 6)))
                    .chain((size + 1..=2 * size).map(|condition| (condition, 10)))
                    .collect(),
                17,
                first,
            ));
        }
        fixtures.push(Fixture::new(
            format!("dense/{size}"),
            size,
            (0..size)
                .map(|condition| (condition, u64::try_from(condition + 1).unwrap()))
                .collect(),
            i128::try_from(size).unwrap(),
            None,
        ));
        fixtures.push(Fixture::new(
            format!("unit/{size}"),
            size,
            (0..size).map(|condition| (condition, 1)).collect(),
            i128::try_from(size / 2).unwrap(),
            None,
        ));
    }
    fixtures.push(Fixture::new(
        "gcd_one_hole/4".into(),
        4,
        vec![(0, 6), (1, 10), (2, 15), (3, 15)],
        17,
        None,
    ));
    fixtures
}

pub(super) fn bench(criterion: &mut Criterion) {
    let control = Cancellation::default();
    let mut benchmark = criterion.benchmark_group("residual_sums");
    benchmark.sample_size(10);
    benchmark.warm_up_time(Duration::from_millis(250));
    benchmark.measurement_time(Duration::from_millis(500));
    for fixture in fixtures() {
        let mut scratch = fixture.verify(&control);
        benchmark.throughput(Throughput::Elements(
            u64::try_from(fixture.terms.len()).unwrap(),
        ));
        benchmark.bench_function(
            BenchmarkId::new("original_region", &fixture.name),
            |bencher| {
                bencher.iter_batched_ref(
                    || (fixture.region.clone(), fixture.knowledge.clone()),
                    |(region, knowledge)| {
                        black_box(
                            fixture
                                .index
                                .narrow_known(
                                    OriginalSubject::new(&fixture.theory, None),
                                    region,
                                    knowledge,
                                    &mut scratch,
                                    RegionLimits::default(),
                                    &control,
                                )
                                .unwrap(),
                        )
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    benchmark.finish();
}
