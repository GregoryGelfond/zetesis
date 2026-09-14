//! Each vertex publishes true once; each outgoing incidence is visited once.

use super::{
    Budget, PositiveError,
    compile::{Graph, State},
};
use crate::{Node, Theory};

pub(super) fn complete(
    theory: &Theory,
    graph: &mut Graph,
    words: &mut [u64],
    budget: &mut Budget<'_>,
) -> Result<(), PositiveError> {
    seed(theory, graph, budget)?;
    let mut next = 0;
    while let Some(&vertex) = graph.queue.get(next) {
        budget.tick()?;
        next += 1;
        if vertex < theory.atom_count() {
            words[vertex / 64] |= 1 << (vertex % 64);
        }
        for edge in graph.offsets[vertex]..graph.offsets[vertex + 1] {
            budget.tick()?;
            budget.statistics.propagated_dependencies += 1;
            let target = graph.targets[edge];
            match graph.states[target] {
                State::AwaitingTwo => graph.states[target] = State::AwaitingOne,
                State::AwaitingOne => activate(target, theory.atom_count(), graph, budget)?,
                State::Inactive | State::True => {}
            }
        }
    }
    Ok(())
}

fn activate(
    vertex: usize,
    atoms: usize,
    graph: &mut Graph,
    budget: &mut Budget<'_>,
) -> Result<(), PositiveError> {
    if graph.states[vertex] != State::True {
        budget.tick()?;
        graph.states[vertex] = State::True;
        graph.queue.push(vertex);
        if vertex < atoms {
            budget.statistics.derived_atoms += 1;
        } else {
            budget.statistics.activated_nodes += 1;
        }
    }
    Ok(())
}

fn seed(theory: &Theory, graph: &mut Graph, budget: &mut Budget<'_>) -> Result<(), PositiveError> {
    for (index, node) in theory.nodes().iter().enumerate() {
        budget.tick()?;
        if let Node::Implies(a, b) = *node
            && theory.nodes()[a] == Node::False
            && theory.nodes()[b] == Node::False
        {
            activate(
                theory.atom_count() + index,
                theory.atom_count(),
                graph,
                budget,
            )?;
        }
    }
    for &root in theory.roots() {
        budget.tick()?;
        if let Node::Atom(atom) = theory.nodes()[root] {
            activate(atom, theory.atom_count(), graph, budget)?;
        }
    }
    Ok(())
}
