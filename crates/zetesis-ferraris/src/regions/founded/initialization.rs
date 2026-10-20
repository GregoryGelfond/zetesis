//! Immutable initial roles for the selected founded body nodes.
//!
//! A role depends only on the exact theory, flattened chains and cyclic-head
//! mask. Region masks are applied on every pass; no previous closure truth is
//! retained. Roles remain parallel to the selected node IDs; preparation may
//! partition both together so mutable counters precede relaxed seed roles.

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
    /// These relaxed leaves have no incoming dependency to replay. An empty
    /// conjunction also has no incoming event, although it is a Boolean node.
    pub(super) const fn is_seed(self) -> bool {
        matches!(self, Self::True | Self::Boundary(_) | Self::Awaiting(0))
    }

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

/// Keep node IDs and their immutable roles aligned while moving relaxed seeds
/// after mutable counters. This changes no incidence row or semantic identity.
/// Owner authority is absent until this complete permutation is published.
pub(super) fn partition(
    body_nodes: &mut [usize],
    initial: &mut [Initial],
    work: &mut Work<'_>,
) -> Result<usize, Stop> {
    debug_assert_eq!(body_nodes.len(), initial.len());
    let mut seeds = initial.len();
    let mut position = 0;
    while position < seeds {
        work.tick()?;
        if initial[position].is_seed() {
            seeds -= 1;
            if position != seeds {
                work.tick()?;
                body_nodes.swap(position, seeds);
                initial.swap(position, seeds);
            }
        } else {
            position += 1;
        }
    }
    Ok(seeds)
}
