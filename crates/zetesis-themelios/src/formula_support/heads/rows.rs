//! One source head projects exact matched relation rows into canonical storage.
//!
//! Preparation owns only source/column metadata. It is shared by possible-support
//! production and formula emission, with no claim about truth or exhaustiveness.

use zetesis_core::TemplateTerm;
use zetesis_core::atom_interner::{PreparedRows, RowColumn};

use super::{AtomPattern, Computation, Counters, StorageLease};
use crate::formula_support::witnesses::WitnessRow;
use crate::formula_support::{Context, GroundingWork, SourceAtom, reserve};
use crate::{FormulaFailure, FormulaLimits, ProgramSite};

pub(crate) struct RowHead<'source> {
    pattern: PreparedRows<'source, 'source>,
    _lease: StorageLease,
}

impl<'source> RowHead<'source> {
    /// Called only for a selected complete positive row. Preparation maps each
    /// variable to its first original column in the chosen traversal order;
    /// matched equality already established any later occurrences' agreement.
    pub(crate) fn new(
        pattern: AtomPattern,
        row: &WitnessRow<'_, 'source>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let components = row
            .support
            .components()
            .ok_or_else(|| crate::formula_support::components::missing(location))?;
        let pattern = pattern.get(components, limits, counters, location)?;
        let mut lease = computation.lease();
        lease.observe(size_of::<Self>(), location)?;
        computation.storage_observed(&lease, 0, size_of::<Self>(), limits, counters, location)?;
        let mut sources = Vec::new();
        reserve(
            &mut sources,
            row.rows.len(),
            &mut lease,
            size_of::<Self>(),
            Context::new(computation, limits, counters, location),
        )?;
        for resolution in row.resolutions {
            counters.work(limits, location)?;
            sources.push(
                &resolution
                    .rows()
                    .expect("selected positive source")
                    .relation,
            );
        }
        let mut columns = Vec::new();
        let other = lease.bytes();
        reserve(
            &mut columns,
            pattern.terms().len(),
            &mut lease,
            other,
            Context::new(computation, limits, counters, location),
        )?;
        for index in 0..pattern.terms().len() {
            counters.work(limits, location)?;
            let column = match pattern.terms().at(index).expect("checked head arity") {
                TemplateTerm::Constant(_) => None,
                TemplateTerm::Variable(variable) => {
                    let column = project(variable, row, limits, counters, location)?
                        .ok_or(FormulaFailure::UnsafeVariable { variable, location })?;
                    Some(column)
                }
            };
            columns.push(column);
        }
        // The eligible source class has only relational bindings, every head
        // slot is complete, and the support snapshot owns admitted canonical
        // patterns and relations. A declined core capability therefore denotes
        // an invalid source owner, rather than a second execution route.
        let prepared = computation
            .prepare_rows(
                pattern,
                sources,
                columns,
                GroundingWork::new(limits, counters, location),
            )?
            .ok_or(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location,
            })?;
        let bytes = size_of::<Self>() as u128 + prepared.storage_bytes()
            - size_of::<PreparedRows<'_, '_>>() as u128;
        let bytes = usize::try_from(bytes).map_err(|_| FormulaFailure::Limit {
            resource: crate::FormulaResource::SupportBytes,
            observed: bytes,
            limit: limits.max_support_bytes as u128,
            location,
        })?;
        // The vectors moved without changing capacities: their existing
        // lease becomes the final plan's sole retained storage receipt.
        lease.observe(bytes, location)?;
        Ok(Self {
            pattern: prepared,
            _lease: lease,
        })
    }

    pub(crate) fn atom(
        &self,
        row: &WitnessRow<'_, '_>,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<SourceAtom, FormulaFailure> {
        computation.row_atom(&self.pattern, row.rows, limits, counters, location)
    }
}

fn project(
    variable: usize,
    row: &WitnessRow<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Option<RowColumn>, FormulaFailure> {
    for (input, occurrence) in row.patterns.iter().enumerate() {
        let terms = occurrence.atom().terms();
        for column in 0..terms.len() {
            counters.work(limits, location)?;
            if matches!(terms.at(column), Some(TemplateTerm::Variable(found)) if found == variable)
            {
                return Ok(Some(RowColumn { input, column }));
            }
        }
    }
    Ok(None)
}
