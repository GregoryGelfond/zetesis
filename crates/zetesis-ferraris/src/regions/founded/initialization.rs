//! Immutable initial roles for the selected founded body nodes.
//!
//! A role depends only on the exact theory, flattened chains and cyclic-head
//! mask. Region masks are applied on every pass; no previous closure truth is
//! retained. Rows are parallel to the existing ordered body-node selection.

use super::{Narrower, NodeView, State, Theory, Work, bit, chain_position};
use zetesis_cpu::Stop;

#[derive(Clone, Copy, Debug)]
pub(super) enum Initial {
    Inactive,
    Awaiting(usize),
    True,
    Boundary(usize),
}

impl Initial {
    pub(super) fn read(
        theory: &Theory,
        index: &Narrower,
        cyclic_atoms: &[u64],
        node: usize,
    ) -> Result<Self, Stop> {
        if index.absorbed[node] {
            return Ok(Self::Inactive);
        }
        Ok(
            match theory.view().node(node).map_err(|_| Stop::InvalidProgram)? {
                NodeView::False => Self::Inactive,
                NodeView::Atom(atom) => {
                    if bit(cyclic_atoms, atom) {
                        Self::Awaiting(1)
                    } else {
                        Self::Boundary(atom)
                    }
                }
                NodeView::And(_) | NodeView::Or(_) => {
                    let chain = index.chain_of[node].ok_or(Stop::InvalidProgram)?;
                    let chain = &index.chains[chain_position(chain)];
                    Self::Awaiting(if chain.disjunction {
                        1
                    } else {
                        chain.leaves(&index.extrema).len()
                    })
                }
                NodeView::Implies(_, _) | NodeView::Threshold(_) | NodeView::ExtremumCut(_) => {
                    Self::True
                }
            },
        )
    }

    pub(super) fn state(self, atom_never: &[u64]) -> State {
        match self {
            Self::Inactive => State::Inactive,
            Self::Awaiting(count) => State::Awaiting(count),
            Self::True => State::True,
            Self::Boundary(atom) => {
                if bit(atom_never, atom) {
                    State::Inactive
                } else {
                    State::True
                }
            }
        }
    }
}

pub(super) fn prepare(
    theory: &Theory,
    index: &Narrower,
    body_nodes: &[usize],
    cyclic_atoms: &[u64],
    work: &mut Work<'_>,
) -> Result<Vec<Initial>, Stop> {
    work.tick()?;
    let mut initial = Vec::new();
    initial
        .try_reserve_exact(body_nodes.len())
        .map_err(|_| Stop::Allocation)?;
    for &node in body_nodes {
        work.tick()?;
        initial.push(Initial::read(theory, index, cyclic_atoms, node)?);
    }
    Ok(initial)
}
