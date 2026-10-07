//! Complete tight-plan and candidate-support construction with atomic choices.
//!
//! Admitted input theories and correctness checks are outside timers. Timed
//! calls include the actual public constructors, their work/control checks and
//! allocations. Successful returned plans/restrictions are dropped after timing;
//! refusal cleanup remains timed. Per-iteration batching retains one result.
//! Every choice uses separate Atom-node occurrences of the same semantic atom.

use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, Interpretation, Limits, Node, SupportLimits, Theory, TightError, TightPlan,
    TightPlanLimits, TightProducerKind, models, support_restriction,
};

const CHOICES: usize = 2_048;

#[derive(Clone, Copy)]
enum Order {
    Forward,
    Reversed,
    Alternating,
}

impl Order {
    fn reversed(self, atom: usize) -> bool {
        match self {
            Self::Forward => false,
            Self::Reversed => true,
            Self::Alternating => atom & 1 != 0,
        }
    }
}

struct Fixture {
    theory: Theory,
    refused: bool,
}

impl Fixture {
    fn new(order: Order, ordinary: bool, refused: bool) -> Self {
        let mut nodes = vec![Node::falsum()];
        let mut roots = Vec::new();
        if ordinary {
            let left = nodes.len();
            nodes.push(Node::atom(CHOICES));
            let right = nodes.len();
            nodes.push(Node::atom(CHOICES + 1));
            roots.push(nodes.len());
            nodes.push(Node::or_pair([left, right]));
        }
        for atom in 0..CHOICES {
            let positive = nodes.len();
            nodes.push(Node::atom(atom));
            let repeated = nodes.len();
            nodes.push(Node::atom(if refused && atom + 1 == CHOICES {
                0
            } else {
                atom
            }));
            let negative = nodes.len();
            nodes.push(Node::implies(repeated, 0));
            roots.push(nodes.len());
            nodes.push(if order.reversed(atom) {
                Node::or_pair([negative, positive])
            } else {
                Node::or_pair([positive, negative])
            });
        }
        Self {
            theory: Theory::new(
                CHOICES + 2,
                zetesis_ferraris::FormulaParts::new(nodes, vec![]).unwrap(),
                roots,
                AdmissionLimits::default(),
            )
            .unwrap(),
            refused,
        }
    }

    fn verify_tight(&self, cancellation: &Cancellation) {
        let attempt =
            TightPlan::compile_accounted(&self.theory, TightPlanLimits::default(), cancellation);
        assert!(attempt.work > 0);
        if self.refused {
            assert_eq!(
                attempt.result.unwrap_err(),
                TightError::UnsupportedRoot {
                    root: *self.theory.roots().last().unwrap(),
                }
            );
            return;
        }
        let plan = attempt.result.unwrap();
        assert!(plan.theory().same_instance(&self.theory));
        assert_eq!(plan.producers().len(), CHOICES);
        assert_eq!(plan.ranks(), vec![0; self.theory.atom_count()]);
        assert_eq!(plan.statistics().work, attempt.work);
        for (atom, (&root, producer)) in
            self.theory.roots().iter().zip(plan.producers()).enumerate()
        {
            assert_eq!(producer.head(), atom);
            assert_eq!(producer.root(), root);
            assert_eq!(producer.body(), None);
            assert_eq!(producer.kind(), TightProducerKind::Choice);
        }
    }

    fn verify_support(&self, cancellation: &Cancellation) {
        let attempt = support_restriction(&self.theory, support_limits(), cancellation);
        assert!(attempt.work > 0);
        let restriction = attempt.result.unwrap();
        if self.refused {
            assert!(restriction.is_none());
            return;
        }
        let restriction = restriction.unwrap();
        assert!(!restriction.same_instance(&self.theory));
        assert_eq!(restriction.atom_count(), self.theory.atom_count());
        assert_eq!(restriction.roots().len(), self.theory.atom_count());
        assert!(restriction.nodes().starts_with(self.theory.nodes()));
        // Choices permit either membership value. The independent ordinary
        // disjunction supplies support for one head only when the other is false.
        // Selecting neither ordinary head satisfies this necessary restriction;
        // the original asserted disjunction excludes that interpretation.
        for choices_present in [false, true] {
            for ordinary in 0..4 {
                let atoms = (0..self.theory.atom_count()).filter(|&atom| {
                    if atom < CHOICES {
                        choices_present
                    } else {
                        ordinary & (1 << (atom - CHOICES)) != 0
                    }
                });
                let candidate = Interpretation::new(&restriction, atoms).unwrap();
                assert_eq!(
                    models(&restriction, &candidate, Limits::default(), cancellation).unwrap(),
                    ordinary != 3,
                );
            }
        }
    }
}

fn support_limits() -> SupportLimits {
    SupportLimits {
        admission: AdmissionLimits::default(),
        max_work: 100_000_000,
    }
}

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    let mut benchmark = criterion.benchmark_group("tight_admission");
    benchmark.throughput(Throughput::Elements(1));
    let cancellation = Cancellation::default();

    for (name, order, refused) in [
        ("forward", Order::Forward, false),
        ("reversed", Order::Reversed, false),
        ("alternating", Order::Alternating, false),
        ("late_cross_atom", Order::Forward, true),
    ] {
        let fixture = Fixture::new(order, false, refused);
        fixture.verify_tight(&cancellation);
        benchmark.bench_function(BenchmarkId::new("compile", name), |bencher| {
            bencher.iter_batched(
                || &fixture.theory,
                |theory| {
                    black_box(TightPlan::compile(
                        black_box(theory),
                        black_box(TightPlanLimits::default()),
                        black_box(&cancellation),
                    ))
                },
                BatchSize::PerIteration,
            );
        });
    }
    for (name, order, refused) in [
        ("mixed_forward", Order::Forward, false),
        ("mixed_reversed", Order::Reversed, false),
        ("late_cross_atom", Order::Forward, true),
    ] {
        let fixture = Fixture::new(order, true, refused);
        fixture.verify_support(&cancellation);
        benchmark.bench_function(BenchmarkId::new("support", name), |bencher| {
            bencher.iter_batched(
                || &fixture.theory,
                |theory| {
                    black_box(support_restriction(
                        black_box(theory),
                        black_box(support_limits()),
                        black_box(&cancellation),
                    ))
                },
                BatchSize::PerIteration,
            );
        });
    }
    benchmark.finish();
    criterion.final_summary();
}
