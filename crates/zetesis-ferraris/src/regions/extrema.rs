//! Borrowed witness columns for native ordinal cuts.
//!
//! The numeric row remains authoritative. For witness propagation only, repeated
//! condition IDs have a least and greatest rank. Prefix witnesses sort by the
//! least rank; suffix witnesses sort by the greatest rank in reverse. Every cut
//! borrows a prefix of one column. Two columns per used row and one descriptor
//! per cut retain O(E + G) space; no cut owns a copy of its qualifying leaves.
//!
//! Condition-to-column incidences store the first guard containing the condition.
//! Runtime streams those guards into the ordinary connective event counters.
//! Each condition event may therefore cost O(G), but no E-by-G transpose is
//! retained, and the processed-event invariant of region knowledge is unchanged.
//! Preparation sorts occurrences and guards in O(E log E + G log G). It has no
//! independent cancellation contract, like the enclosing narrower constructor.

use zetesis_cpu::Stop;

use crate::{ExtremumCutRule, NodeView, Theory};

use super::{Adjacency, Chain, ChainOperands, Closure, Count, Narrower, Step, Work, bit};

#[derive(Clone, Debug, Default)]
pub(super) struct Extrema {
    columns: Vec<Column>,
    cuts: Vec<(usize, usize, usize)>,
    constants: Vec<(usize, bool)>,
    incidences: Vec<Incidence>,
    by_condition: Option<Adjacency>,
}

#[derive(Clone, Debug)]
struct Column {
    conditions: Vec<usize>,
    /// (qualifying prefix length, original formula node), sorted lexically.
    guards: Vec<(usize, usize)>,
}

#[derive(Clone, Copy, Debug)]
struct Incidence {
    condition: usize,
    column: usize,
    first_guard: usize,
}

fn reserve<T>(length: usize) -> Result<Vec<T>, Stop> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| Stop::Allocation)?;
    Ok(result)
}

fn push<T>(values: &mut Vec<T>, value: T) -> Result<(), Stop> {
    values.try_reserve(1).map_err(|_| Stop::Allocation)?;
    values.push(value);
    Ok(())
}

impl Extrema {
    pub(super) fn prepare(theory: &Theory) -> Result<Self, Stop> {
        if theory.parts().extremum_cuts().is_empty() {
            return Ok(Self::default());
        }
        let mut result = Self::default();
        let mut rows = reserve(theory.parts().rows().len())?;
        rows.resize(theory.parts().rows().len(), None);
        // Temporary rank columns are released after all guard prefixes are read.
        let mut ranks: Vec<Vec<u64>> = Vec::new();
        for node in 0..theory.nodes().len() {
            let NodeView::ExtremumCut(cut) =
                theory.view().node(node).map_err(|_| Stop::InvalidProgram)?
            else {
                continue;
            };
            if let Some(value) = cut.endpoint_truth() {
                push(&mut result.constants, (node, value))?;
                continue;
            }
            let descriptor = theory.nodes()[node]
                .extremum_cut_descriptor()
                .ok_or(Stop::InvalidProgram)?;
            let source = theory.parts().extremum_cuts()[descriptor].row;
            let first = if let Some(first) = rows[source] {
                first
            } else {
                let first = result.append_row(cut.row, &mut ranks)?;
                rows[source] = Some(first);
                first
            };
            let (column, length) = match *cut.rule {
                ExtremumCutRule::Prefix(boundary) => {
                    (first, ranks[first].partition_point(|&rank| rank < boundary))
                }
                ExtremumCutRule::Suffix(boundary) => (
                    first + 1,
                    ranks[first + 1].partition_point(|&rank| rank >= boundary),
                ),
            };
            if length == 0 {
                push(&mut result.constants, (node, false))?;
                continue;
            }
            push(&mut result.columns[column].guards, (length, node))?;
            push(&mut result.cuts, (node, column, length))?;
        }
        for (column, data) in result.columns.iter_mut().enumerate() {
            data.guards.sort_unstable();
            for (position, &condition) in data.conditions.iter().enumerate() {
                let first_guard = data
                    .guards
                    .partition_point(|&(length, _)| length <= position);
                if first_guard != data.guards.len() {
                    push(
                        &mut result.incidences,
                        Incidence {
                            condition,
                            column,
                            first_guard,
                        },
                    )?;
                }
            }
        }
        if !result.incidences.is_empty() {
            result.by_condition = Some(Adjacency::build(
                theory.nodes().len(),
                result
                    .incidences
                    .iter()
                    .enumerate()
                    .map(|(index, edge)| (edge.condition, index)),
            )?);
        }
        Ok(result)
    }

