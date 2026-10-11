//! Remap detached families without copying their row once per guard.

use super::{
    Buffer, Builder, Event, FALSUM, FormulaFailure, FormulaSuffix, Node, ProgramSite, nodes, remap,
};
use zetesis_ferraris::{AdmissionError, ExtremumCutRule, NumericDomain};

/// The forward links group every occurrence of one source row, even when its
/// guards are not adjacent. A first guard's children precede it, so all bounds of
/// that row can be published there without changing any dependency. Later source
/// positions reuse the prepared map; roots and origins still follow source order.
struct Families {
    next: Buffer<Option<usize>>,
    mapped: Buffer<Option<usize>>,
}

impl Families {
    fn new(
        builder: &mut Builder<'_, '_, '_>,
        suffix: &FormulaSuffix,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let mut heads = buffer(builder, location)?;
        let mut next = buffer(builder, location)?;
        let mut mapped = buffer(builder, location)?;
        heads.resize(
            suffix.parts().rows().len(),
            None,
            builder.computation,
            builder.limits,
            &mut builder.counters,
            location,
        )?;
        next.resize(
            suffix.view().len(),
            None,
            builder.computation,
            builder.limits,
            &mut builder.counters,
            location,
        )?;
        mapped.resize(
            suffix.view().len(),
            None,
            builder.computation,
            builder.limits,
            &mut builder.counters,
            location,
        )?;
        for (ordinal, node) in suffix.parts().nodes().iter().enumerate().rev() {
            builder.work(location)?;
            let row = if let Some(descriptor) = node.threshold_descriptor() {
                Some(
                    suffix
                        .parts()
                        .thresholds()
                        .get(descriptor)
                        .map(|entry| entry.row),
                )
            } else {
                node.extremum_cut_descriptor().map(|descriptor| {
                    suffix
                        .parts()
                        .extremum_cuts()
                        .get(descriptor)
                        .map(|entry| entry.row)
                })
            };
            if let Some(row) = row {
                let row = row.and_then(|row| heads.slice_mut().get_mut(row)).ok_or(
                    FormulaFailure::Theory {
                        error: AdmissionError::Span,
                        location,
                    },
                )?;
                next.slice_mut()[ordinal] = *row;
                *row = Some(ordinal);
            }
        }
        Ok(Self { next, mapped })
    }

    fn entries(
        &self,
        builder: &mut Builder<'_, '_, '_>,
        suffix: &FormulaSuffix,
        ordinal: usize,
        location: ProgramSite,
    ) -> Result<Buffer<nodes::Entry>, FormulaFailure> {
        let mut entries = buffer(builder, location)?;
        let mut cursor = Some(ordinal);
        while let Some(position) = cursor {
            builder.work(location)?;
            builder.counters.record(Event::NodeLookup);
            let (_, rule) = nodes::Row::read(
                suffix
                    .view()
                    .node(position)
                    .map_err(|error| FormulaFailure::Theory { error, location })?,
            )
            .expect("family contains only numeric nodes");
            entries.push(
                nodes::Entry::new(rule),
                builder.computation,
                builder.limits,
                &mut builder.counters,
                location,
            )?;
            cursor = self.next.slice()[position];
        }
        Ok(entries)
    }

    fn intern(
        &mut self,
        builder: &mut Builder<'_, '_, '_>,
        suffix: &FormulaSuffix,
        ordinal: usize,
        canonical: &[usize],
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        if let Some(node) = self.mapped.slice()[ordinal] {
            return Ok(node);
        }
        let (row, _) = nodes::Row::read(
            suffix
                .view()
                .node(ordinal)
                .map_err(|error| FormulaFailure::Theory { error, location })?,
        )
        .expect("family starts at a numeric node");
        let mut conditions = buffer(builder, location)?;
        for &condition in row.conditions {
            conditions.push(
                remap(condition, suffix.first(), canonical),
                builder.computation,
                builder.limits,
                &mut builder.counters,
                location,
            )?;
        }
        let mut entries = self.entries(builder, suffix, ordinal, location)?;
        let row = nodes::Row {
            conditions: conditions.slice(),
            ..row
        };
        match row.domain {
            NumericDomain::Weights => {
                let mut pending = filled(builder, entries.len(), 0, location)?;
                intern_family(
                    builder,
                    row,
                    entries.slice_mut(),
                    nodes::Pending::Weights(pending.slice_mut()),
                    location,
                )?;
            }
            NumericDomain::Ordinal { .. } => {
                let mut pending =
                    filled(builder, entries.len(), ExtremumCutRule::Prefix(0), location)?;
                intern_family(
                    builder,
                    row,
                    entries.slice_mut(),
                    nodes::Pending::Ordinal(pending.slice_mut()),
                    location,
                )?;
            }
        }
        // The mapping scan is charged before interning so a stopped publication
        // cannot leave a partially initialized local family map.
        let mut position = ordinal;
        for entry in entries.iter() {
            self.mapped.slice_mut()[position] = Some(entry.node);
            if entry.inserted {
                builder.counters.record(Event::NodeInserted);
            }
            if let Some(next) = self.next.slice()[position] {
                position = next;
            }
        }
        Ok(self.mapped.slice()[ordinal].expect("family was mapped"))
    }
}

