//! Each vertex publishes true once; each outgoing incidence is visited once.

use super::{
    Budget, PositiveError,
    compile::{Graph, State},
};
use crate::{NodeView, Theory};
use zetesis_cpu::Stop;

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
                State::Awaiting(1) => activate(target, theory.atom_count(), graph, budget)?,
                State::Awaiting(remaining) => graph.states[target] = State::Awaiting(remaining - 1),
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
    for index in 0..theory.view().len() {
        budget.tick()?;
        if let NodeView::Implies(a, b) = theory
            .view()
            .node(index)
            .map_err(|_| Stop::InvalidProgram)?
        {
            budget.tick()?;
            let left = theory.view().node(a).map_err(|_| Stop::InvalidProgram)?;
            budget.tick()?;
            let right = theory.view().node(b).map_err(|_| Stop::InvalidProgram)?;
            if left == NodeView::False && right == NodeView::False {
                activate(
                    theory.atom_count() + index,
                    theory.atom_count(),
                    graph,
                    budget,
                )?;
            }
        }
    }
    for &root in theory.roots() {
        budget.tick()?;
        if let NodeView::Atom(atom) = theory.view().node(root).map_err(|_| Stop::InvalidProgram)? {
            activate(atom, theory.atom_count(), graph, budget)?;
        }
    }
    Ok(())
}
