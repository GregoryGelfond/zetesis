//! One coordinator-owned assignment replays an exact positive witness row.
//!
//! Workers return only source occurrence coordinates. This owner reuses the
//! existing row-column projection and canonical assignment import, never an
//! invented head, duplicate matcher or full copied frame per completion.

#[cfg(test)]
mod tests;

use zetesis_core::atom_registry::RowColumn;

use super::{
    Binding, Buffer, Computation, Context, FormulaFailure, GroundingWork, StorageLease,
    WitnessPlan, WitnessRow, owner,
};
use crate::ExpansionFailure;
use crate::formula_ir::LiteralIr;
use crate::formula_support::decided_negations_hold;
use crate::formula_support::evaluation::Evaluation;
use crate::formula_support::heads::row_column;
use crate::formula_support::integer_comparisons;
use crate::formula_support::row_blocks::prime::ResidualSkip;
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

    pub(crate) fn assignment(&self) -> &Binding<'static> {
        &self.values
    }

    /// Apply body binds, residual-suffix comparisons, and decided default
    /// negation to the restored assignment. `false` omits the row, matching a
    /// serial walk that rejected the same binding.
    pub(crate) fn replay_residuals(
        &mut self,
        skip: ResidualSkip,
        decided: impl Iterator<Item = usize>,
        context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let rule = self.plan.base.rule;
        for (variable, column) in self.columns.iter().enumerate() {
            if column.is_none() {
                self.values.clear(variable, limits, counters, location)?;
            }
        }
        let mut evaluation = Evaluation::default();
        for (index, literal) in rule.body.iter().enumerate() {
            counters.work(limits, location)?;
            match literal {
                LiteralIr::Bind { target, value } => {
                    match crate::formula_support::expression(
                        value,
                        &self.values,
                        computation,
                        limits,
                        counters,
                        location,
                    ) {
                        Ok(key) => {
                            self.values.set(*target, &key, limits, counters, location)?;
                        }
                        Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })) => {
                            return Ok(false);
                        }
                        Err(error) => return Err(error),
                    }
                }
                LiteralIr::Compare(comparison) | LiteralIr::ArgumentCheck(comparison)
                    if skip.contains(index) =>
                {
                    let decided = integer_comparisons::decide(
                        comparison,
                        &self.values,
                        Context::new(&*computation, limits, counters, location),
                    )?;
                    let holds = if let Some(holds) = decided {
                        holds
                    } else {
                        match evaluation.source_comparison(
                            comparison.operands(),
                            &self.values,
                            Context::new(computation, limits, counters, location),
                        ) {
                            Ok(holds) => holds,
                            Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                                ..
                            })) => {
                                return Ok(false);
                            }
                            Err(error) => return Err(error),
                        }
                    };
                    if !holds {
                        return Ok(false);
                    }
                }
                _ => {}
            }
        }
        decided_negations_hold(rule, decided, &self.values, computation, limits, counters)
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
