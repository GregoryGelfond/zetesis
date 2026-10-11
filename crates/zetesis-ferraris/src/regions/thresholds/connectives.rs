//! Exact-prefix transport between native unit rows and live Boolean chains.
//!
//! A chain's distinct leaves denote a unit sum: its conjunction has the leaf
//! count as bound, and its disjunction has bound one. Existing chain counters
//! still perform all local Boolean propagation. Only authenticated exact
//! prefixes exchange original-subject requirements with native row state.
//!
//! A nonempty native row is eligible exactly when its greatest grouped weight
//! is one. Its terms are then distinct conditions in ascending order. Every
//! nonempty exact prefix or extension has the same least condition, its anchor.
//! Immutable projections associate each unit row and each matching nonempty
//! chain with one anchor. They select candidates, never establish a prefix.
//! Empty rows and chains keep their local constant rules. Borrowed chain leaves
//! retain their order, and each posting retains the original row or chain order.
//!
//! For R native rows, U eligible rows, C Boolean chains, L total chain leaves
//! and A distinct native anchors, preparation takes O(R + U log U + L +
//! C log(A + 1) + R log(A + 1)) work. Each chain computes its minimum once.
//! With B matching chains, retained logical storage is A anchor keys, two
//! A + 1 offset vectors, U row IDs, B chain IDs and B reverse (chain,anchor)
//! entries: O(U + B) words including bounded vector spare capacity. Compact
//! construction adds O(A) cursors; every added reservation is fallible. No
//! row/chain pairs, node-sized offsets or per-region state are added. With no
//! eligible native row, preparation allocates nothing and never reads chains.
//!
//! A Boolean event looks up its reverse entry in O(log(B + 1)); a dirty native
//! row looks up its anchor in O(log(A + 1)). Each then visits only that anchor's
//! opposite posting. Cheap truth, bound and length filters precede exact-prefix
//! authentication, which costs O(sum(m log(k + 1))) for candidate chain lengths
//! m and shorter row lengths k. Every runtime lookup comparison, posting entry,
//! leaf visit and membership comparison consumes propagation work. Dense anchor
//! buckets can still be expensive, but no product is retained. Requirements
//! tighten monotonically; the existing coalesced row worklist is the only scratch.
//!
//! Frozen subjects skip the bridge. Atom ranking, mutable knowledge and the
//! logical index receipt are unchanged. Anchor construction, like native row
//! sorting, is explicit preparation rather than charged propagation work.

use std::cmp::Ordering;

use zetesis_cpu::Stop;

use crate::regions::{Chain, Closure, Count, Narrower, Step, Work, bit};

use super::{Adjacency, Row, Thresholds, reserve};

#[cfg(test)]
mod anchors_tests;
#[cfg(test)]
mod tests;

/// One immutable anchor relation projected to native rows and Boolean chains.
/// Anchor slots cover only distinct first conditions of grouped unit rows;
/// no node-sized offsets or authenticated row/chain pairs are stored.
#[derive(Clone, Debug)]
pub(super) struct Connectives {
    anchors: Vec<usize>,
    rows: Adjacency,
    chains: Adjacency,
    /// Sparse reverse projection, ordered by original chain ID.
    by_chain: Vec<(usize, usize)>,
}

impl Connectives {
    pub(super) fn prepare(rows: &[Row], chains: &[Chain]) -> Result<Option<Self>, Stop> {
        let anchors = anchor_keys(rows)?;
        if anchors.is_empty() {
            // In particular, a Boolean-only theory does not inspect chains.
            return Ok(None);
        }
        let by_chain = chain_anchors(chains, &anchors)?;
        if by_chain.is_empty() {
            return Ok(None);
        }
        let row_postings = Adjacency::build(
            anchors.len(),
            rows.iter().enumerate().filter_map(|(row, data)| {
                unit_first(data).map(|first| {
                    let anchor = anchors.binary_search(&first).expect("prepared row anchor");
                    (anchor, row)
                })
            }),
        )?;
        let chain_postings = Adjacency::build(
            anchors.len(),
            by_chain.iter().map(|&(chain, anchor)| (anchor, chain)),
        )?;
        Ok(Some(Self {
            anchors,
            rows: row_postings,
            chains: chain_postings,
            by_chain,
        }))
    }

