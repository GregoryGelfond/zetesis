//! Each vertex publishes true once; each outgoing incidence is visited once.

use super::{
    Budget, PositiveError,
    activation::{self, Effects, Point},
    compile::Graph,
};
use crate::{NodeView, Theory};
use zetesis_algebra::Stop;

pub(super) fn complete(
    theory: &Theory,
    graph: &mut Graph,
    words: &mut [u64],
    budget: &mut Budget<'_, '_>,
) -> Result<(), PositiveError> {
    seed(theory, graph, budget)?;
    activation::drain(
        theory.atom_count(),
        &graph.offsets,
        &graph.edges,
        &mut graph.states,
        &mut graph.queue,
        words,
        &mut Legacy {
            atoms: theory.atom_count(),
            budget,
        },
    )
}

/// Preserve the existing tick and statistics boundaries, including the edge
/// count before a later activation refusal. Fine-grained operation hooks add no
/// legacy debit or cancellation poll.
struct Legacy<'a, 'b, 'c> {
    atoms: usize,
    budget: &'a mut Budget<'b, 'c>,
}

impl Effects for Legacy<'_, '_, '_> {
    type Error = PositiveError;

    fn before(&mut self, point: Point, _work: u64) -> Result<(), Self::Error> {
        match point {
            Point::Operation => Ok(()),
            Point::Dequeue | Point::Activation => self.budget.tick(),
            Point::Incidence => {
                self.budget.tick()?;
                self.budget.statistics.propagated_dependencies += 1;
                Ok(())
            }
        }
    }

    fn activated(&mut self, vertex: usize, _source: Option<usize>) -> Result<(), Self::Error> {
        if vertex < self.atoms {
            self.budget.statistics.derived_atoms += 1;
        } else {
            self.budget.statistics.activated_nodes += 1;
        }
        Ok(())
    }
}

fn activate(
    vertex: usize,
    atoms: usize,
    graph: &mut Graph,
    budget: &mut Budget<'_, '_>,
) -> Result<(), PositiveError> {
    activation::activate(
        vertex,
        None,
        &mut graph.states,
        &mut graph.queue,
        &mut Legacy { atoms, budget },
    )
}

fn seed(
    theory: &Theory,
    graph: &mut Graph,
    budget: &mut Budget<'_, '_>,
) -> Result<(), PositiveError> {
    for index in 0..theory.view().len() {
        budget.tick()?;
        let node = theory
            .view()
            .node(index)
            .map_err(|_| Stop::InvalidProgram)?;
        if let NodeView::Threshold(threshold) = node {
            if threshold.bound <= 0 {
                activate(
                    theory.atom_count() + index,
                    theory.atom_count(),
                    graph,
                    budget,
                )?;
            }
        } else if let NodeView::ExtremumCut(cut) = node {
            if cut.endpoint_truth() == Some(true) {
                activate(
                    theory.atom_count() + index,
                    theory.atom_count(),
                    graph,
                    budget,
                )?;
            }
        } else if let NodeView::Implies(a, b) = node {
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