    /// Derive two immutable witness orderings from one authoritative row.
    /// Duplicate conditions retain both extreme ranks without changing the row.
    fn append_row(
        &mut self,
        row: crate::ExtremumRow<'_>,
        ranks: &mut Vec<Vec<u64>>,
    ) -> Result<usize, Stop> {
        let mut grouped = reserve(row.conditions.len())?;
        grouped.extend(
            row.conditions
                .iter()
                .copied()
                .zip(row.ranks.iter().copied())
                .map(|(condition, rank)| (condition, rank, rank)),
        );
        grouped.sort_unstable_by_key(|&(condition, _, _)| condition);
        let mut distinct = 0;
        for position in 0..grouped.len() {
            let (condition, lo, hi) = grouped[position];
            if distinct > 0 && grouped[distinct - 1].0 == condition {
                grouped[distinct - 1].1 = grouped[distinct - 1].1.min(lo);
                grouped[distinct - 1].2 = grouped[distinct - 1].2.max(hi);
            } else {
                grouped[distinct] = (condition, lo, hi);
                distinct += 1;
            }
        }
        grouped.truncate(distinct);
        let first = self.columns.len();
        for suffix in [false, true] {
            if suffix {
                grouped
                    .sort_unstable_by_key(|&(condition, _, hi)| (std::cmp::Reverse(hi), condition));
            } else {
                grouped.sort_unstable_by_key(|&(condition, lo, _)| (lo, condition));
            }
            let mut conditions = reserve(distinct)?;
            let mut values = reserve(distinct)?;
            conditions.extend(grouped.iter().map(|&(condition, _, _)| condition));
            values.extend(
                grouped
                    .iter()
                    .map(|&(_, lo, hi)| if suffix { hi } else { lo }),
            );
            push(
                &mut self.columns,
                Column {
                    conditions,
                    guards: Vec::new(),
                },
            )?;
            push(ranks, values)?;
        }
        Ok(first)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.cuts.is_empty() && self.constants.is_empty()
    }

    /// Each nonconstant cut appends exactly one borrowed-operand Chain.
    pub(super) fn chain_count(&self) -> usize {
        self.cuts.len()
    }

    pub(super) fn leaves(&self, column: usize, length: usize) -> &[usize] {
        &self.columns[column].conditions[..length]
    }

    pub(super) fn append_chains(
        &self,
        chains: &mut Vec<Chain>,
        chain_of: &mut [Option<std::num::NonZeroUsize>],
    ) -> Result<(), Stop> {
        chains
            .try_reserve(self.cuts.len())
            .map_err(|_| Stop::Allocation)?;
        for &(root, column, length) in &self.cuts {
            chain_of[root] = Some(super::encoded_chain(chains.len()));
            chains.push(Chain {
                disjunction: true,
                root,
                operands: ChainOperands::Ordinal { column, length },
            });
        }
        Ok(())
    }

