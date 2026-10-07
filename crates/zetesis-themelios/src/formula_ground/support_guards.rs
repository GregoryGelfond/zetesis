//! Transpose complete necessary-support incidences without flattening shared bodies.
//!
//! A single recorded producer B supplies h -> B. For its direct conjunction
//! children c, the family of double-negated guards is equivalent to one
//! double-negated (OR requiring heads) -> c per child. Original truth agrees by
//! finite incidence distribution; double negation freezes that same truth for
//! arbitrary M/J. Producer discovery is complete before this module runs.

use std::num::NonZeroUsize;

use super::{Builder, FALSUM, Node, VERUM, metadata::Metadata};
use crate::formula_support::Buffer;
use crate::{FormulaFailure, FormulaResource, ProgramSite};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
struct Group {
    condition: usize,
    first_atom: usize,
    start: usize,
    length: usize,
    filled: usize,
}

/// These buffers own only coordinates into the same immutable formula prefix.
/// Each Buffer has its sole workspace lease; the enclosing stack value adds no
/// second charge for its embedded headers. Native graph storage remains separate.
struct Plan {
    directory: Buffer<Option<NonZeroUsize>>,
    groups: Buffer<Group>,
    heads: Buffer<usize>,
}

impl Builder<'_, '_, '_> {
    pub(super) fn support_guards(&mut self) -> Result<(), FormulaFailure> {
        let metadata = std::mem::take(&mut self.metadata);
        #[cfg(test)]
        if !tests::enabled() {
            return self.unshared_guards(&metadata);
        }
        let Some(plan) = Plan::prepare(self, &metadata)? else {
            return self.unshared_guards(&metadata);
        };
        // Unselected guards keep their atom order. Transposed groups follow in
        // first-incidence order; ordinary roots and their evidence are unchanged.
        for atom in 0..self.catalog.len() {
            self.work(metadata.location(atom))?;
            if producer(&metadata, atom).is_none() {
                self.support_guard(&metadata, atom)?;
            }
        }
        for group in plan.groups.iter() {
            let heads = &plan.heads.slice()[group.start..group.start + group.length];
            self.transposed_guard(&metadata, *group, heads)?;
        }
        Ok(())
    }

    fn unshared_guards(&mut self, metadata: &Metadata) -> Result<(), FormulaFailure> {
        for atom in 0..self.catalog.len() {
            self.support_guard(metadata, atom)?;
        }
        Ok(())
    }

    fn support_guard(&mut self, metadata: &Metadata, atom: usize) -> Result<(), FormulaFailure> {
        let location = metadata.location(atom);
        let mut supported = FALSUM;
        for antecedent in metadata.producers(atom) {
            self.work(location)?;
            supported = self.or(supported, antecedent, location)?;
        }
        let head = self.node(Node::Atom(atom), location)?;
        let necessary = self.node(Node::Implies(head, supported), location)?;
        let negative = self.neg(necessary, location)?;
        let guard = self.neg(negative, location)?;
        self.admit_root(metadata.origins(atom).len(), location)?;
        let origins = metadata.copy_origins(atom, &mut self.counters, self.limits)?;
        self.publish_root(guard, origins);
        Ok(())
    }

    fn transposed_guard(
        &mut self,
        metadata: &Metadata,
        group: Group,
        heads: &[usize],
    ) -> Result<(), FormulaFailure> {
        let location = metadata.location(group.first_atom);
        #[cfg(test)]
        tests::point(tests::Point::Publish);
        // The counted row is complete and nonempty. Retain duplicate operands;
        // they cost storage/work even though they do not change disjunction truth.
        let premise = match heads {
            [only] => *only,
            row => self.node(Node::Or(row), location)?,
        };
        let necessary = self.node(Node::Implies(premise, group.condition), location)?;
        let negative = self.neg(necessary, location)?;
        let guard = self.neg(negative, location)?;
        self.admit_root(0, location)?;
        let evidence = self.guard_evidence(metadata, heads, location)?;
        self.admit_root(evidence.len(), location)?;
        // The temporary union is accounted separately. Final root-owned evidence
        // is copied only after both public emitted-evidence ceilings admit it.
        self.counters
            .charge_work(evidence.len() as u128, self.limits, location)?;
        let mut origins = Vec::new();
        origins
            .try_reserve_exact(evidence.len())
            .map_err(|error| FormulaFailure::MetadataAllocation { error, location })?;
        origins.extend_from_slice(evidence.slice());
        self.publish_root(guard, origins);
        Ok(())
    }

    fn guard_evidence(
        &mut self,
        metadata: &Metadata,
        heads: &[usize],
        location: ProgramSite,
    ) -> Result<Buffer<ProgramSite>, FormulaFailure> {
        #[cfg(test)]
        tests::point(tests::Point::Evidence);
        let mut evidence: Buffer<ProgramSite> =
            Buffer::new(self.computation, self.limits, &mut self.counters, location)?;
        for &head in heads {
            self.work(location)?;
            let Node::Atom(atom) = self.nodes.view().node(head).expect("owned head") else {
                unreachable!("the incidence arena contains only atom nodes")
            };
            for origin in metadata.origins(atom) {
                self.work(location)?;
                let (mut start, mut end) = (0, evidence.len());
                let mut present = false;
                while start < end {
                    self.work(location)?;
                    let middle = start + (end - start) / 2;
                    match evidence.slice()[middle].cmp(&origin) {
                        std::cmp::Ordering::Less => start = middle + 1,
                        std::cmp::Ordering::Greater => end = middle,
                        std::cmp::Ordering::Equal => {
                            present = true;
                            break;
                        }
                    }
                }
                if !present {
                    self.counters.charge_work(
                        (evidence.len() - start) as u128,
                        self.limits,
                        location,
                    )?;
                    evidence.push(
                        origin,
                        self.computation,
                        self.limits,
                        &mut self.counters,
                        location,
                    )?;
                    evidence.slice_mut()[start..].rotate_right(1);
                }
            }
        }
        Ok(evidence)
    }
}

