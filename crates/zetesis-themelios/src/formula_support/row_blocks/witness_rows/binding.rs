//! One coordinator-owned assignment replays an exact positive witness row.
//!
//! Workers return only source occurrence coordinates. This owner reuses the
//! existing row-column projection and canonical assignment import, never an
//! invented head, duplicate matcher or full copied frame per completion.

#[cfg(test)]
mod tests;

use zetesis_core::atom_interner::RowColumn;

use super::{
    Binding, Buffer, Computation, Context, FormulaFailure, GroundingWork, StorageLease,
    WitnessPlan, WitnessRow, owner,
};
use crate::formula_support::heads::row_column;

pub(crate) struct RowBinding<'plan, 'source> {
    plan: &'plan WitnessPlan<'plan, 'source>,
    columns: Buffer<Option<RowColumn>>,
    values: Binding<'static>,
    _lease: StorageLease,
}

impl<'plan, 'source: 'plan> RowBinding<'plan, 'source> {
    pub(crate) fn new(
        plan: &'plan WitnessPlan<'plan, 'source>,
        row: &WitnessRow<'_, 'source>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        check_row(plan, row, limits, counters, location)?;
        let mut lease = computation.lease();
        let header = size_of::<Self>()
            - size_of::<Buffer<Option<RowColumn>>>()
            - size_of::<Binding<'static>>();
        lease.observe(header, location)?;
        computation.storage_observed(&lease, 0, header, limits, counters, location)?;
        let mut columns = Buffer::new(computation, limits, counters, location)?;
        columns.reserve(
            plan.base.rule.variables,
            computation,
            limits,
            counters,
            location,
        )?;
        for variable in 0..plan.base.rule.variables {
            let column = row_column(variable, row, limits, counters, location)?;
            columns.push(column, computation, limits, counters, location)?;
        }
        let mut values = Binding::new(computation, limits, counters, location)?;
        values.extend_scope(
            plan.base.rule.variables,
            computation,
            limits,
            counters,
            location,
        )?;
        Ok(Self {
            plan,
            columns,
            values,
            _lease: lease,
        })
    }

    /// Every selected column is imported afresh. No inferred unchanged prefix
    /// licenses reuse of a slot after a refused or partially completed replay.
    pub(crate) fn read(
        &mut self,
        row: &WitnessRow<'_, 'source>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<&Binding<'static>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        check_row(self.plan, row, limits, counters, location)?;
        for (variable, column) in self.columns.iter().enumerate() {
            counters.work(limits, location)?;
            let Some(column) = column else {
                continue;
            };
            let value = row
                .rows
                .get(column.input)
                .and_then(|source| source.value(column.column))
                .ok_or_else(|| owner(location))?;
            self.values
                .set_term_with(variable, value, computation.read(), location, || {
                    counters.work(limits, location)
                })?;
        }
        Ok(&self.values)
    }
}

fn check_row(
    plan: &WitnessPlan<'_, '_>,
    row: &WitnessRow<'_, '_>,
    limits: &super::FormulaLimits,
    counters: &mut super::Counters,
    location: super::ProgramSite,
) -> Result<(), FormulaFailure> {
    counters.work(limits, location)?;
    if !std::ptr::eq(row.patterns, plan.base.plan.patterns.as_slice()) {
        return Err(owner(location));
    }
    counters.work(limits, location)?;
    if !std::ptr::eq(row.resolutions, plan.sources.slice()) {
        return Err(owner(location));
    }
    counters.work(limits, location)?;
    if row.rows.len() != plan.base.plan.patterns.len()
        || !std::ptr::eq(row.support.relations(), plan.relations)
    {
        return Err(owner(location));
    }
    Ok(())
}
