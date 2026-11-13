//! Complete flat keys select one original row through existing membership.
//!
//! The indexed fallback first preserves an existing zero/singleton posting.
//! A wider complete pattern refines that posting to the key's one row. Among
//! few candidates the equality identifiers its probe resolved decide it:
//! a row is the key's exactly when every column holds the key's identifier
//! (`ColumnRelations.accepts_exact`). Longer postings, and keys the probe did
//! not record, keep the ordered point query. Both select the same row. Partial
//! patterns keep their postings, and prior structural/domain/table routes keep
//! precedence. The caller still performs matching and scalar evaluation.

use std::ops::Range;

use super::{BindingView, FormulaFailure, GroundingWork, PositivePattern, Support};
use crate::formula_support::delta;
use crate::formula_support::relations::{KeyIdentifiers, RelationRows};
use zetesis_core::TemplateTerm;

#[cfg(test)]
mod tests;

/// The longest posting whose candidates identifier comparison settles.
///
/// A candidate costs at most one identifier comparison per column, so one
/// selection compares at most this many rows. The ordered point query instead
/// compares whole terms at a number of rows logarithmic in the relation, which
/// serves a long posting better. The bound chooses between two realizations
/// of one selection and never changes the selected row.
const IDENTIFIED_CANDIDATES: usize = 64;

/// The posting still needs the caller's ordinary pattern and delta selection;
/// an exact range already includes the original-occurrence delta restriction.
pub(in crate::formula_support) enum IndexedSelection<'source> {
    Posting(Option<&'source [usize]>),
    Exact(Range<usize>),
}

impl<'source> Support<'source> {
    /// Resolve the existing posting once. Empty/singleton candidates already
    /// bound fan-out by one, so the ordinary matcher finishes those queries.
    /// Only wider candidates can justify an additional complete-tuple
    /// selection: by the identifiers this probe resolved when they settle it,
    /// otherwise by the ordered point query. Every index operation belongs to
    /// the one already-recorded logical probe.
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
        let mut key = KeyIdentifiers::new();
        let posting = self.probe_identifying_at(
            rows,
            pattern.atom(),
            values,
            &mut key,
            GroundingWork::new(limits, counters, location),
        )?;
        let candidates =
            posting.map_or_else(|| rows.map_or(0, RelationRows::row_count), <[usize]>::len);
        if candidates > 1 {
            if let Some(range) = Self::identified_at(
                rows,
                pattern,
                (posting, &key),
                partition,
                GroundingWork::new(limits, counters, location),
            )? {
                return Ok(IndexedSelection::Exact(range));
            }
            if let Some(range) = self.exact_at(
                rows,
                pattern,
                values,
                partition,
                GroundingWork::new(limits, counters, location),
            )? {
                return Ok(IndexedSelection::Exact(range));
            }
        }
        Ok(IndexedSelection::Posting(posting))
    }

    /// Select a complete key's row among the candidates of its posting by
    /// equality identifiers alone.
    ///
    /// `None` declines the route: the pattern is not flat, its probe recorded
    /// no complete key, or the posting is longer than
    /// [`IDENTIFIED_CANDIDATES`]. `Some(0..0)` is an established empty query.
    ///
    /// A posting holds every row agreeing with the key on its own column, a
    /// row agrees with the whole key exactly when each column holds the key's
    /// identifier, and a relation holds a tuple once. The first agreeing
    /// candidate is therefore the key's only row, and no agreeing candidate
    /// means the key is absent. Each identifier comparison consumes one unit
    /// of work and nothing is allocated. This refinement records no second
    /// logical probe; its caller already owns that event.
    fn identified_at(
        rows: Option<&RelationRows<'_>>,
        pattern: PositivePattern<'_>,
        (posting, key): (Option<&[usize]>, &KeyIdentifiers),
        partition: (Option<usize>, usize),
        work: GroundingWork<'_>,
    ) -> Result<Option<Range<usize>>, FormulaFailure> {
        #[cfg(test)]
        if !tests::enabled() {
            return Ok(None);
        }
        let (Some(rows), PositivePattern::Flat(pattern), Some(posting)) = (rows, pattern, posting)
        else {
            return Ok(None);
        };
        if posting.len() > IDENTIFIED_CANDIDATES {
            return Ok(None);
        }
        let Some(identifiers) = key.complete(pattern.predicate().arity()) else {
            return Ok(None);
        };
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let mut located = None;
        'candidates: for &row in posting {
            for (column, &identifier) in rows.relation.columns().zip(identifiers) {
                counters.work(limits, location)?;
                let held = column
                    .get(row)
                    .expect("a posting names rows of its own relation");
                if held != identifier {
                    continue 'candidates;
                }
            }
            located = Some(row);
            break;
        }
        let range = Self::partitioned(
            located,
            rows,
            partition,
            GroundingWork::new(limits, counters, location),
        )?;
        #[cfg(test)]
        tests::record(tests::Route::Identifiers);
        Ok(Some(range))
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
        partition: (Option<usize>, usize),
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
        let range = Self::partitioned(
            row,
            rows,
            partition,
            GroundingWork::new(limits, counters, location),
        )?;
        #[cfg(test)]
        tests::record(tests::Route::Ordered);
        Ok(Some(range))
    }

    /// Keep a located row only inside its occurrence's delta partition. Each
    /// bound comparison and the final selection consume one unit of work.
    fn partitioned(
        row: Option<usize>,
        rows: &RelationRows<'_>,
        (pivot, occurrence): (Option<usize>, usize),
        work: GroundingWork<'_>,
    ) -> Result<Range<usize>, FormulaFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
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
        Ok(selected.map_or(0..0, |row| row..row + 1))
    }
}
