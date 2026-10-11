//! Exact-prefix transport between native unit rows and live Boolean chains.
//!
//! A chain's distinct leaves denote a unit sum: its conjunction has the leaf
//! count as bound, and its disjunction has bound one. Existing chain counters
//! still perform all local Boolean propagation. Only authenticated exact
//! prefixes exchange original-subject requirements with native row state.
//!
//! A nonempty native row is eligible exactly when its greatest grouped weight
//! is one. Its terms are then distinct conditions in ascending order. Every
//! nonempty exact prefix or extension has the same first condition. A dirty
//! native row probes that condition's existing Boolean parent posting; a
//! Boolean event finds its least leaf and probes the existing native condition
//! posting. Distinct chain leaves and grouped row terms give one posting per
//! candidate. Empty rows and chains keep their local constant rules.
//!
//! Preparation records only whether any eligible row exists, taking O(R)
//! constant-time row checks and constant retained space. It neither inspects
//! chains nor retains row/chain pairs. With no eligible row, Boolean events
//! perform no bridge scans. A Boolean event otherwise scans its m leaves for
//! the minimum; each event or dirty row then visits P posting entries and
//! authenticates candidate pairs in O(sum(m log(k + 1))) comparisons, where k
//! is the shorter row length. Every leaf visit, posting entry and membership
//! comparison consumes propagation work before inspection. Searches repeat
//! when a row's requirements tighten; no hidden preparation allowance pays
//! for them. The existing coalesced dirty-row worklist is the only scratch.
//!
//! Bounds tighten monotonically; only processed Boolean truth events introduce
//! their fixed endpoint bound. Frozen subjects skip the bridge. Atom ranking,
//! mutable knowledge and the logical index receipt are unchanged.

use std::cmp::Ordering;

use zetesis_cpu::Stop;

use crate::regions::{Chain, Closure, Count, Narrower, Step, Work, bit, chain_position};

use super::{Row, Thresholds};

#[cfg(test)]
mod tests;

/// Presence means that at least one grouped row can denote a Boolean unit sum.
/// Candidate discovery is deferred to charged original-subject events.
#[derive(Clone, Debug)]
pub(super) struct Connectives;

impl Connectives {
    pub(super) fn prepare(rows: &[Row]) -> Option<Self> {
        rows.iter()
            .any(|row| unit_first(row).is_some())
            .then_some(Self)
    }
}

fn unit_first(row: &Row) -> Option<usize> {
    let &(first, 1) = row.terms.first()? else {
        return None;
    };
    Some(first)
}

/// Authenticate the shorter canonical row without reordering borrowed leaves.
/// Distinct leaves at or below the prefix's last condition must all belong to
/// the prefix, and their count must equal its length. Thus no earlier condition
/// is missing or replaced by an unrelated leaf. Larger leaves are extensions.
fn exact_prefix(
    terms: &[(usize, i128)],
    leaves: &[usize],
    work: &mut Work<'_>,
) -> Result<bool, Stop> {
    let prefix = &terms[..terms.len().min(leaves.len())];
    let Some(&(last, _)) = prefix.last() else {
        return Ok(false);
    };
    let mut matched = 0;
    for &leaf in leaves {
        work.tick()?;
        if leaf <= last {
            if !contains(prefix, leaf, work)? {
                return Ok(false);
            }
            matched += 1;
        }
    }
    Ok(matched == prefix.len())
}

/// The candidate interval shrinks on every comparison. Each actual term read
/// consumes work, including the comparison that finds or rejects membership.
fn contains(terms: &[(usize, i128)], leaf: usize, work: &mut Work<'_>) -> Result<bool, Stop> {
    let mut start = 0;
    let mut end = terms.len();
    while start < end {
        work.tick()?;
        let middle = start + (end - start) / 2;
        match terms[middle].0.cmp(&leaf) {
            Ordering::Less => start = middle + 1,
            Ordering::Equal => return Ok(true),
            Ordering::Greater => end = middle,
        }
    }
    Ok(false)
}

fn bound(chain: &Chain) -> i128 {
    if chain.disjunction {
        1
    } else {
        chain.operands.len() as i128
    }
}

