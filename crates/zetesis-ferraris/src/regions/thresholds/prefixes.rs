//! Exact-prefix requirements between prepared native rows.
//!
//! Grouped terms retain their existing descending-weight order. An exact
//! prefix therefore has the same conditions and coefficients as the start of
//! its extension, so its sum is no greater under every interpretation. A
//! prefix's required lower bound holds for its extension; an extension's
//! exclusive upper bound holds for its prefix. Equal term lists transport
//! both requirements. This inference is used only on original subjects.
//!
//! Lexicographic sorting borrows the term lists; adjacent longest-common-prefix
//! scans and a stack find the nearest present prefix without testing all pairs.
//! Equal lists form a linear chain. The resulting forest has at most R - 1
//! links for R rows, stored twice in checked compact adjacency. Preparation
//! uses O(R) extra words, O(R log R * K + E) worst-case term work for maximum
//! row length K and E grouped terms, and leaves the existing logical index
//! receipt unchanged. It creates neither guard pairs nor new formula nodes.

use zetesis_cpu::Stop;

use crate::regions::{Closure, Count, Narrower, Step, Work};

use super::{Adjacency, Row, reserve};

#[cfg(test)]
mod tests;

pub(super) fn prepare(rows: &[Row]) -> Result<Option<Adjacency>, Stop> {
    let links = links(rows)?;
    if links.is_empty() {
        return Ok(None);
    }
    Adjacency::build(
        rows.len(),
        links
            .iter()
            .flat_map(|&(prefix, extension)| [(prefix, extension), (extension, prefix)]),
    )
    .map(Some)
}

/// The stack contains one latest representative of each prefix length on the
/// preceding row's path. An adjacent LCP discards every branch that cannot
/// prefix the next row. A same-length match replaces the representative,
/// linking equal rows as a chain rather than forming all equal pairs.
fn links(rows: &[Row]) -> Result<Vec<(usize, usize)>, Stop> {
    if rows.len() < 2 {
        return Ok(Vec::new());
    }
    let mut order = reserve(rows.len())?;
    order.extend(0..rows.len());
    order.sort_unstable_by(|&left, &right| {
        rows[left]
            .terms
            .cmp(&rows[right].terms)
            .then(left.cmp(&right))
    });
    let mut ancestors: Vec<usize> = reserve(rows.len())?;
    let mut links = reserve(rows.len() - 1)?;
    let mut previous: Option<usize> = None;
    for row in order {
        let common = previous.map_or(0, |previous| {
            rows[previous]
                .terms
                .iter()
                .zip(&rows[row].terms)
                .take_while(|(left, right)| left == right)
                .count()
        });
        while ancestors
            .last()
            .is_some_and(|&ancestor| rows[ancestor].terms.len() > common)
        {
            ancestors.pop();
        }
        if let Some(&prefix) = ancestors.last() {
            links.push((prefix, row));
            if rows[prefix].terms.len() == rows[row].terms.len() {
                ancestors.pop();
            }
        }
        ancestors.push(row);
        previous = Some(row);
    }
    Ok(links)
}

impl<C: Count> Closure<'_, C> {
    /// Consume one dirty row and its actual neighbour visits. Tightening a
    /// requirement queues the target even when it has no guard at that bound;
    /// absent intermediate guards therefore cannot interrupt transitive closure.
    /// Each value comes from a processed guard and moves monotonically, so each
    /// guard requirement can successfully tighten a target across each edge at
    /// most once per direction; neighbour visits may repeat.
    /// No arithmetic is needed, including for extreme admitted i128 bounds.
    pub(in crate::regions) fn threshold_prefixes(
        &mut self,
        index: &Narrower,
        row: usize,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        work.tick()?;
        let source = self.known.thresholds[row];
        let length = index.thresholds.rows[row].terms.len();
        let mut step = Step::Unchanged;
        for &target in index.thresholds.related(row) {
            work.tick()?;
            let target_length = index.thresholds.rows[target].terms.len();
            let state = &mut self.known.thresholds[target];
            let mut changed = false;
            if length <= target_length && state.lower < source.lower {
                state.lower = source.lower;
                changed = true;
            }
            if length >= target_length
                && let Some(upper) = source.upper
                && state.upper.is_none_or(|prior| upper < prior)
            {
                state.upper = Some(upper);
                changed = true;
            }
            if changed {
                self.lists.queue_threshold(target);
                step = step.join(self.threshold_row(index, target, None, work)?);
                if step == Step::Contradiction {
                    return Ok(step);
                }
                step = Step::Changed;
            }
        }
        Ok(step)
    }
}
