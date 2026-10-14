//! Fixed-theory tight checking isolates global dispatch and one-row overhead.
//!
//! Every fixture has the same Boolean prefix and candidate outcomes. Only its
//! final test changes: Boolean disjunction, one weighted threshold, or one
//! ordinal cut. Unused raw numeric storage is a fourth, Boolean-only control.
//! The public checker still visits the complete admitted DAG. Construction,
//! certificate compilation, interpretations and correctness checks are untimed.

use std::{hint::black_box, time::Duration};

use criterion::{BenchmarkId, Criterion, Throughput};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, EvaluationWorkspace, ExtremumCutRule, ExtremumRow, FormulaNodes, FormulaParts,
    Interpretation, Node, NodeView, NumericDomain, NumericRow, Theory, ThresholdDescriptor,
    TightCheckLimits, TightPlan, TightPlanLimits, TightVerdict, TightWorkspace,
};

const ROUNDS: usize = 16_384;
const WORLDS: usize = 4;

#[derive(Clone, Copy)]
enum Tail {
    Boolean,
    UnusedThreshold,
    Threshold,
    Extremum,
}

impl Tail {
    fn label(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::UnusedThreshold => "unused_threshold",
            Self::Threshold => "one_threshold",
            Self::Extremum => "one_extremum",
        }
    }

    fn numeric(self) -> bool {
        matches!(self, Self::Threshold | Self::Extremum)
    }
}

struct Fixture {
    plan: TightPlan,
    worlds: [Interpretation; WORLDS],
    tail: Tail,
}

fn prepare(tail: Tail, control: &Cancellation) -> Fixture {
    // Two atomic choices precede the shared Boolean body. Each round computes
    // conjunction and disjunction from the same preceding pair. After the first
    // round they denote a & b and a | b, respectively, in every interpretation.
    let prefix = vec![
        Node::falsum(),
        Node::atom(0),
        Node::atom(1),
        Node::implies(1, 0),
        Node::or_pair([1, 3]),
        Node::implies(2, 0),
        Node::or_pair([2, 5]),
    ];
    let mut nodes = FormulaNodes::new(FormulaParts::new(prefix, vec![]).unwrap());
    let mut transaction = nodes.transaction();
    let limits = AdmissionLimits::default();
    let mut pair = [1, 2];
    for _ in 0..ROUNDS {
        pair = [
            transaction
                .push(NodeView::And(&pair), limits.max_nodes, limits.max_operands)
                .unwrap(),
            transaction
                .push(NodeView::Or(&pair), limits.max_nodes, limits.max_operands)
                .unwrap(),
        ];
    }
    let test = match tail {
        Tail::Boolean | Tail::UnusedThreshold => transaction
            .push(NodeView::Or(&pair), limits.max_nodes, limits.max_operands)
            .unwrap(),
        Tail::Threshold => {
            transaction
                .push_threshold_family_range(
                    &pair,
                    &[1, 2],
                    &[2],
                    limits.max_nodes,
                    limits.max_operands,
                )
                .unwrap()
                .start
        }
        Tail::Extremum => {
            transaction
                .push_extremum_cut_family_range(
                    ExtremumRow {
                        conditions: &pair,
                        ranks: &[1, 2],
                        rank_count: 4,
                    },
                    &[ExtremumCutRule::Suffix(2)],
                    limits.max_nodes,
                    limits.max_operands,
                )
                .unwrap()
                .start
        }
    };
    let root = transaction
        .push(
            NodeView::Implies(test, 0),
            limits.max_nodes,
            limits.max_operands,
        )
        .unwrap();
    transaction.commit();
    let mut parts = nodes.into_parts();
    if matches!(tail, Tail::UnusedThreshold) {
        parts = with_unused_threshold(&parts, pair);
    }
    assert_eq!(parts.nodes().len(), 2 * ROUNDS + 9);
    assert_eq!(
        parts.rows().len(),
        usize::from(!matches!(tail, Tail::Boolean))
    );
    let live = parts
        .nodes()
        .iter()
        .filter(|node| {
            node.threshold_descriptor().is_some() || node.extremum_cut_descriptor().is_some()
        })
        .count();
    assert_eq!(live, usize::from(tail.numeric()));
    let theory = Theory::new(2, parts, vec![4, 6, root], limits).unwrap();
    assert_eq!(
        EvaluationWorkspace::requested_payload_bytes(&theory),
        theory.nodes().len() as u128 + if tail.numeric() { 24 } else { 0 },
    );
    let plan = TightPlan::compile(&theory, TightPlanLimits::default(), control).unwrap();
    assert!(plan.theory().same_instance(&theory));
    assert_eq!(plan.producers().len(), 2);
    let worlds = std::array::from_fn(|mask| {
        Interpretation::new(&theory, (0..2).filter(|atom| mask & (1 << atom) != 0)).unwrap()
    });
    Fixture { plan, worlds, tail }
}

