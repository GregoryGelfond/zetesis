//! Unit-count bounds across Boolean witnesses.
//!
//! Flat theories use the sparse least-condition anchor join and exact-prefix
//! authentication. Its preparation is O(R + U log U + L + (R + C) log(A + 1))
//! for R native rows, U unit rows, C chains, L chain leaves and A distinct row
//! anchors. Retained storage is O(U + C); there is no row-by-chain table. With
//! no unit row, preparation allocates nothing and never examines chains.
//!
//! A shared associative DAG can hide an intermediate cut such as [C,b,d]
//! inside ((C or b) or d). Opaque and fully unfolded minima do not identify
//! every such cut. For a nested Boolean event, walk its same-kind cone and
//! filter its distinct node IDs against the existing row anchors. For a native
//! event, follow the anchor's existing immediate parent edges through same-kind
//! chains. Both discoveries are temporary joins, without an ancestor table.
//!
//! Candidates are then authenticated in their actual direction. A Boolean
//! witness implies a row witness when every terminal path meets a row condition;
//! stop at those conditions, including compounds. A row witness implies a
//! OR witness when every row condition occurs in its same-kind cone. A true
//! AND entails all k contained conditions, so a native exclusive upper bound
//! at most k refutes that AND, even when the row has additional terms.
//! For OR the witness is truth; for AND it is falsity. These implications do
//! not require independent conditions. Native rows remain distinct unit terms.
//!
//! One requested cone or ancestor walk visits O(V + E) reachable nodes/edges.
//! Cover authentication adds O(V log(Ur + 1)) comparisons for Ur row terms;
//! containment adds O(Ur) marked-node reads. Discovery adds anchor searches and
//! posting visits. All runtime visits/comparisons are metered and cancellable.
//! Dense buckets may still be expensive. Worker scratch is O(N + E + R + C),
//! reserved lazily with checked allocation and actual-capacity accounting;
//! cloned region knowledge and the flat counter path remain unchanged.
//!
//! Frozen subjects skip this bridge: these are original-subject consequences,
//! never an assumption about arbitrary frozen-reduct interpretations.

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
    /// Retained anchor projections; the enclosing Thresholds owns the header.
    pub(super) fn retained_bytes(&self) -> u128 {
        self.rows.retained_bytes()
            + self.chains.retained_bytes()
            + self.anchors.capacity() as u128 * size_of::<usize>() as u128
            + self.by_chain.capacity() as u128 * size_of::<(usize, usize)>() as u128
    }

    pub(super) fn prepare(
        rows: &[Row],
        chains: &[Chain],
        extrema: &crate::regions::extrema::Extrema,
        shared_connectives: bool,
    ) -> Result<Option<Self>, Stop> {
        let anchors = anchor_keys(rows)?;
        if anchors.is_empty() {
            // In particular, a Boolean-only theory does not inspect chains.
            return Ok(None);
        }
        let by_chain = chain_anchors(chains, extrema, &anchors)?;
        if by_chain.is_empty() && !shared_connectives {
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

    /// A cone node is only a discovery key, not proof of a row relation.
    fn rows_at(&self, node: usize, work: &mut Work<'_>) -> Result<&[usize], Stop> {
        let Some(anchor) = find(&self.anchors, node, |&condition| condition, work)? else {
            return Ok(&[]);
        };
        Ok(&self.rows[anchor])
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
fn chain_anchors(
    chains: &[Chain],
    extrema: &crate::regions::extrema::Extrema,
    anchors: &[usize],
) -> Result<Vec<(usize, usize)>, Stop> {
    let mut reverse = Vec::new();
    for (chain, data) in chains.iter().enumerate() {
        let Some(first) = data.leaves(extrema).iter().min() else {
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

fn bound(disjunction: bool, leaves: &[usize]) -> i128 {
    if disjunction { 1 } else { leaves.len() as i128 }
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
        let expanded = crate::regions::witnesses::expands(index, chain);
        let rows = if expanded {
            // Each distinct cone node has at most one anchor slot, and each
            // row has one anchor. Thus this temporary join emits no row twice.
            self.lists.witnesses.cone(index, chain, work)?;
            self.lists.witnesses.clear_candidates();
            for position in 0..self.lists.witnesses.prepared().len() {
                let node = self.lists.witnesses.prepared()[position];
                for &row in anchors.rows_at(node, work)? {
                    work.tick()?;
                    self.lists.witnesses.push_candidate(row)?;
                }
            }
            &[][..]
        } else {
            anchors.rows(chain, work)?
        };
        let count = if expanded {
            self.lists.witnesses.candidates().len()
        } else {
            rows.len()
        };
        let mut step = Step::Unchanged;
        // Authentication reuses witness scratch, so release each candidate
        // borrow before walking its cone. Candidate IDs remain unchanged.
        let mut position = 0;
        while position < count {
            let row = if expanded {
                self.lists.witnesses.candidates()[position]
            } else {
                rows[position]
            };
            position += 1;
            work.tick()?;
            let data = &index.thresholds.rows[row];
            let length = data.terms.len();
            let state = self.known.thresholds[row];
            let bound = if expanded && !source.disjunction {
                length as i128
            } else {
                bound(source.disjunction, source.leaves(&index.extrema))
            };
            let useful = if value {
                state.lower < bound
            } else {
                state.upper.is_none_or(|upper| bound < upper)
            };
            if !useful {
                continue;
            }
            let authenticated = if expanded {
                self.lists
                    .witnesses
                    .covered_by(index, chain, work, |node, work| {
                        contains(&data.terms, node, work)
                    })?
            } else {
                let compatible = if value {
                    source.leaves(&index.extrema).len() <= length
                } else {
                    source.leaves(&index.extrema).len() >= length
                };
                compatible && exact_prefix(&data.terms, source.leaves(&index.extrema), work)?
            };
            if !authenticated {
                continue;
            }
            let state = &mut self.known.thresholds[row];
            if value {
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
        let shared = !index.witness_chains.is_empty();
        let chains = if shared {
            // Discovery follows one connective per branch. An irrelevant
            // polarity stays irrelevant along that branch. A known absorbing
            // truth (OR sure / AND never) already entails that same truth at
            // every ancestor; ordinary node events complete it if still pending.
            // Keep the opposite-known polarity: authentication may refute it.
            let sure = &*self.known.sure;
            let never = &*self.known.never;
            self.lists
                .witnesses
                .parent_chains(index, first, work, |chain| {
                    if chain.disjunction {
                        source.lower >= 1 && !bit(sure, chain.root)
                    } else {
                        source.upper.is_some_and(|upper| upper <= length as i128)
                            && !bit(never, chain.root)
                    }
                })?;
            &[][..]
        } else {
            anchors.chains(first, work)?
        };
        let count = if shared {
            self.lists.witnesses.candidates().len()
        } else {
            chains.len()
        };
        // Cone walks mutate scratch without changing this candidate sequence.
        let mut position = 0;
        while position < count {
            let chain_index = if shared {
                self.lists.witnesses.candidates()[position]
            } else {
                chains[position]
            };
            position += 1;
            work.tick()?;
            let chain = &index.chains[chain_index];
            // Keep the cheap truth and bound filters before any cone walk.
            let sure = chain.disjunction && source.lower >= 1 && !bit(self.known.sure, chain.root);
            let never = !chain.disjunction
                && source.upper.is_some_and(|upper| upper <= length as i128)
                && !bit(self.known.never, chain.root);
            if !(sure || never) {
                continue;
            }
            let expanded = crate::regions::witnesses::expands(index, chain_index);
            let authenticated = if expanded {
                self.lists.witnesses.cone(index, chain_index, work)?;
                let mut contained = 0;
                for &(condition, _) in &data.terms {
                    work.tick()?;
                    contained += usize::from(self.lists.witnesses.visited(condition));
                }
                if sure {
                    contained == length
                } else {
                    // A true AND makes every contained row condition true.
                    // Retain the exact-prefix inference when this native row
                    // has extra terms but its exclusive upper bound is tight.
                    source.upper.is_some_and(|upper| upper <= contained as i128)
                }
            } else {
                let bound = bound(chain.disjunction, chain.leaves(&index.extrema));
                let compatible =
                    (sure && length <= chain.leaves(&index.extrema).len() && source.lower >= bound)
                        || (never
                            && length >= chain.leaves(&index.extrema).len()
                            && source.upper.is_some_and(|upper| upper <= bound));
                compatible && exact_prefix(&data.terms, chain.leaves(&index.extrema), work)?
            };
            if !authenticated {
                continue;
            }
            step = step.join(if sure {
                self.sure(chain.root)
            } else {
                self.never(chain.root)
            });
            if step == Step::Contradiction {
                return Ok(step);
            }
        }
        Ok(step)
    }
}
