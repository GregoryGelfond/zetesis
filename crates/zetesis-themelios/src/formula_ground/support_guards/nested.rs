//! Shared nested requirements of complete singleton producers.
//!
//! An admitted h -> B guard entails h -> c for every conjunct c of B, even
//! across shared AND nodes. Any disjunction of such heads therefore implies c.
//! Supplement only disjunctions already represented in the formula: the maximal
//! existing binary prefix and a distinct existing flat row, when present. These
//! guards retain the originals and are double-negated:
//! they expose a consequence to original propagation without constraining J in
//! the frozen reduct beyond the original guard's truth at M.
//!
//! Only nested, nonconstant leaves are recorded; immediate leaves already have
//! the ordinary transposition consumer. Duplicate paths count a head once. No
//! inference uses a partial producer family: a non-singleton family is skipped.
//! Two walks cost O(sum(V + E)) over the eligible producer cones, not over their
//! tree unfoldings. Generation marks and one reusable AND queue take O(N) and
//! O(max V) scratch. The counted directory has O(N) entries, and only shared
//! groups retain their I head incidences. All scratch has ordinary workspace
//! leases and all visits, copies and emitted formulas retain existing charges.
//! Probes publish no OR nodes or head incidences. At most two existing premises
//! receive guards per condition, with evidence limited to their covered heads.
//! The temporary incidence arena remains I entries even when no premise exists.
//! This can add guard formulas; the earlier non-expansion price gate is unchanged.

use super::{Buffer, Builder, FALSUM, FormulaFailure, Metadata, Node, Plan, VERUM, producer};
use crate::ProgramSite;
use crate::formula_ground::nodes;
use crate::grounding_observer::Event;

#[cfg(test)]
mod tests;

/// Isolate the earlier transposition law in its existing representation tests.
#[cfg(test)]
pub(super) fn without<T>(action: impl FnOnce() -> T) -> T {
    tests::scoped(false, None, action).0
}

