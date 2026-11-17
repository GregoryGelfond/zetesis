//! A one-tuple transaction containing only equality IDs and tentative AVL nodes.
//!
//! Later values inspect the latest tentative patch before the published tree.
//! No row, dictionary representative or tree link changes before final admission.
//! With a new values and height h, retained patches use O(a h) cells. Their linear
//! reverse lookup can cost O(a² h²) metadata probes; each probe is charged. This
//! depends on this tuple's width, never on shifting the historical dictionary.

use crate::{
    catalog::{AtomRef, Atoms, storage::TermId},
    ordered_index::{Directions, Index, Node, Planned, Step, position, spine::RightSpine},
};

use super::super::{
    Cell, DictionaryIndex, Failure, Layout, Resource, Work, ceiling, dictionary::Probe,
};

pub(super) struct Added {
    pub cell: Cell,
    pub term: TermId,
}

#[derive(Default)]
pub(super) struct Plan {
    pub ids: Vec<u32>,
    pub added: Vec<Added>,
    pub encoding_bytes: u128,
}

/// Temporary scalar scratch is admitted only after a novel row is established.
/// A session includes its retained header and capacities at operation entry.
#[derive(Clone, Copy)]
pub(super) enum Admission {
    Temporary,
    Retained,
}

impl Plan {
    pub(super) fn capacity_bytes(&self) -> usize {
        super::vector_bytes(&self.ids) + super::vector_bytes(&self.added)
    }

    pub(super) fn clear(&mut self) {
        self.ids.clear();
        self.added.clear();
        self.encoding_bytes = 0;
    }

    /// Only this tuple's unpublished distinct values are scanned. Historical
    /// dictionary hits use the complete canonical inverse instead.
    fn staged(
        &self,
        term: TermId,
        start: usize,
        work: &mut Work,
    ) -> Result<Option<usize>, Failure> {
        for (offset, added) in self.added.iter().enumerate() {
            work.tick(1)?;
            if added.term == term {
                return start.checked_add(offset).map(Some).ok_or(Failure::Overflow);
            }
        }
        Ok(None)
    }
}

pub(super) fn values(
    layout: &mut Layout,
    atoms: Atoms<'_>,
    atom: AtomRef<'_>,
    encoding_bytes: u128,
    plan: &mut Plan,
    work: &mut Work,
) -> Result<(), Failure> {
    let Layout {
        dictionary, index, ..
    } = layout;
    let DictionaryIndex::Append(index) = index else {
        return Err(Failure::Dictionary);
    };
    plan.encoding_bytes = encoding_bytes;
    work.grow(&mut plan.ids, atom.values().len())?;
    work.grow(&mut plan.added, atom.values().len())?;
    for (column, value) in atom.values().iter().enumerate() {
        work.tick(1)?;
        plan.encoding_bytes = plan
            .encoding_bytes
            .checked_add(value.canonical_bytes_with(|| work.tick(1))? as u128)
            .ok_or(Failure::Overflow)?;
        let (term, found) = match index
            .identities
            .probe_with(atoms.read(), value, &mut || work.tick(1))?
        {
            Probe::Local { term, equality } => (term, equality.map(|id| id as usize)),
            Probe::Unavailable(error) => return Err(Failure::Read(error)),
        };
        let found = match found {
            Some(id) => Some(id),
            None => plan.staged(term, dictionary.len(), work)?,
        };
        let id = if let Some(id) = found {
            id
        } else {
            let id = dictionary
                .len()
                .checked_add(plan.added.len())
                .ok_or(Failure::Overflow)?;
            ceiling(
                Resource::Values,
                id as u128 + 1,
                (work.limits.max_values as u128).min(u128::from(u32::MAX) + 1),
            )?;
            work.tick(1)?;
            plan.added.push(Added {
                cell: Cell {
                    row: atoms.len(),
                    column,
                },
                term,
            });
            id
        };
        work.tick(1)?;
        plan.ids
            .push(u32::try_from(id).map_err(|_| Failure::Overflow)?);
    }
    Ok(())
}

/// Both routes come from this owner's actual checked tuple comparisons.
#[derive(Clone, Copy)]
pub(super) enum RowPath<'a> {
    Recorded(&'a Directions),
    BeyondLast(usize),
}

pub(super) fn row(
    index: &mut Index,
    id: usize,
    route: RowPath<'_>,
    retained: Option<RightSpine>,
    work: &mut Work,
) -> Result<Planned, Failure> {
    let reuse = match (&route, retained) {
        (RowPath::BeyondLast(last), Some(spine)) => {
            work.tick(1)?;
            spine.matches(index, index.root, *last)
        }
        _ => false,
    };
    if !reuse {
        index.path.clear();
    }
    let mut cursor = if reuse { None } else { index.root };
    let mut offset = 0;
    while let Some(link) = cursor {
        work.tick(1)?;
        let right = match &route {
            RowPath::Recorded(directions) => directions.get(offset).ok_or(Failure::Dictionary)?,
            RowPath::BeyondLast(_) => true,
        };
        let previous = position(link);
        let node = index.nodes[previous];
        push(
            index,
            Step {
                id: previous,
                node,
                right,
                changed: false,
            },
            work,
        )?;
        cursor = node.children[usize::from(right)];
        offset += 1;
    }
    match route {
        RowPath::Recorded(directions) if offset != directions.len() => {
            return Err(Failure::Dictionary);
        }
        RowPath::BeyondLast(last) => {
            debug_assert_eq!(index.path.last().map(|step| step.id), Some(last));
        }
        RowPath::Recorded(_) => {}
    }
    push(
        index,
        Step {
            id,
            node: Node::default(),
            right: false,
            changed: true,
        },
        work,
    )?;
    index.plan_changes_from(index.root, id, &mut || work.tick(1))
}

fn push(index: &mut Index, step: Step, work: &mut Work) -> Result<(), Failure> {
    work.grow(&mut index.path, 1)?;
    work.tick(1)?;
    index.path.push(step);
    Ok(())
}
