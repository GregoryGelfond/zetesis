//! Complete flat keys select one original row through existing membership.
//!
//! The indexed fallback first preserves an existing zero/singleton posting.
//! Wider complete patterns can refine that domain with the point query. Partial
//! patterns keep their postings, and prior structural/domain/table routes keep
//! precedence. The caller still performs matching and scalar evaluation.

use std::ops::Range;

use super::{BindingView, FormulaFailure, GroundingWork, PositivePattern, Support};
use crate::formula_support::delta;
use crate::formula_support::relations::RelationRows;
use zetesis_core::TemplateTerm;

#[cfg(test)]
mod tests;

/// The posting still needs the caller's ordinary pattern and delta selection;
/// an exact range already includes the original-occurrence delta restriction.
pub(in crate::formula_support) enum IndexedSelection<'source> {
    Posting(Option<&'source [usize]>),
    Exact(Range<usize>),
}

impl<'source> Support<'source> {
    /// Resolve the existing posting once. Empty/singleton candidates already
    /// bound fan-out by one, so the ordinary matcher finishes those queries.
    /// Only wider candidates can justify an additional complete-tuple lookup.
    /// Both index operations belong to the one already-recorded logical probe.
    pub(in crate::formula_support) fn indexed_selection_at(
        &self,
        rows: Option<&'source RelationRows<'source>>,
        pattern: PositivePattern<'_>,
        values: BindingView<'_>,
        partition: (Option<usize>, usize),
        work: GroundingWork<'_>,
    ) -> Result<IndexedSelection<'source>, FormulaFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let posting = self.probe_at(rows, pattern.atom(), values, limits, counters, location)?;
        let candidates =
            posting.map_or_else(|| rows.map_or(0, RelationRows::row_count), <[usize]>::len);
        if candidates > 1
            && let Some(range) = self.exact_at(
                rows,
                pattern,
                values,
                partition,
                GroundingWork::new(limits, counters, location),
            )?
        {
            return Ok(IndexedSelection::Exact(range));
        }
        Ok(IndexedSelection::Posting(posting))
    }

    /// `None` declines the route; `Some(0..0)` is an established empty query.
    /// A singleton always uses the original predicate-local row identity.
    /// This refinement records no second logical probe; its caller already
    /// owns that event. Every performed lookup operation still consumes work.
    pub(in crate::formula_support) fn exact_at(
        &self,
        rows: Option<&RelationRows<'_>>,
        pattern: PositivePattern<'_>,
        values: BindingView<'_>,
        (pivot, occurrence): (Option<usize>, usize),
        work: GroundingWork<'_>,
    ) -> Result<Option<Range<usize>>, FormulaFailure> {
        #[cfg(test)]
        if !tests::enabled() {
            return Ok(None);
        }
        let (Some(rows), PositivePattern::Flat(pattern)) = (rows, pattern) else {
            return Ok(None);
        };
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        Self::admit(self.live_bytes(), limits, counters, location)?;
        let terms = pattern.terms();
        for column in 0..terms.len() {
            counters.work(limits, location)?;
            if let TemplateTerm::Variable(variable) =
                terms.at(column).expect("admitted flat pattern column")
            {
                if variable >= values.len() {
                    return Err(FormulaFailure::UnsafeVariable { variable, location });
                }
                if values.get(variable).is_none() {
                    return Ok(None);
                }
            }
        }
        // AtomKey checks the complete argument sequence again. Charge that
        // actual scan before constructing its allocation-free borrowed view.
        counters.charge_work(terms.len() as u128, limits, location)?;
        let key = pattern
            .key(values)
            .expect("all referenced flat slots were checked present");
        let scoped = self.indexed_limits(limits, counters, location)?;
        let row = self
            .relations
            .lookup_key_at(rows, &key, &scoped, counters, location)
            .map_err(|error| self.indexed_failure(error))?;
        let selected = if let Some(row) = row {
            let partition = delta::interval(pivot, occurrence, rows.old_rows(), rows.row_count());
            counters.work(limits, location)?;
            let after_start = row >= partition.start;
            counters.work(limits, location)?;
            let before_end = row < partition.end;
            (after_start && before_end).then_some(row)
        } else {
            None
        };
        counters.work(limits, location)?;
        #[cfg(test)]
        tests::record();
        Ok(Some(selected.map_or(0..0, |row| row..row + 1)))
    }
}