impl Thresholds {
    pub(super) fn has_requirement_links(&self, row: usize) -> bool {
        !self.related(row).is_empty()
            || (self.connectives.is_some() && unit_first(&self.rows[row]).is_some())
    }
}

impl<C: Count> Closure<'_, C> {
    /// A processed true Boolean prefix supplies a lower bound to its native
    /// extension; a false Boolean extension supplies an exclusive upper bound
    /// to its native prefix. Each candidate is authenticated before transport.
    pub(in crate::regions) fn threshold_connective(
        &mut self,
        index: &Narrower,
        chain: usize,
        value: bool,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let source = &index.chains[chain];
        if index.thresholds.connectives.is_none() || value != source.disjunction {
            // teach_chain already forced every leaf of a true And or false Or.
            // Processed condition events establish the same lower bound (all
            // prefix leaves true) or a stronger upper bound (all terms false)
            // wherever that Boolean requirement could be transported.
            return Ok(Step::Unchanged);
        }
        let mut first = None;
        for &leaf in &source.operands {
            work.tick()?;
            first = Some(first.map_or(leaf, |prior: usize| prior.min(leaf)));
        }
        let Some(first) = first else {
            return Ok(Step::Unchanged);
        };
        let bound = bound(source);
        let mut step = Step::Unchanged;
        for &edge in index.thresholds.condition(first) {
            work.tick()?;
            let row = index.thresholds.incidences[edge].row;
            let data = &index.thresholds.rows[row];
            if unit_first(data) != Some(first) {
                continue;
            }
            let length = data.terms.len();
            let state = self.known.thresholds[row];
            let lower = value && source.operands.len() <= length && state.lower < bound;
            let upper = !value
                && source.operands.len() >= length
                && state.upper.is_none_or(|upper| bound < upper);
            if !(lower || upper) || !exact_prefix(&data.terms, &source.operands, work)? {
                continue;
            }
            let state = &mut self.known.thresholds[row];
            if lower {
                state.lower = bound;
            } else {
                state.upper = Some(bound);
            }
            self.lists.queue_threshold(row);
            step = step.join(self.threshold_row(index, row, None, work)?);
            if step == Step::Contradiction {
                return Ok(step);
            }
            step = Step::Changed;
        }
        Ok(step)
    }

    /// Compose native-prefix transport with Boolean endpoints discovered for
    /// the same dirty requirement. Both consume original row state and stop at
    /// the first contradiction, without charging the remaining neighbours.
    pub(in crate::regions) fn threshold_requirements(
        &mut self,
        index: &Narrower,
        row: usize,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let mut step = self.threshold_prefixes(index, row, work)?;
        if step == Step::Contradiction || index.thresholds.connectives.is_none() {
            return Ok(step);
        }
        let data = &index.thresholds.rows[row];
        let Some(first) = unit_first(data) else {
            return Ok(step);
        };
        let source = self.known.thresholds[row];
        let length = data.terms.len();
        if source.lower < 1 && source.upper.is_none_or(|upper| upper > length as i128) {
            return Ok(step);
        }
        for &parent in &index.parents[first] {
            work.tick()?;
            let Some(chain) = index.chain_of[parent] else {
                continue;
            };
            let chain = &index.chains[chain_position(chain)];
            let bound = bound(chain);
            // Full lower bounds and zero upper bounds already force every
            // leaf through threshold_row. Ordinary chain events therefore
            // establish true And and false Or without another prefix join.
            let sure = chain.disjunction
                && length <= chain.operands.len()
                && source.lower >= bound
                && !bit(self.known.sure, chain.root);
            let never = !chain.disjunction
                && length >= chain.operands.len()
                && source.upper.is_some_and(|upper| upper <= bound)
                && !bit(self.known.never, chain.root);
            if !(sure || never) || !exact_prefix(&data.terms, &chain.operands, work)? {
                continue;
            }
            if sure {
                step = step.join(self.sure(chain.root));
                if step == Step::Contradiction {
                    return Ok(step);
                }
            }
            if never {
                step = step.join(self.never(chain.root));
                if step == Step::Contradiction {
                    return Ok(step);
                }
            }
        }
        Ok(step)
    }
}
