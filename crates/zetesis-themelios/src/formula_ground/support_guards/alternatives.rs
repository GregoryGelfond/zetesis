//! Necessary conditions shared by every complete producer of one head.
//!
//! The retained guard h -> OR bodies entails h -> c when every body entails
//! c. Follow AND edges only, intersecting exact opaque leaf identities across
//! all producers. OR, implication and aggregate nodes are leaves, not evidence
//! about their operands. Exact FALSUM entails every condition; VERUM defeats
//! the inference. Repeated producers remain part of the complete family but
//! do not strengthen its intersection. No candidate or support snapshot is
//! used as truth evidence. Original guards remain, and each additional guard
//! is double-negated, so its frozen truth depends only on the candidate M.
//!
//! Skip singleton, equal-body and entirely opaque families before allocation.
//! Among eligible bodies, the smallest immediate width seeds the intersection.
//! Stop a family when its intersection becomes empty. A reusable marked walk
//! visits each AND DAG by identity, with O(V + E) work per visited body and
//! O(K) filtering per remaining body for K surviving candidates. A complete
//! metadata scan costs O(A + P) for A atoms and P producer entries. Scratch is
//! O(N + V + K): byte marks bounded by the largest eligible producer node and
//! one queue and candidate row, all leased and reused within this pass. Reset
//! only previously visited marks. No storage survives formula construction.
//! Publication costs the existing guard and complete-source-evidence charges.

use super::{Buffer, Builder, FALSUM, FormulaFailure, Metadata, Node, VERUM};
use crate::ProgramSite;

#[cfg(test)]
mod tests;

pub(super) fn publish(
    builder: &mut Builder<'_, '_, '_>,
    metadata: &Metadata,
) -> Result<(), FormulaFailure> {
    #[cfg(test)]
    if !tests::enabled() {
        return Ok(());
    }
    let mut scratch = None;
    for atom in 0..builder.catalog.len() {
        let location = metadata.location(atom);
        builder.work(location)?;
        let Some(family) = family(builder, metadata, atom)? else {
            continue;
        };
        if scratch.is_none() {
            scratch = Some(Scratch::new(builder, location)?);
        }
        let scratch = scratch.as_mut().expect("eligible family owns scratch");
        scratch.intersect(builder, metadata, atom, family)?;
        if scratch.candidates.len() == 0 {
            continue;
        }
        let head = builder.node(Node::Atom(atom), location)?;
        for &condition in scratch.candidates.iter() {
            // A positive cycle can mention its own head. Its tautological
            // consequence would add a root without teaching anything.
            builder.work(location)?;
            if condition == head {
                continue;
            }
            #[cfg(test)]
            tests::point(tests::Point::Publish);
            builder.condition_guard(metadata, condition, &[head], head, location)?;
            #[cfg(test)]
            tests::published();
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Family {
    seed: usize,
    prefix: usize,
}

/// This inspection uses complete construction metadata, not currently viable
/// producers. Only the admitted constant FALSUM is omitted. A fact (VERUM)
/// prevents every nontrivial shared requirement, regardless of producer order.
fn family(
    builder: &mut Builder<'_, '_, '_>,
    metadata: &Metadata,
    atom: usize,
) -> Result<Option<Family>, FormulaFailure> {
    let producers = metadata.producers(atom);
    if producers.len() < 2 {
        return Ok(None);
    }
    let location = metadata.location(atom);
    let mut seed = None;
    let mut first = None;
    let mut distinct = false;
    let mut conjunction = false;
    let mut prefix = 0;
    for body in producers {
        #[cfg(test)]
        tests::point(tests::Point::Eligibility);
        builder.work(location)?;
        if body == VERUM {
            return Ok(None);
        }
        if body == FALSUM {
            continue;
        }
        if let Some(first) = first {
            distinct |= body != first;
        } else {
            first = Some(body);
        }
        let width = match builder.nodes.view().node(body).expect("owned producer") {
            Node::And(row) => {
                conjunction = true;
                row.len()
            }
            _ => 1,
        };
        if seed.is_none_or(|(_, previous)| width < previous) {
            seed = Some((body, width));
        }
        prefix = prefix.max(body.checked_add(1).expect("allocated producer node"));
    }
    Ok(if distinct && conjunction {
        Some(Family {
            seed: seed.expect("distinct nonconstant producers").0,
            prefix,
        })
    } else {
        None
    })
}

struct Scratch {
    seen: Buffer<u8>,
    pending: Buffer<usize>,
    candidates: Buffer<usize>,
}

impl Scratch {
    fn new(
        builder: &mut Builder<'_, '_, '_>,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        Ok(Self {
            seen: Buffer::new(
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?,
            pending: Buffer::new(
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?,
            candidates: Buffer::new(
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?,
        })
    }

    fn intersect(
        &mut self,
        builder: &mut Builder<'_, '_, '_>,
        metadata: &Metadata,
        atom: usize,
        family: Family,
    ) -> Result<(), FormulaFailure> {
        let location = metadata.location(atom);
        if self.seen.len() < family.prefix {
            self.seen.resize(
                family.prefix,
                0,
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?;
        }
        self.candidates.clear();
        self.walk(builder, family.seed, true, location)?;
        for body in metadata.producers(atom) {
            builder.work(location)?;
            if self.candidates.len() == 0 {
                break;
            }
            if body == family.seed || body == FALSUM {
                continue;
            }
            self.walk(builder, body, false, location)?;
            let mut retained = 0;
            for position in 0..self.candidates.len() {
                #[cfg(test)]
                tests::point(tests::Point::Intersection);
                builder.work(location)?;
                let condition = self.candidates.slice()[position];
                if self.seen.slice()[condition] != 0 {
                    self.candidates.slice_mut()[retained] = condition;
                    retained += 1;
                }
            }
            self.candidates.resize(
                retained,
                0,
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?;
        }
        Ok(())
    }

    /// Queue contains every marked node exactly once. Topological node IDs
    /// bound all edges by the producer prefix. Resetting those marks before a
    /// new walk avoids a full-prefix clear and is valid across family changes.
    fn walk(
        &mut self,
        builder: &mut Builder<'_, '_, '_>,
        body: usize,
        collect: bool,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        for &node in self.pending.iter() {
            builder.work(location)?;
            self.seen.slice_mut()[node] = 0;
        }
        self.pending.clear();
        self.enqueue(builder, body, location)?;
        let mut cursor = 0;
        while cursor < self.pending.len() {
            #[cfg(test)]
            tests::point(tests::Point::Walk);
            builder.work(location)?;
            let node = self.pending.slice()[cursor];
            cursor += 1;
            let width = match builder.nodes.view().node(node).expect("owned conjunct") {
                Node::And(row) => Some(row.len()),
                _ => None,
            };
            if let Some(width) = width {
                for position in 0..width {
                    builder.work(location)?;
                    let child = super::child(builder, node, position);
                    self.enqueue(builder, child, location)?;
                }
            } else if collect && ![FALSUM, VERUM].contains(&node) {
                self.candidates.push(
                    node,
                    builder.computation,
                    builder.limits,
                    builder.counters,
                    location,
                )?;
            }
        }
        Ok(())
    }

    fn enqueue(
        &mut self,
        builder: &mut Builder<'_, '_, '_>,
        node: usize,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        if self.seen.slice()[node] == 0 {
            self.pending.push(
                node,
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?;
            self.seen.slice_mut()[node] = 1;
        }
        Ok(())
    }
}