    /// Find a processed chain's one anchor, charging each reverse-index probe.
    fn rows(&self, chain: usize, work: &mut Work<'_>) -> Result<&[usize], Stop> {
        let Some(position) = find(&self.by_chain, chain, |&(chain, _)| chain, work)? else {
            return Ok(&[]);
        };
        Ok(&self.rows[self.by_chain[position].1])
    }

    /// Find a native row's anchor, charging each immutable key probe.
    fn chains(&self, first: usize, work: &mut Work<'_>) -> Result<&[usize], Stop> {
        let Some(anchor) = find(&self.anchors, first, |&condition| condition, work)? else {
            return Ok(&[]);
        };
        Ok(&self.chains[anchor])
    }
}

/// Allocate only after finding a native unit row. Sorting and deduplication
/// retain one key per distinct anchor; the allocation is bounded by unit rows.
fn anchor_keys(rows: &[Row]) -> Result<Vec<usize>, Stop> {
    let count = rows.iter().filter(|row| unit_first(row).is_some()).count();
    let mut anchors = reserve(count)?;
    anchors.extend(rows.iter().filter_map(unit_first));
    anchors.sort_unstable();
    anchors.dedup();
    Ok(anchors)
}

/// Scan each nonempty chain's borrowed leaves once, without reordering them.
/// Only a minimum shared with some native unit row receives a reverse entry.
/// Increasing chain IDs preserve the old parent-posting order at every anchor.
fn chain_anchors(chains: &[Chain], anchors: &[usize]) -> Result<Vec<(usize, usize)>, Stop> {
    let mut reverse = Vec::new();
    for (chain, data) in chains.iter().enumerate() {
        let Some(first) = data.operands.iter().min() else {
            continue;
        };
        if let Ok(anchor) = anchors.binary_search(first) {
            reverse.try_reserve(1).map_err(|_| Stop::Allocation)?;
            reverse.push((chain, anchor));
        }
    }
    Ok(reverse)
}

/// A shrinking binary-search interval; charge before reading each candidate.
fn find<T>(
    values: &[T],
    wanted: usize,
    key: impl Fn(&T) -> usize,
    work: &mut Work<'_>,
) -> Result<Option<usize>, Stop> {
    let mut start = 0;
    let mut end = values.len();
    while start < end {
        work.tick()?;
        let middle = start + (end - start) / 2;
        match key(&values[middle]).cmp(&wanted) {
            Ordering::Less => start = middle + 1,
            Ordering::Equal => return Ok(Some(middle)),
            Ordering::Greater => end = middle,
        }
    }
    Ok(None)
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
    Ok(find(terms, leaf, |&(condition, _)| condition, work)?.is_some())
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
        if value != source.disjunction {
            // teach_chain already forced every leaf of a true And or false Or.
            // Processed condition events establish the same lower bound (all
            // prefix leaves true) or a stronger upper bound (all terms false)
            // wherever that Boolean requirement could be transported.
            return Ok(Step::Unchanged);
        }
        let Some(anchors) = &index.thresholds.connectives else {
            return Ok(Step::Unchanged);
        };
        let bound = bound(source);
        let mut step = Step::Unchanged;
        for &row in anchors.rows(chain, work)? {
            work.tick()?;
            let data = &index.thresholds.rows[row];
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
        if step == Step::Contradiction {
            return Ok(step);
        }
        let Some(anchors) = &index.thresholds.connectives else {
            return Ok(step);
        };
        let data = &index.thresholds.rows[row];
        let Some(first) = unit_first(data) else {
            return Ok(step);
        };
        let source = self.known.thresholds[row];
        let length = data.terms.len();
        if source.lower < 1 && source.upper.is_none_or(|upper| upper > length as i128) {
            return Ok(step);
        }
        for &chain in anchors.chains(first, work)? {
            work.tick()?;
            let chain = &index.chains[chain];
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