/// Metadata repetitions are not coalesced to manufacture singleton eligibility.
fn producer(metadata: &Metadata, atom: usize) -> Option<usize> {
    let mut producers = metadata.producers(atom);
    if producers.len() != 1 {
        return None;
    }
    producers
        .next()
        .filter(|body| ![FALSUM, VERUM].contains(body))
}

fn child_count(builder: &Builder<'_, '_, '_>, body: usize) -> usize {
    match builder.nodes.view().node(body).expect("owned producer") {
        Node::And(row) => row.len(),
        _ => 1,
    }
}

fn child(builder: &Builder<'_, '_, '_>, body: usize, index: usize) -> usize {
    match builder.nodes.view().node(body).expect("owned producer") {
        Node::And(row) => row[index],
        _ => body,
    }
}

impl Plan {
    fn prepare(
        builder: &mut Builder<'_, '_, '_>,
        metadata: &Metadata,
    ) -> Result<Option<Self>, FormulaFailure> {
        // No sharing is possible with fewer than two heads. In particular a
        // lone unsupported atom retains the existing evidence-admission order.
        if builder.catalog.len() < 2 {
            return Ok(None);
        }
        let (mut selected, mut incidences) = (0usize, 0u128);
        for atom in 0..builder.catalog.len() {
            let location = metadata.location(atom);
            builder.work(location)?;
            if let Some(body) = producer(metadata, atom) {
                builder.work(location)?;
                let count = child_count(builder, body);
                // A vacuous row has no incidence to carry its source evidence.
                // Retain the original path even if a future owner admits it.
                if count == 0 {
                    return Ok(None);
                }
                selected += 1;
                incidences += count as u128;
            }
        }
        // At least two nonempty rows give I >= 2. For K groups, S singleton
        // groups and D wide incidences, E = 6K + D = I + 6K - S >= I + 6:
        // K=1 gives S=0; K>=2 gives S<=K. Reject an impossible operand price
        // before allocating the directory. This does not establish sharing.
        if selected < 2 || incidences + 6 > 6 * selected as u128 {
            return Ok(None);
        }
        let location = metadata.location(0);
        let mut directory = Buffer::new(
            builder.computation,
            builder.limits,
            &mut builder.counters,
            location,
        )?;
        directory.resize(
            builder.nodes.view().len(),
            None,
            builder.computation,
            builder.limits,
            &mut builder.counters,
            location,
        )?;
        let mut plan = Self {
            directory,
            groups: Buffer::new(
                builder.computation,
                builder.limits,
                &mut builder.counters,
                location,
            )?,
            heads: Buffer::new(
                builder.computation,
                builder.limits,
                &mut builder.counters,
                location,
            )?,
        };
        plan.count(builder, metadata)?;
        if !plan.improves(builder, metadata, selected)? {
            return Ok(None);
        }
        #[cfg(test)]
        tests::accepted();
        plan.fill(builder, metadata)?;
        Ok(Some(plan))
    }

