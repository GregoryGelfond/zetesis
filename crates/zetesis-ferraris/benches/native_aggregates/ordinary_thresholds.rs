//! Fixed-workload comparison of ordinary threshold nodes and the former shared DP.
//!
//! No search, source grounding, pure-mask reducer, or benchmark-only runtime API.
//! Boolean construction transcribes the nonnegative Ge family recurrence from
//! 2fc38fb4 aggregate/family.rs: one table up to the largest bound, shared by guards.
//! Preparation uses today's checked storage/admission APIs; it is not a timing of
//! the historical aggregate compiler's validation, accounting or cancellation.

use std::{hint::black_box, time::Duration};

use criterion::{BenchmarkId, Criterion, Throughput};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison, AggregateElement, AggregateFamilyLimits, AggregateGuard,
    EvaluationLimits, EvaluationWorkspace, FormulaNodes, FormulaParts, FrozenReduct,
    Interpretation, Limits, Node, NodeView, Theory, append_aggregate_family,
};

#[path = "propagation_probe.rs"]
mod propagation_probe;

const WORLDS: usize = 4;
const PAIRS: usize = WORLDS * WORLDS;

#[derive(Clone, Copy)]
enum Representation {
    Native,
    BooleanDp,
}

impl Representation {
    fn label(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::BooleanDp => "shared_boolean_dp",
        }
    }
}

struct Row {
    elements: Vec<AggregateElement>,
    guards: Vec<AggregateGuard>,
}

impl Row {
    fn new(count: usize, weighted: bool, guards: usize) -> Self {
        let elements: Vec<_> = (0..count)
            .map(|condition| AggregateElement {
                condition,
                weight: if weighted {
                    [0, 1, 2, 3, 5, 8][condition % 6]
                } else {
                    1
                },
            })
            .collect();
        let total: i64 = elements
            .iter()
            .map(|element| i64::from(element.weight))
            .sum();
        let lower = if guards == 1 {
            total / 2
        } else {
            total / 2 - 1
        };
        let guards = (0..guards)
            .map(|offset| AggregateGuard {
                comparison: AggregateComparison::Ge,
                bound: lower + i64::try_from(offset).unwrap(),
            })
            .collect();
        Self { elements, guards }
    }

    fn prefix(&self) -> FormulaParts {
        FormulaParts::new(
            (0..self.elements.len()).map(Node::atom).collect(),
            Vec::new(),
        )
        .unwrap()
    }

    fn sum(&self, mut active: impl FnMut(usize) -> bool) -> i128 {
        self.elements.iter().fold(0, |total, element| {
            total
                + if active(element.condition) {
                    i128::from(element.weight)
                } else {
                    0
                }
        })
    }
}

fn prepare(
    row: &Row,
    prefix: FormulaParts,
    representation: Representation,
    control: &Cancellation,
) -> Theory {
    let mut nodes = FormulaNodes::new(prefix);
    let roots = match representation {
        Representation::Native => append_aggregate_family(
            &mut nodes,
            &row.elements,
            &row.guards,
            AggregateFamilyLimits::default(),
            control,
        )
        .unwrap()
        .roots()
        .to_vec(),
        Representation::BooleanDp => boolean_family(&mut nodes, row),
    };
    let theory = Theory::new(
        row.elements.len(),
        nodes.into_parts(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap();
    match representation {
        Representation::Native => {
            assert_eq!(theory.parts().rows().len(), 1);
            assert_eq!(theory.parts().thresholds().len(), row.guards.len());
        }
        Representation::BooleanDp => assert!(theory.parts().thresholds().is_empty()),
    }
    theory
}

fn boolean_family(nodes: &mut FormulaNodes, row: &Row) -> Vec<usize> {
    let mut transaction = nodes.transaction();
    let limits = AdmissionLimits::default();
    let mut push = |node: NodeView<'_>| {
        transaction
            .push(node, limits.max_nodes, limits.max_operands)
            .unwrap()
    };
    let falsum = push(NodeView::False);
    let truth = push(NodeView::Implies(falsum, falsum));
    let maximum =
        usize::try_from(row.guards.iter().map(|guard| guard.bound).max().unwrap()).unwrap();
    let mut previous = vec![falsum; maximum + 1];
    let mut current = previous.clone();
    previous[0] = truth;
    for element in &row.elements {
        if element.weight == 0 {
            continue;
        }
        let weight = usize::try_from(element.weight).unwrap();
        current[0] = truth;
        for (target, cell) in current.iter_mut().enumerate().skip(1) {
            let included = push(NodeView::And(&[
                element.condition,
                previous[target.saturating_sub(weight)],
            ]));
            *cell = push(NodeView::Or(&[previous[target], included]));
        }
        std::mem::swap(&mut previous, &mut current);
    }
    let roots = row
        .guards
        .iter()
        .map(|guard| previous[usize::try_from(guard.bound).unwrap()])
        .collect();
    transaction.commit();
    roots
}

fn interpretation(theory: &Theory, active: impl Fn(usize) -> bool) -> Interpretation {
    Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|&atom| active(atom)),
    )
    .unwrap()
}