    /// Stream direct native parents without retaining logical occurrences.
    pub(super) fn parents(
        &self,
        condition: usize,
    ) -> impl DoubleEndedIterator<Item = usize> + Clone + '_ {
        let incidences = self
            .by_condition
            .as_ref()
            .map_or(&[][..], |index| &index[condition]);
        incidences.iter().flat_map(move |&incidence| {
            let edge = self.incidences[incidence];
            self.columns[edge.column].guards[edge.first_guard..]
                .iter()
                .map(|&(_, node)| node)
        })
    }

    pub(super) fn parent_count(&self, condition: usize) -> usize {
        let Some(index) = &self.by_condition else {
            return 0;
        };
        index[condition].iter().fold(0usize, |total, &incidence| {
            let edge = self.incidences[incidence];
            total
                .checked_add(self.columns[edge.column].guards.len() - edge.first_guard)
                .expect("logical formula occurrences fit admission")
        })
    }

    /// Actual payload capacities, including column headers. The containing
    /// narrower owns this value's inline header; scratch rank columns are gone.
    pub(super) fn retained_bytes(&self) -> u128 {
        self.columns.capacity() as u128 * size_of::<Column>() as u128
            + self
                .columns
                .iter()
                .map(|column| {
                    column.conditions.capacity() as u128 * size_of::<usize>() as u128
                        + column.guards.capacity() as u128 * size_of::<(usize, usize)>() as u128
                })
                .sum::<u128>()
            + self.cuts.capacity() as u128 * size_of::<(usize, usize, usize)>() as u128
            + self.constants.capacity() as u128 * size_of::<(usize, bool)>() as u128
            + self.incidences.capacity() as u128 * size_of::<Incidence>() as u128
            + self.by_condition.as_ref().map_or(0, |index| {
                index.retained_bytes() - size_of::<Adjacency>() as u128
            })
    }
}

impl<C: Count> Closure<'_, C> {
    pub(super) fn seed_extrema(
        &mut self,
        index: &Narrower,
        frozen: Option<&[bool]>,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let mut step = Step::Unchanged;
        for &(node, value) in &index.extrema.constants {
            work.tick()?;
            let value = value && frozen.is_none_or(|truth| truth[node]);
            step = step.join(if value {
                self.sure(node)
            } else {
                self.never(node)
            });
            if step == Step::Contradiction {
                break;
            }
        }
        Ok(step)
    }

    /// Cuts in one sorted column are nested disjunctions. A true shorter cut
    /// forces longer cuts; a false longer cut excludes shorter cuts. Original
    /// gates remain authoritative: an inactive frozen cut cannot teach a bound.
    pub(super) fn extremum_order(
        &mut self,
        index: &Narrower,
        chain: usize,
        frozen: Option<&[bool]>,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let source = &index.chains[chain];
        let ChainOperands::Ordinal { column, length } = source.operands else {
            return Ok(Step::Unchanged);
        };
        if frozen.is_some_and(|truth| !truth[source.root]) {
            return Ok(Step::Unchanged);
        }
        let value = bit(self.known.sure, source.root);
        let guards = &index.extrema.columns[column].guards;
        let mut step = Step::Unchanged;
        for &(size, node) in guards {
            work.tick()?;
            if (value && size < length)
                || (!value && size > length)
                || frozen.is_some_and(|truth| !truth[node])
            {
                continue;
            }
            step = step.join(if value {
                self.sure(node)
            } else {
                self.never(node)
            });
            if step == Step::Contradiction {
                break;
            }
        }
        Ok(step)
    }
}

#[cfg(test)]
mod tests;

/// A fingerprint only selects candidate equal disjunctions; it proves nothing.
/// Runtime authenticates exact distinct-leaf equality before transporting truth.
/// At most one entry per disjunction is retained, and only buckets containing a
/// native cut survive preparation. No row-pair or expanded-prefix table exists.
#[derive(Clone, Debug, Default)]
pub(super) struct EqualWitnesses {
    entries: Vec<(Signature, usize)>,
    by_chain: Vec<(usize, Signature)>,
    preparation_work: u64,
}

type Signature = (usize, usize, usize, usize);

