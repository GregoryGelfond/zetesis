//! Untimed public-API diagnostic; no assertion assumes the propagation outcome.
//!
//! Reuses the evaluator benchmark's transcription of the former shared Boolean DP.

use super::{Row, boolean_family};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison, AggregateElement, AggregateFamilyLimits, AggregateGuard,
    FormulaNodes, FormulaParts, Interpretation, Limits, Narrower, Narrowing, NarrowingScratch,
    Node, NodeView, OriginalSubject, Region, RegionLimits, Theory, append_aggregate_family, models,
};

#[derive(Clone, Copy)]
enum Representation {
    CompiledNative,
    SharedBooleanDp,
    AllThresholds,
}

impl Representation {
    fn label(self) -> &'static str {
        match self {
            Self::CompiledNative => "compiled_native_mixed_boundary",
            Self::SharedBooleanDp => "former_shared_boolean_dp",
            Self::AllThresholds => "native_both_numeric_control",
        }
    }
}

fn theory(representation: Representation, control: &Cancellation) -> Theory {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..4).map(Node::atom).collect(), Vec::new()).unwrap());
    let row = Row {
        elements: (0..3)
            .map(|condition| AggregateElement {
                condition,
                weight: 1,
            })
            .collect(),
        guards: [2, 3]
            .map(|bound| AggregateGuard {
                comparison: AggregateComparison::Ge,
                bound,
            })
            .to_vec(),
    };
    let limits = AdmissionLimits::default();
    let guards = match representation {
        Representation::CompiledNative => append_aggregate_family(
            &mut nodes,
            &row.elements,
            &row.guards,
            AggregateFamilyLimits::default(),
            control,
        )
        .unwrap()
        .roots()
        .to_vec(),
        Representation::SharedBooleanDp => boolean_family(&mut nodes, &row),
        Representation::AllThresholds => nodes
            .push_threshold_family(
                &[0, 1, 2],
                &[1, 1, 1],
                &[2, 3],
                limits.max_nodes,
                limits.max_operands,
            )
            .unwrap(),
    };
    let mut transaction = nodes.transaction();
    let mut push = |node: NodeView<'_>| {
        transaction
            .push(node, limits.max_nodes, limits.max_operands)
            .unwrap()
    };
    let falsum = push(NodeView::False);
    let not_two = push(NodeView::Implies(guards[0], falsum));
    // z -> G(3) exposes G(3)=false as the observable atom consequence z=false.
    let observer = push(NodeView::Implies(3, guards[1]));
    transaction.commit();
    Theory::new(4, nodes.into_parts(), vec![not_two, observer], limits).unwrap()
}

pub(super) fn run() {
    let control = Cancellation::default();
    for representation in [
        Representation::CompiledNative,
        Representation::SharedBooleanDp,
        Representation::AllThresholds,
    ] {
        let theory = theory(representation, &control);
        let narrower = Narrower::new(&theory);
        let mut region = Region::all_open(4);
        let (outcome, statistics) = narrower
            .narrow_known(
                OriginalSubject::new(&theory, None),
                &mut region,
                &mut narrower.knowledge(),
                &mut NarrowingScratch::default(),
                RegionLimits::default(),
                &control,
            )
            .unwrap();
        let mut model_count = 0;
        for mask in 0_u32..16 {
            let candidate =
                Interpretation::new(&theory, (0..4).filter(|&atom| mask & (1 << atom) != 0))
                    .unwrap();
            let is_model = models(&theory, &candidate, Limits::default(), &control).unwrap();
            let expected = (mask & 7).count_ones() < 2 && mask & 8 == 0;
            assert_eq!(is_model, expected, "{} mask={mask}", representation.label());
            if is_model {
                model_count += 1;
                assert_ne!(outcome, Narrowing::Refuted);
                for atom in 0..4 {
                    if let Some(value) = region.decision(atom) {
                        assert_eq!(value, mask & (1 << atom) != 0);
                    }
                }
            }
        }
        eprintln!(
            "boundary_probe representation={} outcome={outcome:?} decisions={:?} \
             preferred={:?} propagation={statistics:?} checked_models={model_count}",
            representation.label(),
            (0..4).map(|atom| region.decision(atom)).collect::<Vec<_>>(),
            region.split_atom(),
        );
    }
}
