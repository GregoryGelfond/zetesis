//! Shared seed views preserve checked static transport without a device.
use std::sync::Arc;

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, GroundProgram, Predicate, Program, Seed, SeedSelection,
    Sign, StaticLimits, Template, Term, Value, ValueLimits, ValueNode,
};

use super::{BatchPlan, PackedSeeds};
use crate::{GpuErrorKind, GpuLimits};

fn graph(atoms: Vec<Atom>) -> GroundProgram {
    let templates = atoms
        .into_iter()
        .map(|atom| {
            let pattern = AtomPattern::new(
                atom.predicate().clone(),
                atom.values().iter().cloned().map(Term::Constant).collect(),
            )
            .unwrap();
            Template::new(Some(pattern.clone()), vec![], vec![pattern], vec![], vec![])
        })
        .collect();
    let program = Program::new(templates, AdmissionLimits::default()).unwrap();
    GroundProgram::compile(&program, StaticLimits::default()).unwrap()
}
fn number(value: i32) -> Atom {
    Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(value)]).unwrap()
}
fn plan(graph: &GroundProgram, worlds: usize) -> BatchPlan {
    BatchPlan::new(
        graph,
        worlds,
        GpuLimits::default(),
        &wgpu::Limits::default(),
    )
    .unwrap()
}
fn selected(graph: &GroundProgram, atoms: impl IntoIterator<Item = Atom>) -> SeedSelection {
    SeedSelection::new(graph.program(), atoms.into_iter().map(Arc::new)).unwrap()
}

#[test]
fn selected_views_pack_exact_word_boundaries() {
    let graph = graph((0..70).map(number).collect());
    assert_eq!(graph.word_count(), 3);
    let selected = selected(&graph, [69, 0, 64, 32, 31, 0].into_iter().map(number));
    let empty = Seed::new(graph.program(), []).unwrap();
    let batch = PackedSeeds::new(
        &graph,
        [empty.view(), selected.view()].into_iter(),
        &plan(&graph, 2),
    )
    .unwrap();
    assert_eq!(batch.seeds, [0, 0, 0, 0x8000_0001, 1, 0x21]);
}

#[test]
fn selected_views_keep_complete_typed_identity() {
    let values = [
        Value::Infimum,
        Value::Number(-1),
        Value::String("1".into()),
        Value::Symbol("1".into()),
        Value::Supremum,
        Value::from_nodes(
            vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(1)],
            ValueLimits::default(),
        )
        .unwrap(),
        Value::from_nodes(
            vec![
                ValueNode::Function {
                    name: "f".into(),
                    arity: 1,
                    sign: Sign::Negative,
                },
                ValueNode::Number(1),
            ],
            ValueLimits::default(),
        )
        .unwrap(),
    ];
    let atoms: Vec<_> = values
        .into_iter()
        .enumerate()
        .map(|(i, value)| {
            Atom::new(
                Predicate::with_sign(
                    "p",
                    1,
                    if i % 2 == 0 {
                        Sign::Positive
                    } else {
                        Sign::Negative
                    },
                )
                .unwrap(),
                vec![value],
            )
            .unwrap()
        })
        .collect();
    let graph = graph(atoms.clone());
    let selection = selected(&graph, atoms.iter().cloned());
    let seed = Seed::new(graph.program(), atoms).unwrap();
    let expected = graph.seed_words(&seed).unwrap();
    let batch = PackedSeeds::new(&graph, [selection.view()].into_iter(), &plan(&graph, 1)).unwrap();
    assert_eq!(batch.seeds, expected);
    assert_eq!(
        batch
            .seeds
            .iter()
            .map(|word| word.count_ones())
            .sum::<u32>(),
        7
    );
}

#[test]
fn a_foreign_selection_is_refused() {
    let other = graph(vec![number(1)]);
    let graph = graph(vec![number(1)]);
    let selection = selected(&other, [number(1)]);
    assert!(
        matches!(PackedSeeds::new(&graph, [selection.view()].into_iter(), &plan(&graph, 1)), Err(error) if error.kind() == GpuErrorKind::Seed)
    );
}

#[test]
fn empty_graph_views_keep_the_dummy_word() {
    let graph = graph(vec![]);
    let selection = selected(&graph, []);
    let batch = PackedSeeds::new(
        &graph,
        [selection.view(), selection.view()].into_iter(),
        &plan(&graph, 2),
    )
    .unwrap();
    assert_eq!(batch.seeds, [0]);
}

#[test]
fn packing_refuses_an_incomplete_candidate_sequence() {
    let graph = graph(vec![number(1)]);
    let selection = selected(&graph, []);
    assert!(
        matches!(PackedSeeds::new(&graph, [selection.view()].into_iter(), &plan(&graph, 2)), Err(error) if error.kind() == GpuErrorKind::Seed)
    );
}

#[test]
fn packing_refuses_an_excess_candidate_sequence() {
    let graph = graph(vec![number(1)]);
    let selection = selected(&graph, []);
    assert!(
        matches!(PackedSeeds::new(&graph, [selection.view(), selection.view()].into_iter(), &plan(&graph, 1)), Err(error) if error.kind() == GpuErrorKind::Seed)
    );
}