pub(super) fn publish(
    builder: &mut Builder<'_, '_, '_>,
    metadata: &Metadata,
) -> Result<(), FormulaFailure> {
    #[cfg(test)]
    if !tests::enabled() {
        return Ok(());
    }
    let Some(prefix) = eligible_prefix(builder, metadata)? else {
        return Ok(());
    };
    let location = metadata.location(0);
    let mut scratch = Scratch::new(builder, prefix, location)?;
    let mut plan = Plan::new(builder, prefix, location)?;
    visit(
        builder,
        metadata,
        &mut scratch,
        |builder, atom, condition| plan.record(builder, atom, condition, metadata.location(atom)),
    )?;
    if !plan.allocate_heads(builder, metadata, true)? {
        return Ok(());
    }
    scratch.reset(builder, location)?;
    let mut head = None;
    visit(
        builder,
        metadata,
        &mut scratch,
        |builder, atom, condition| fill(&mut plan, builder, metadata, &mut head, atom, condition),
    )?;
    for group in plan.groups.iter() {
        builder.work(metadata.location(group.first_atom))?;
        if group.length < 2 {
            continue;
        }
        let location = metadata.location(group.first_atom);
        let heads = &plan.heads.slice()[group.start..group.start + group.length];
        for premise in premises(builder, heads, location)?.into_iter().flatten() {
            #[cfg(test)]
            tests::point(tests::Point::Publish);
            builder.condition_guard(metadata, *group, &heads[..premise.heads], premise.node)?;
            #[cfg(test)]
            tests::published();
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Premise {
    node: usize,
    heads: usize,
}

/// An existing disjunction can be known while each individual head stays open.
/// Retain its identity instead of adding a disjunction with no such consumer.
/// The input contains distinct atom nodes in catalog order. The maximal binary
/// prefix covers every preceding matched prefix; a flat row can independently
/// become known, so retain it too when distinct. Neither probe publishes nodes.
fn premises(
    builder: &mut Builder<'_, '_, '_>,
    heads: &[usize],
    location: ProgramSite,
) -> Result<[Option<Premise>; 2], FormulaFailure> {
    let mut prefix = None;
    let mut existing = FALSUM;
    for (position, &head) in heads.iter().enumerate() {
        builder.work(location)?;
        if existing == FALSUM {
            existing = head;
            continue;
        }
        let Some(found) = find(builder, Node::Or(&[existing, head]), location)? else {
            break;
        };
        existing = found;
        prefix = Some(Premise {
            node: found,
            heads: position + 1,
        });
    }
    // At width two the flat key is exactly the pair already probed. A lone
    // head adds no information beyond the retained original support guard.
    if heads.len() <= 2 {
        return Ok([prefix, None]);
    }
    let flat = find(builder, Node::Or(heads), location)?
        .filter(|node| prefix.is_none_or(|prefix| prefix.node != *node))
        .map(|node| Premise {
            node,
            heads: heads.len(),
        });
    Ok([prefix, flat])
}

fn find(
    builder: &mut Builder<'_, '_, '_>,
    node: Node<'_>,
    location: ProgramSite,
) -> Result<Option<usize>, FormulaFailure> {
    builder.work(location)?;
    builder.counters.record(Event::NodeLookup);
    nodes::find(
        &builder.node_indices,
        &builder.nodes,
        node,
        location,
        || builder.counters.work(builder.limits, location),
    )
}

/// Bound marks by the largest producer node, not by later guard formulas.
/// Two distinct heads must contain a nested AND before sharing is possible.
fn eligible_prefix(
    builder: &mut Builder<'_, '_, '_>,
    metadata: &Metadata,
) -> Result<Option<usize>, FormulaFailure> {
    let (mut eligible, mut prefix) = (0usize, 0usize);
    for atom in 0..builder.catalog.len() {
        let location = metadata.location(atom);
        builder.work(location)?;
        let Some(body) = producer(metadata, atom) else {
            continue;
        };
        let count = match builder.nodes.view().node(body).expect("owned producer") {
            Node::And(row) => row.len(),
            _ => continue,
        };
        for position in 0..count {
            builder.work(location)?;
            let child = super::child(builder, body, position);
            if matches!(
                builder.nodes.view().node(child).expect("owned conjunct"),
                Node::And(_)
            ) {
                eligible += 1;
                prefix = prefix.max(body.checked_add(1).expect("allocated producer node"));
                break;
            }
        }
    }
    Ok((eligible > 1).then_some(prefix))
}

struct Scratch {
    seen: Buffer<usize>,
    pending: Buffer<usize>,
}

impl Scratch {
    fn new(
        builder: &mut Builder<'_, '_, '_>,
        prefix: usize,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let mut seen = Buffer::new(
            builder.computation,
            builder.limits,
            builder.counters,
            location,
        )?;
        seen.resize(
            prefix,
            0,
            builder.computation,
            builder.limits,
            builder.counters,
            location,
        )?;
        Ok(Self {
            seen,
            pending: Buffer::new(
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?,
        })
    }

    fn reset(
        &mut self,
        builder: &mut Builder<'_, '_, '_>,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        builder
            .counters
            .charge_work(self.seen.len() as u128, builder.limits, location)?;
        self.seen.slice_mut().fill(0);
        Ok(())
    }

    fn enqueue(
        &mut self,
        builder: &mut Builder<'_, '_, '_>,
        node: usize,
        generation: usize,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        if self.seen.slice()[node] != generation {
            self.seen.slice_mut()[node] = generation;
            self.pending.push(
                node,
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?;
        }
        Ok(())
    }
}

/// Walk only AND edges, never interpreting an opaque leaf as a conjunction.
/// Mark direct leaves first so a second, nested path cannot count them anew.
fn visit(
    builder: &mut Builder<'_, '_, '_>,
    metadata: &Metadata,
    scratch: &mut Scratch,
    mut condition: impl FnMut(&mut Builder<'_, '_, '_>, usize, usize) -> Result<(), FormulaFailure>,
) -> Result<(), FormulaFailure> {
    for atom in 0..builder.catalog.len() {
        let location = metadata.location(atom);
        builder.work(location)?;
        let Some(body) = producer(metadata, atom) else {
            continue;
        };
        // Eligible producer roots bound the prefix. A later direct-only body
        // has no nested incidence and must not address its smaller mark owner.
        if body >= scratch.seen.len() {
            continue;
        }
        let count = match builder.nodes.view().node(body).expect("owned producer") {
            Node::And(row) => row.len(),
            _ => continue,
        };
        let generation = atom.checked_add(1).expect("allocated atom position");
        scratch.pending.clear();
        for position in 0..count {
            builder.work(location)?;
            let child = super::child(builder, body, position);
            if matches!(
                builder.nodes.view().node(child).expect("owned conjunct"),
                Node::And(_)
            ) {
                scratch.enqueue(builder, child, generation, location)?;
            } else {
                scratch.seen.slice_mut()[child] = generation;
            }
        }
        let mut cursor = 0;
        while cursor < scratch.pending.len() {
            #[cfg(test)]
            tests::point(tests::Point::Visit);
            builder.work(location)?;
            let conjunction = scratch.pending.slice()[cursor];
            cursor += 1;
            let count = super::child_count(builder, conjunction);
            for position in 0..count {
                builder.work(location)?;
                let child = super::child(builder, conjunction, position);
                if scratch.seen.slice()[child] == generation {
                    continue;
                }
                if matches!(
                    builder.nodes.view().node(child).expect("owned conjunct"),
                    Node::And(_)
                ) {
                    scratch.enqueue(builder, child, generation, location)?;
                } else {
                    scratch.seen.slice_mut()[child] = generation;
                    if ![FALSUM, VERUM].contains(&child) {
                        condition(builder, atom, child)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn fill(
    plan: &mut Plan,
    builder: &mut Builder<'_, '_, '_>,
    metadata: &Metadata,
    head: &mut Option<(usize, usize)>,
    atom: usize,
    condition: usize,
) -> Result<(), FormulaFailure> {
    let group = plan.directory.slice()[condition]
        .expect("counted condition")
        .get()
        - 1;
    let group = &mut plan.groups.slice_mut()[group];
    if group.length < 2 {
        return Ok(());
    }
    #[cfg(test)]
    tests::point(tests::Point::Fill);
    let location = metadata.location(atom);
    builder.work(location)?;
    let node = match *head {
        Some((prior, node)) if prior == atom => node,
        _ => {
            let node = builder.node(Node::Atom(atom), location)?;
            *head = Some((atom, node));
            node
        }
    };
    plan.heads.slice_mut()[group.start + group.filled] = node;
    group.filled += 1;
    Ok(())
}