fn pattern(world: usize, atom: usize) -> bool {
    match world {
        0 => false,
        1 => true,
        2 => atom.is_multiple_of(2),
        _ => !atom.is_multiple_of(3),
    }
}

fn original_batch(
    workspace: &mut EvaluationWorkspace,
    worlds: &[Interpretation; WORLDS],
    control: &Cancellation,
) -> [(bool, u64); WORLDS] {
    std::array::from_fn(|index| {
        let attempt = workspace.evaluate(
            black_box(&worlds[index]),
            EvaluationLimits::default(),
            control,
        );
        (attempt.result.unwrap().is_model(), attempt.work)
    })
}

fn freeze_batch<'a>(
    worlds: &'a [Interpretation; WORLDS],
    control: &Cancellation,
) -> [FrozenReduct<'a>; WORLDS] {
    std::array::from_fn(|index| {
        FrozenReduct::new(black_box(&worlds[index]), Limits::default(), control).unwrap()
    })
}

fn query_batch(
    frozen: &[FrozenReduct<'_>; WORLDS],
    worlds: &[Interpretation; WORLDS],
    control: &Cancellation,
) -> [bool; PAIRS] {
    std::array::from_fn(|index| {
        frozen[index / WORLDS]
            .is_satisfied_by(
                black_box(&worlds[index % WORLDS]),
                Limits::default(),
                control,
            )
            .unwrap()
    })
}

/// All M,J over six atoms, including J outside M, every guard separately.
/// The independent expected result is the direct nonnegative sum; with atomic
/// conditions, recursively frozen eligibility is exactly membership in M AND J.
fn exhaustive_parity(control: &Cancellation) {
    for weighted in [false, true] {
        let row = Row::new(6, weighted, 4);
        for representation in [Representation::Native, Representation::BooleanDp] {
            let theory = prepare(&row, row.prefix(), representation, control);
            for (&root, guard) in theory.roots().iter().zip(&row.guards) {
                let single = Theory::new(
                    theory.atom_count(),
                    theory.parts().try_clone().unwrap(),
                    vec![root],
                    AdmissionLimits::default(),
                )
                .unwrap();
                let worlds: Vec<_> = (0usize..64)
                    .map(|bits| interpretation(&single, |atom| bits & (1 << atom) != 0))
                    .collect();
                let mut workspace = EvaluationWorkspace::default();
                for (m, candidate) in worlds.iter().enumerate() {
                    let sum_m = row.sum(|atom| m & (1 << atom) != 0);
                    let expected_original = sum_m >= i128::from(guard.bound);
                    let actual =
                        workspace.evaluate(candidate, EvaluationLimits::default(), control);
                    assert_eq!(actual.result.unwrap().is_model(), expected_original);
                    let frozen = FrozenReduct::new(candidate, Limits::default(), control).unwrap();
                    for (j, tested) in worlds.iter().enumerate() {
                        let sum_frozen = row.sum(|atom| (m & j) & (1 << atom) != 0);
                        let expected = expected_original && sum_frozen >= i128::from(guard.bound);
                        assert_eq!(
                            frozen
                                .is_satisfied_by(tested, Limits::default(), control)
                                .unwrap(),
                            expected
                        );
                    }
                }
            }
        }
    }
}

pub(super) fn bench(criterion: &mut Criterion) {
    propagation_probe::run();
    eprintln!(
        "FORMULA_LAYOUT node_bytes={} view_bytes={} view_alignment={}",
        size_of::<Node>(),
        size_of::<NodeView<'_>>(),
        align_of::<NodeView<'_>>(),
    );
    let control = Cancellation::default();
    exhaustive_parity(&control);
    let mut benchmark = criterion.benchmark_group("ordinary_thresholds");
    benchmark.sample_size(10);
    benchmark.warm_up_time(Duration::from_millis(250));
    benchmark.measurement_time(Duration::from_millis(500));
    for weighted in [false, true] {
        for count in [8, 64] {
            for guards in [1, 4] {
                let row = Row::new(count, weighted, guards);
                let prefix = row.prefix();
                let kind = if weighted { "sum" } else { "count" };
                for representation in [Representation::Native, Representation::BooleanDp] {
                    let label = format!("{kind}-{count}-guards{guards}/{}", representation.label());
                    let theory =
                        prepare(&row, prefix.try_clone().unwrap(), representation, &control);
                    let worlds = std::array::from_fn(|world| {
                        interpretation(&theory, |atom| pattern(world, atom))
                    });
                    let mut workspace = EvaluationWorkspace::default();
                    workspace
                        .reserve(&theory, EvaluationLimits::default().max_bytes, &control)
                        .unwrap();
                    let expected_original: [bool; WORLDS] = std::array::from_fn(|world| {
                        let sum = row.sum(|atom| pattern(world, atom));
                        row.guards
                            .iter()
                            .all(|guard| sum >= i128::from(guard.bound))
                    });
                    let original = original_batch(&mut workspace, &worlds, &control);
                    assert_eq!(original.map(|(truth, _)| truth), expected_original);
                    let frozen = freeze_batch(&worlds, &control);
                    let expected_frozen: [bool; PAIRS] = std::array::from_fn(|pair| {
                        let m = pair / WORLDS;
                        let j = pair % WORLDS;
                        let sum = row.sum(|atom| pattern(m, atom) && pattern(j, atom));
                        expected_original[m]
                            && row
                                .guards
                                .iter()
                                .all(|guard| sum >= i128::from(guard.bound))
                    });
                    assert_eq!(query_batch(&frozen, &worlds, &control), expected_frozen);
                    eprintln!(
                        "ORDINARY_THRESHOLD fixture={label} nodes={} occurrences={} rows={} descriptors={} formula_capacity_bytes={} original_workspace_bytes={} frozen_mask_cells={} original_batch_work={}",
                        theory.nodes().len(),
                        theory.parts().occurrences(),
                        theory.parts().rows().len(),
                        theory.parts().thresholds().len(),
                        theory.parts().retained_bytes(),
                        workspace.retained_bytes(),
                        WORLDS * theory.nodes().len(),
                        original.iter().map(|(_, work)| work).sum::<u64>()
                    );
                    benchmark.throughput(Throughput::Elements(u64::try_from(WORLDS).unwrap()));
                    benchmark.bench_function(
                        BenchmarkId::new("original_reused_workspace", &label),
                        |bencher| {
                            bencher.iter(|| {
                                black_box(original_batch(&mut workspace, &worlds, &control))
                            });
                        },
                    );
                    benchmark.throughput(Throughput::Elements(u64::try_from(PAIRS).unwrap()));
                    benchmark.bench_function(
                        BenchmarkId::new("frozen_public_query", &label),
                        |bencher|
                        // This public query allocates its tested truth workspace on every call.
                        bencher.iter(|| black_box(query_batch(&frozen, &worlds, &control))),
                    );
                }
            }
        }
    }
    benchmark.finish();
}
