//! Fixed-region native requirements through flat and shared Boolean cones.
//!
//! Admission, index preparation, fresh region/knowledge construction and result
//! assertions are outside timing. Each call starts from the same all-open region
//! and uses a warmed reusable scratch. Both polarities must force the observer
//! while leaving every contribution atom open. This measures one deterministic
//! closure, not optimizing search or a changed answer population.
//! Consumed region and knowledge cleanup remains inside the timed closure.

use std::hint::black_box;

use criterion::{BatchSize, Criterion, Throughput};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, FormulaNodes, FormulaParts, Narrower, Narrowing, NarrowingScratch, Node,
    NodeView, OriginalSubject, Region, RegionLimits, Theory,
};

#[derive(Clone, Copy)]
enum Shape {
    Flat,
    Wide,
    Deep,
}

impl Shape {
    fn label(self) -> &'static str {
        match self {
            Self::Flat => "flat",
            Self::Wide => "wide_shared",
            Self::Deep => "deep_shared",
        }
    }
}

fn push(nodes: &mut FormulaNodes, node: NodeView<'_>) -> usize {
    let mut transaction = nodes.transaction();
    let result = transaction.push(node, usize::MAX, usize::MAX).unwrap();
    transaction.commit();
    result
}

fn chain(nodes: &mut FormulaNodes, disjunction: bool, children: &[usize]) -> usize {
    push(
        nodes,
        if disjunction {
            NodeView::Or(children)
        } else {
            NodeView::And(children)
        },
    )
}

fn theory(shape: Shape, width: usize, disjunction: bool) -> Theory {
    let mut nodes = FormulaNodes::new(
        FormulaParts::new((0..=width).map(Node::atom).collect(), Vec::new()).unwrap(),
    );
    let conditions: Vec<_> = (0..width).collect();
    let full = match shape {
        Shape::Flat => chain(&mut nodes, disjunction, &conditions),
        Shape::Wide => {
            let inner = chain(&mut nodes, disjunction, &conditions[..2]);
            let children: Vec<_> = std::iter::once(inner).chain(2..width).collect();
            let full = chain(&mut nodes, disjunction, &children);
            push(&mut nodes, NodeView::Implies(inner, 0));
            full
        }
        Shape::Deep => {
            let mut root = chain(&mut nodes, disjunction, &conditions);
            for _ in 1..width {
                let parent = chain(&mut nodes, disjunction, &[root, 0]);
                push(&mut nodes, NodeView::Implies(root, 0));
                root = parent;
            }
            root
        }
    };
    // Explicit native ingress keeps both endpoint guards in the shared row;
    // compiler specialization to an And/Or would bypass the measured bridge.
    let guards = nodes
        .push_threshold_family(
            &conditions,
            &vec![1; width],
            &[1, width as i128],
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    let required = if disjunction {
        guards[0]
    } else {
        let falsum = push(&mut nodes, NodeView::False);
        push(&mut nodes, NodeView::Implies(guards[1], falsum))
    };
    let observer = if disjunction {
        push(&mut nodes, NodeView::Implies(full, width))
    } else {
        push(&mut nodes, NodeView::Or(&[full, width]))
    };
    Theory::new(
        width + 1,
        nodes.into_parts(),
        vec![required, observer],
        AdmissionLimits::default(),
    )
    .unwrap()
}

pub(super) fn benchmark(criterion: &mut Criterion) {
    let mut benchmark = criterion.benchmark_group("native_requirement_cones");
    let cancellation = Cancellation::default();
    for width in [32, 256] {
        for shape in [Shape::Flat, Shape::Wide, Shape::Deep] {
            for disjunction in [false, true] {
                let theory = theory(shape, width, disjunction);
                let narrower = Narrower::new(&theory);
                let mut scratch = NarrowingScratch::default();
                let mut verified = Region::all_open(theory.atom_count());
                let (outcome, receipt) = narrower
                    .narrow_known(
                        OriginalSubject::new(&theory, None),
                        &mut verified,
                        &mut narrower.knowledge(),
                        &mut scratch,
                        RegionLimits::default(),
                        &cancellation,
                    )
                    .unwrap();
                assert_eq!(outcome, Narrowing::Fixed { changed: true });
                assert_eq!(verified.decision(width), Some(true));
                assert!((0..width).all(|atom| verified.is_open(atom)));
                let connective = if disjunction { "or_true" } else { "and_false" };
                let label = format!("{}/{connective}/{width}", shape.label());
                eprintln!("native_requirement_cones {label}: {receipt:?}");
                benchmark.throughput(Throughput::Elements(1));
                benchmark.bench_function(label, |bencher| {
                    bencher.iter_batched(
                        || (Region::all_open(theory.atom_count()), narrower.knowledge()),
                        |(mut region, mut knowledge)| {
                            black_box(
                                narrower
                                    .narrow_known(
                                        OriginalSubject::new(&theory, None),
                                        &mut region,
                                        &mut knowledge,
                                        &mut scratch,
                                        RegionLimits::default(),
                                        &cancellation,
                                    )
                                    .unwrap(),
                            )
                        },
                        BatchSize::PerIteration,
                    );
                });
            }
        }
    }
    benchmark.finish();
}