impl EqualWitnesses {
    pub(super) fn prepare(index: &Narrower) -> Result<Self, Stop> {
        if index.extrema.cuts.is_empty() {
            return Ok(Self::default());
        }
        let mut scratch = super::witnesses::Scratch::default();
        let mut work = Work::new(u64::MAX);
        let mut entries = Vec::new();
        for (chain, data) in index.chains.iter().enumerate() {
            if !data.disjunction {
                continue;
            }
            let leaves = scratch.read(index, chain, None, &mut work)?;
            let mut signature = (leaves.len(), usize::MAX, 0usize, 0usize);
            for &leaf in leaves {
                work.tick()?;
                signature.1 = signature.1.min(leaf);
                // Wrapping is deliberate fingerprint arithmetic, not ASP arithmetic.
                signature.2 = signature.2.wrapping_add(leaf);
                signature.3 ^= leaf;
            }
            push(&mut entries, (signature, chain))?;
        }
        entries.sort_unstable();
        let mut retained = 0;
        let mut start = 0;
        while start < entries.len() {
            let key = entries[start].0;
            let mut end = start + 1;
            while end < entries.len() && entries[end].0 == key {
                end += 1;
            }
            if end - start > 1
                && entries[start..end]
                    .iter()
                    .any(|&(_, chain)| index.chains[chain].ordinal())
            {
                for position in start..end {
                    entries[retained] = entries[position];
                    retained += 1;
                }
            }
            start = end;
        }
        entries.truncate(retained);
        let mut by_chain = reserve(entries.len())?;
        by_chain.extend(entries.iter().map(|&(signature, chain)| (chain, signature)));
        by_chain.sort_unstable_by_key(|&(chain, _)| chain);
        Ok(Self {
            entries,
            by_chain,
            preparation_work: work.spent,
        })
    }

    pub(super) fn retained_bytes(&self) -> u128 {
        self.entries.capacity() as u128 * size_of::<(Signature, usize)>() as u128
            + self.by_chain.capacity() as u128 * size_of::<(usize, Signature)>() as u128
    }

    pub(super) fn preparation_work(&self) -> u64 {
        self.preparation_work
    }

    fn candidates(&self, chain: usize, work: &mut Work<'_>) -> Result<&[(Signature, usize)], Stop> {
        let mut start = 0;
        let mut end = self.by_chain.len();
        while start < end {
            work.tick()?;
            let middle = start + (end - start) / 2;
            match self.by_chain[middle].0.cmp(&chain) {
                std::cmp::Ordering::Less => start = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
                std::cmp::Ordering::Equal => {
                    let key = self.by_chain[middle].1;
                    // Two charged partition searches retain the exact bucket.
                    let boundary = |upper: bool, work: &mut Work<'_>| -> Result<usize, Stop> {
                        let mut start = 0;
                        let mut end = self.entries.len();
                        while start < end {
                            work.tick()?;
                            let middle = start + (end - start) / 2;
                            if self.entries[middle].0 < key
                                || (upper && self.entries[middle].0 == key)
                            {
                                start = middle + 1;
                            } else {
                                end = middle;
                            }
                        }
                        Ok(start)
                    };
                    return Ok(&self.entries[boundary(false, work)?..boundary(true, work)?]);
                }
            }
        }
        Ok(&[])
    }
}

impl<C: Count> Closure<'_, C> {
    /// Original formulas with equal associative witness leaves also have equal
    /// frozen values when both own-node gates are active. Authentication uses
    /// original structure; it does not apply original numeric bounds to a reduct.
    pub(super) fn equal_extremum_witnesses(
        &mut self,
        index: &Narrower,
        chain: usize,
        frozen: Option<&[bool]>,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let source = &index.chains[chain];
        if !source.disjunction || frozen.is_some_and(|truth| !truth[source.root]) {
            return Ok(Step::Unchanged);
        }
        let value = bit(self.known.sure, source.root);
        let mut step = Step::Unchanged;
        for &(_, target) in index.equal_extrema.candidates(chain, work)? {
            work.tick()?;
            let root = index.chains[target].root;
            if target == chain || frozen.is_some_and(|truth| !truth[root]) {
                continue;
            }
            if self
                .lists
                .witnesses
                .equal_leaves(index, chain, target, work)?
            {
                step = step.join(if value {
                    self.sure(root)
                } else {
                    self.never(root)
                });
                if step == Step::Contradiction {
                    break;
                }
            }
        }
        Ok(step)
    }
}
