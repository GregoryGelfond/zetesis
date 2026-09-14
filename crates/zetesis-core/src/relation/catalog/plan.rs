//! A one-tuple transaction containing only equality IDs and tentative AVL nodes.
//!
//! Later values inspect the latest tentative patch before the published tree.
//! No row, dictionary representative or tree link changes before final admission.
//! With a new values and height h, retained patches use O(a h) cells. Their linear
//! reverse lookup can cost O(a² h²) metadata probes; each probe is charged. This
//! depends on this tuple's width, never on shifting the historical dictionary.

use std::{cmp::Ordering, mem::size_of};

use crate::{
    Atom, Value,
    ordered_index::{Directions, Index, Link, Node, Step, position},
};

use super::super::{Cell, DictionaryIndex, Failure, Layout, Resource, Work, ceiling};

pub(super) struct Plan {
    pub ids: Vec<u32>,
    pub added: Vec<Cell>,
    pub patches: Vec<Step>,
    pub root: Link,
    pub payload: u128,
}

impl Plan {
    fn node(&self, nodes: &[Node], id: usize, work: &mut Work) -> Result<Node, Failure> {
        for patch in self.patches.iter().rev() {
            work.tick(1)?;
            if patch.id == id {
                return Ok(patch.node);
            }
        }
        work.tick(1)?;
        nodes.get(id).copied().ok_or(Failure::Dictionary)
    }

    fn value<'a>(
        &self,
        dictionary: &[Cell],
        atoms: &'a [Atom],
        atom: &'a Atom,
        id: usize,
    ) -> Result<&'a Value, Failure> {
        let cell = if let Some(cell) = dictionary.get(id) {
            return atoms
                .get(cell.row)
                .and_then(|atom| atom.values().get(cell.column))
                .ok_or(Failure::Dictionary);
        } else {
            self.added
                .get(id - dictionary.len())
                .ok_or(Failure::Dictionary)?
        };
        atom.values().get(cell.column).ok_or(Failure::Dictionary)
    }

    fn locate(
        &self,
        index: &Index,
        dictionary: &[Cell],
        atoms: &[Atom],
        atom: &Atom,
        value: &Value,
        work: &mut Work,
    ) -> Result<(Option<usize>, Directions), Failure> {
        let mut route = Directions::default();
        let mut cursor = self.root;
        while let Some(link) = cursor {
            work.tick(1)?;
            let id = position(link);
            let order = work.compare(value, self.value(dictionary, atoms, atom, id)?)?;
            if order.is_eq() {
                return Ok((Some(id), route));
            }
            let right = order == Ordering::Greater;
            route.push(right).ok_or(Failure::Dictionary)?;
            cursor = self.node(&index.nodes, id, work)?.children[usize::from(right)];
        }
        Ok((None, route))
    }

    fn extend(
        &mut self,
        index: &mut Index,
        id: usize,
        route: &Directions,
        work: &mut Work,
    ) -> Result<(), Failure> {
        index.path.clear();
        let mut cursor = self.root;
        for offset in 0..route.len() {
            work.tick(1)?;
            let right = route.get(offset).ok_or(Failure::Dictionary)?;
            let previous = position(cursor.ok_or(Failure::Dictionary)?);
            let node = self.node(&index.nodes, previous, work)?;
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
        }
        if cursor.is_some() {
            return Err(Failure::Dictionary);
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
        let root = index.plan_from(self.root, id, &mut || work.tick(1))?;
        work.tick(index.path.len() as u128)?;
        let changed = index.path.iter().filter(|step| step.changed).count();
        work.grow(&mut self.patches, changed)?;
        for step in &index.path {
            work.tick(1)?;
            if step.changed {
                self.patches.push(*step);
            }
        }
        self.root = root;
        Ok(())
    }

    pub fn release(self, work: &mut Work) {
        work.release(self.ids);
        work.release(self.added);
        work.release(self.patches);
        work.live -= size_of::<Self>();
    }
}

pub(super) fn values(
    layout: &mut Layout,
    atoms: &[Atom],
    atom: &Atom,
    payload: u128,
    work: &mut Work,
) -> Result<Plan, Failure> {
    let Layout {
        dictionary, index, ..
    } = layout;
    let DictionaryIndex::Append(index) = index else {
        return Err(Failure::Dictionary);
    };
    work.include(size_of::<Plan>())?;
    let mut plan = Plan {
        ids: work.reserve(atom.values().len())?,
        added: work.reserve(atom.values().len())?,
        patches: Vec::new(),
        root: index.root,
        payload,
    };
    for (column, value) in atom.values().iter().enumerate() {
        work.tick(1)?;
        plan.payload = plan
            .payload
            .checked_add(value.payload_bytes() as u128)
            .ok_or(Failure::Overflow)?;
        let (found, route) = plan.locate(index, dictionary, atoms, atom, value, work)?;
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
            plan.extend(index, id, &route, work)?;
            work.tick(1)?;
            plan.added.push(Cell {
                row: atoms.len(),
                column,
            });
            id
        };
        work.tick(1)?;
        plan.ids
            .push(u32::try_from(id).map_err(|_| Failure::Overflow)?);
    }
    Ok(plan)
}

pub(super) fn row(
    index: &mut Index,
    id: usize,
    route: &Directions,
    work: &mut Work,
) -> Result<Link, Failure> {
    index.path.clear();
    let mut cursor = index.root;
    for offset in 0..route.len() {
        work.tick(1)?;
        let right = route.get(offset).ok_or(Failure::Dictionary)?;
        let previous = position(cursor.ok_or(Failure::Dictionary)?);
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
    }
    if cursor.is_some() {
        return Err(Failure::Dictionary);
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
    index.plan(id, &mut || work.tick(1))
}

fn push(index: &mut Index, step: Step, work: &mut Work) -> Result<(), Failure> {
    work.grow(&mut index.path, 1)?;
    work.tick(1)?;
    index.path.push(step);
    Ok(())
}
