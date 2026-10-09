//! Necessary structural matches over exact completed source occurrences.
//!
//! Preparation runs the ordinary matcher with no incoming bindings. Every
//! later match adds equalities, so its row is in this ordered selection. Rows,
//! terms and candidate truth stay with their existing owners.

use super::super::{
    Computation, Context, Counters, GroundingWork, PositivePattern, StorageLease, order,
    relations::{RelationRows, Relations},
    reserve, reserve_exact,
};
use crate::formula_binding::Binding;
use crate::formula_pattern::MatchContext;
use crate::{FormulaFailure, ProgramSite};
use zetesis_core::catalog::{Atoms, TermRef};

pub(in crate::formula_support) struct PatternRows<'source> {
    entries: Vec<Option<Rows<'source>>>,
    lease: StorageLease,
    /// Exact named header and buffer capacity while preparation owns them.
    bytes: usize,
}

struct Rows<'source> {
    source: Atoms<'source>,
    positions: Vec<usize>,
}

impl<'source> PatternRows<'source> {
    pub(super) fn prepare(
        plan: &order::Plan<'source>,
        relations: &Relations<'source>,
        variables: usize,
        budget: &mut crate::expansion::Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Self>, FormulaFailure> {
        let mut structural = false;
        for occurrence in &plan.patterns {
            context
                .work
                .counters
                .work(context.work.limits, context.work.location)?;
            structural |= matches!(occurrence.pattern, PositivePattern::Structural(_));
        }
        if !structural {
            return Ok(None);
        }
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let mut result = Self::new(
            plan.patterns.len(),
            Context::new(computation, limits, counters, location),
        )?;
        let mut empty = Binding::new(computation, limits, counters, location)?;
        empty.extend_scope(variables, computation, limits, counters, location)?;
        let mut captures: Vec<(usize, TermRef<'source>)> = Vec::new();
        let mut scratch = computation.lease();
        scratch.observe(size_of_val(&captures), location)?;
        computation.storage_observed(
            &scratch,
            0,
            size_of_val(&captures),
            limits,
            counters,
            location,
        )?;
        let mut retained = false;
        for (depth, occurrence) in plan.patterns.iter().enumerate() {
            counters.work(limits, location)?;
            let PositivePattern::Structural(pattern) = occurrence.pattern else {
                continue;
            };
            let Some(source) =
                relations.find_with(pattern.atom().predicate(), limits, counters, location)?
            else {
                continue;
            };
            retained |= result.select(
                (depth, pattern),
                source,
                empty.view(computation.read(), limits, counters, location)?,
                computation,
                &mut captures,
                &mut MatchContext {
                    work: GroundingWork::new(limits, counters, location),
                    budget,
                    lease: &mut scratch,
                    other_bytes: size_of::<Vec<(usize, TermRef<'_>)>>(),
                },
            )?;
        }
        if !retained {
            return Ok(None);
        }
        // PreparedRule's admitted slot owns the inline header after publication.
        result
            .lease
            .observe(result.bytes - size_of::<Self>(), location)?;
        Ok(Some(result))
    }

    fn new(
        count: usize,
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
        let mut result = Self {
            entries: Vec::new(),
            lease: computation.lease(),
            bytes: size_of::<Self>(),
        };
        result.lease.observe(size_of::<Self>(), location)?;
        computation.storage_observed(
            &result.lease,
            0,
            size_of::<Self>(),
            limits,
            counters,
            location,
        )?;
        reserve_exact(
            &mut result.entries,
            count,
            &mut result.lease,
            size_of::<Self>(),
            Context::new(computation, limits, counters, location),
        )?;
        result.bytes = result.lease.bytes();
        for _ in 0..count {
            counters.work(limits, location)?;
            result.entries.push(None);
        }
        Ok(result)
    }

    fn select(
        &mut self,
        (depth, pattern): (usize, crate::formula_pattern::Pattern<'_>),
        source: &RelationRows<'source>,
        empty: zetesis_core::BindingView<'_>,
        computation: &Computation<'_, '_>,
        captures: &mut Vec<(usize, TermRef<'source>)>,
        context: &mut MatchContext<'_>,
    ) -> Result<bool, FormulaFailure> {
        let location = context.work.location;
        self.entries[depth] = Some(Rows {
            source: source.atoms,
            positions: Vec::new(),
        });
        for position in 0..source.row_count() {
            context.work.counters.work(context.work.limits, location)?;
            let matches = pattern.matches(
                source
                    .row(position)
                    .expect("position in completed relation"),
                empty,
                computation,
                captures,
                context,
            )?;
            captures.clear();
            if matches {
                self.push(
                    depth,
                    position,
                    Context::new(
                        computation,
                        context.work.limits,
                        context.work.counters,
                        location,
                    ),
                )?;
            }
        }
        if self.entries[depth]
            .as_ref()
            .expect("structural selection")
            .positions
            .len()
            == source.row_count()
        {
            // A complete list would narrow nothing. Its temporary capacity
            // was charged, but no per-rule copy of the carrier is retained.
            let rows = self.entries[depth].take().expect("complete selection");
            self.bytes -= rows.positions.capacity() * size_of::<usize>();
            drop(rows);
            self.lease.observe(self.bytes, location)?;
            Ok(false)
        } else {
            Ok(true)
        }
    }

    fn push(
        &mut self,
        depth: usize,
        position: usize,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        let old = self.entries[depth]
            .as_ref()
            .expect("selected source")
            .positions
            .capacity()
            * size_of::<usize>();
        let other = self.bytes - old;
        let positions = &mut self.entries[depth]
            .as_mut()
            .expect("selected source")
            .positions;
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        reserve(
            positions,
            1,
            &mut self.lease,
            other,
            Context::new(computation, limits, counters, location),
        )?;
        self.bytes = self.lease.bytes();
        // Reservation accounts capacity; publishing the coordinate is work too.
        counters.work(limits, location)?;
        positions.push(position);
        Ok(())
    }

    /// Authenticate before even comparing lengths or using an empty posting.
    pub(in crate::formula_support) fn posting(
        &self,
        depth: usize,
        source: Option<&RelationRows<'_>>,
        limits: &crate::FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<&[usize]>, FormulaFailure> {
        let Some(rows) = &self.entries[depth] else {
            return Ok(None);
        };
        counters.work(limits, location)?;
        if source.is_none_or(|source| !rows.source.same_occurrences(source.atoms)) {
            return Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location,
            });
        }
        Ok(Some(&rows.positions))
    }
}

#[cfg(test)]
mod tests;