    fn count(
        &mut self,
        builder: &mut Builder<'_, '_, '_>,
        metadata: &Metadata,
    ) -> Result<(), FormulaFailure> {
        for atom in 0..builder.catalog.len() {
            let location = metadata.location(atom);
            builder.work(location)?;
            let Some(body) = producer(metadata, atom) else {
                continue;
            };
            for index in 0..child_count(builder, body) {
                #[cfg(test)]
                tests::point(tests::Point::Count);
                builder.work(location)?;
                let condition = child(builder, body, index);
                let group = if let Some(group) = self.directory.slice()[condition] {
                    group.get() - 1
                } else {
                    let group = self.groups.len();
                    // A new group belongs to a distinct node of the owned prefix,
                    // so group < prefix length <= usize::MAX before this increment.
                    let stored = group
                        .checked_add(1)
                        .and_then(NonZeroUsize::new)
                        .expect("each group names a distinct owned prefix node");
                    self.groups.push(
                        Group {
                            condition,
                            first_atom: atom,
                            start: 0,
                            length: 0,
                            filled: 0,
                        },
                        builder.computation,
                        builder.limits,
                        &mut builder.counters,
                        location,
                    )?;
                    self.directory.slice_mut()[condition] = Some(stored);
                    group
                };
                let length = self.groups.slice()[group].length as u128 + 1;
                super::ceiling(
                    FormulaResource::Operands,
                    length,
                    usize::MAX as u128,
                    location,
                )?;
                self.groups.slice_mut()[group].length =
                    usize::try_from(length).expect("checked length");
            }
        }
        Ok(())
    }

    /// A source-independent price gate precedes head-row or formula publication.
    /// These are conservative shape prices before canonical interning, not a
    /// promise that all three nodes of an old guard would have been new.
    fn improves(
        &self,
        builder: &mut Builder<'_, '_, '_>,
        metadata: &Metadata,
        selected: usize,
    ) -> Result<bool, FormulaFailure> {
        let (mut wide, mut operands) = (0u128, 0u128);
        for group in self.groups.iter() {
            builder.work(metadata.location(group.first_atom))?;
            if group.length > 1 {
                wide += 1;
                operands += group.length as u128;
            }
        }
        let groups = self.groups.len() as u128;
        let selected = selected as u128;
        Ok(groups <= selected
            && 3 * groups + wide < 3 * selected
            && 6 * groups + operands <= 6 * selected)
    }

    fn fill(
        &mut self,
        builder: &mut Builder<'_, '_, '_>,
        metadata: &Metadata,
    ) -> Result<(), FormulaFailure> {
        let mut total = 0usize;
        for group in self.groups.slice_mut() {
            let location = metadata.location(group.first_atom);
            builder.work(location)?;
            group.start = total;
            let next = total as u128 + group.length as u128;
            super::ceiling(
                FormulaResource::Operands,
                next,
                usize::MAX as u128,
                location,
            )?;
            total = usize::try_from(next).expect("checked incidence population");
        }
        let location = metadata.location(0);
        self.heads.resize(
            total,
            0,
            builder.computation,
            builder.limits,
            &mut builder.counters,
            location,
        )?;
        for atom in 0..builder.catalog.len() {
            let location = metadata.location(atom);
            builder.work(location)?;
            let Some(body) = producer(metadata, atom) else {
                continue;
            };
            let head = builder.node(Node::Atom(atom), location)?;
            for index in 0..child_count(builder, body) {
                #[cfg(test)]
                tests::point(tests::Point::Fill);
                builder.work(location)?;
                let condition = child(builder, body, index);
                let group = self.directory.slice()[condition]
                    .expect("counted condition has a group")
                    .get()
                    - 1;
                let group = &mut self.groups.slice_mut()[group];
                self.heads.slice_mut()[group.start + group.filled] = head;
                group.filled += 1;
            }
        }
        debug_assert!(self.groups.iter().all(|group| group.filled == group.length));
        Ok(())
    }
}