fn with_unused_threshold(parts: &FormulaParts, pair: [usize; 2]) -> FormulaParts {
    FormulaParts::from_numeric_raw(
        parts.nodes().to_vec(),
        parts.operands().to_vec(),
        pair.to_vec(),
        vec![1, 2],
        vec![NumericRow {
            start: 0,
            length: 2,
            domain: NumericDomain::Weights,
        }],
        vec![ThresholdDescriptor { row: 0, bound: 2 }],
        vec![],
    )
    .unwrap()
}

impl Fixture {
    fn batch(&self, workspace: &mut TightWorkspace, control: &Cancellation) -> [u64; WORLDS] {
        std::array::from_fn(|index| {
            workspace
                .check(
                    black_box(&self.plan),
                    black_box(&self.worlds[index]),
                    TightCheckLimits::default(),
                    black_box(control),
                )
                .result
                .unwrap()
                .work
        })
    }

    fn expected_work(&self, mask: usize, fresh: bool) -> u64 {
        let a = usize::from(mask & 1 != 0);
        let b = usize::from(mask & 2 != 0);
        // Prefix: four implication reads and two choice reads of one or two
        // operands. Every Boolean round reads exactly three operands. The final
        // implication reads two operands; all three roots are checked. Only
        // the empty interpretation reaches the two producers and one support word.
        let reads = 8 - a - b + 3 * ROUNDS + 2 + 2;
        let short_circuit = usize::from(!self.tail.numeric()) * a * b;
        let numeric_reset = usize::from(self.tail.numeric()) * (1 + usize::from(fresh));
        u64::try_from(
            self.plan.theory().nodes().len() + reads - short_circuit
                + 3
                + 3 * usize::from(mask == 0)
                + numeric_reset,
        )
        .unwrap()
    }

    fn verify(&self, control: &Cancellation) -> TightWorkspace {
        let mut workspace = TightWorkspace::default();
        for pass in 0..2 {
            for (mask, candidate) in self.worlds.iter().enumerate() {
                let attempt =
                    workspace.check(&self.plan, candidate, TightCheckLimits::default(), control);
                let check = attempt.result.unwrap();
                assert_eq!(check.work, attempt.work);
                assert_eq!(check.work, self.expected_work(mask, pass == 0 && mask == 0));
                let expected = if mask == 0 {
                    TightVerdict::Stable
                } else {
                    TightVerdict::NotModel {
                        root: *self.plan.theory().roots().last().unwrap(),
                    }
                };
                assert_eq!(check.verdict, expected);
            }
        }
        let retained = workspace.retained_bytes();
        let work = self.batch(&mut workspace, control);
        assert_eq!(
            work,
            std::array::from_fn(|mask| self.expected_work(mask, false))
        );
        assert_eq!(workspace.retained_bytes(), retained);
        eprintln!(
            "TIGHT_NUMERIC fixture={} nodes={} rows={} requested_evaluation_bytes={} retained_workspace_bytes={retained} reused_batch_work={}",
            self.tail.label(),
            self.plan.theory().nodes().len(),
            self.plan.theory().parts().rows().len(),
            EvaluationWorkspace::requested_payload_bytes(self.plan.theory()),
            work.iter().sum::<u64>(),
        );
        workspace
    }
}

pub(super) fn bench(criterion: &mut Criterion) {
    let control = Cancellation::default();
    let mut benchmark = criterion.benchmark_group("tight_numeric");
    benchmark.sample_size(10);
    benchmark.warm_up_time(Duration::from_millis(250));
    benchmark.measurement_time(Duration::from_millis(500));
    benchmark.throughput(Throughput::Elements(u64::try_from(WORLDS).unwrap()));
    for tail in [
        Tail::Boolean,
        Tail::UnusedThreshold,
        Tail::Threshold,
        Tail::Extremum,
    ] {
        let fixture = prepare(tail, &control);
        let mut workspace = fixture.verify(&control);
        benchmark.bench_function(
            BenchmarkId::new("reused_workspace", tail.label()),
            |bencher| {
                bencher.iter(|| black_box(fixture.batch(&mut workspace, &control)));
            },
        );
    }
    benchmark.finish();
}