/// All index scratch and the final map scan are admitted before publication.
fn intern_family(
    builder: &mut Builder<'_, '_, '_>,
    row: nodes::Row<'_>,
    entries: &mut [nodes::Entry],
    pending: nodes::Pending<'_>,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    let slots = entries.len().checked_mul(2).ok_or(FormulaFailure::Theory {
        error: AdmissionError::Allocation,
        location,
    })?;
    let mut table = filled(builder, slots, None, location)?;
    let mut matched_rows = filled(builder, slots, None, location)?;
    builder
        .counters
        .charge_work(entries.len() as u128, builder.limits, location)?;
    let bounds = [builder.node_bound(), builder.operand_bound()];
    nodes::intern_family(
        &mut builder.node_indices,
        &mut builder.nodes,
        nodes::Family {
            row,
            entries,
            pending,
            table: table.slice_mut(),
            matched_rows: matched_rows.slice_mut(),
        },
        bounds,
        location,
        || builder.counters.work(builder.limits, location),
    )
}

fn buffer<T>(
    builder: &mut Builder<'_, '_, '_>,
    location: ProgramSite,
) -> Result<Buffer<T>, FormulaFailure> {
    Buffer::new(
        builder.computation,
        builder.limits,
        &mut builder.counters,
        location,
    )
}

fn filled<T: Copy>(
    builder: &mut Builder<'_, '_, '_>,
    length: usize,
    value: T,
    location: ProgramSite,
) -> Result<Buffer<T>, FormulaFailure> {
    let mut values = buffer(builder, location)?;
    values.resize(
        length,
        value,
        builder.computation,
        builder.limits,
        &mut builder.counters,
        location,
    )?;
    Ok(values)
}

impl Builder<'_, '_, '_> {
    pub(super) fn intern_appended(
        &mut self,
        appended: &FormulaSuffix,
        location: ProgramSite,
    ) -> Result<Buffer<usize>, FormulaFailure> {
        let first = appended.first();
        let mut canonical = buffer(self, location)?;
        let mut families = if appended.parts().rows().is_empty() {
            None
        } else {
            Some(Families::new(self, appended, location)?)
        };
        for ordinal in 0..appended.view().len() {
            self.work(location)?;
            let node = appended
                .view()
                .node(ordinal)
                .map_err(|error| FormulaFailure::Theory { error, location })?;
            let index = match node {
                Node::False => FALSUM,
                Node::Atom(atom) => self.node(Node::Atom(atom), location)?,
                Node::And(row) | Node::Or(row) => self.mapped_group(
                    row,
                    matches!(node, Node::And(_)),
                    |child| remap(child, first, canonical.slice()),
                    location,
                )?,
                Node::Threshold(_) | Node::ExtremumCut(_) => families
                    .as_mut()
                    .expect("numeric suffix has rows")
                    .intern(self, appended, ordinal, canonical.slice(), location)?,
                Node::Implies(left, right) => self.node(
                    Node::Implies(
                        remap(left, first, canonical.slice()),
                        remap(right, first, canonical.slice()),
                    ),
                    location,
                )?,
            };
            canonical.push(
                index,
                self.computation,
                self.limits,
                &mut self.counters,
                location,
            )?;
        }
        Ok(canonical)
    }
}

#[cfg(test)]
mod tests;
