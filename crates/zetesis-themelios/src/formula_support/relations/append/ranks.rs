//! Order-preserving integer ranks for a round's discoveries (design A).
//!
//! Every distinct canonical term and predicate of the round is compared once,
//! in typed order; each atom then sorts by its predicate rank and column ranks,
//! so the population's comparisons read integers only. Ranks depend only on
//! the round's atoms, never on the thread count.

use super::{Counters, FormulaFailure, FormulaLimits, Memory, ProgramSite, TermRef};
use crate::formula_support::GroundingWork;
use rayon::ThreadPool;
use zetesis_core::catalog::PredicateRef;

/// A term's or predicate's canonical position with its first occurrence.
type Entry = (usize, u32);

/// Column ranks parallel to the terms, and predicate ranks parallel to the
/// predicates.
pub(super) type Ranked = (Vec<u32>, Vec<u32>);

/// Column ranks parallel to `terms`, and each predicate's rank parallel to
/// `predicates`. `None` when a value is not canonical in one vocabulary; the
/// caller then compares the values themselves.
pub(super) fn ranks(
    terms: &[TermRef<'_>],
    predicates: &[PredicateRef<'_>],
    memory: &mut Memory<'_>,
    pool: Option<&ThreadPool>,
    (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
) -> Result<Option<Ranked>, FormulaFailure> {
    if u32::try_from(terms.len().max(predicates.len())).is_err() {
        return Ok(None);
    }
    let Some(columns) = rank(
        terms,
        |term| {
            let first = terms[0];
            (term.same_vocabulary(first) == Some(true))
                .then(|| term.canonical_position())
                .flatten()
        },
        |left, right, charged| {
            left.compare_ref_with(*right, || {
                *charged += 1;
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap_or_else(|never| match never {})
        },
        memory,
        pool,
        (limits, counters, location),
    )?
    else {
        return Ok(None);
    };
    let Some(heads) = rank(
        predicates,
        |predicate| {
            let first = predicates[0];
            (predicate.same_canonical(first).is_some())
                .then(|| predicate.canonical_position())
                .flatten()
        },
        |left, right, charged| {
            left.compare_ref_with(*right, || {
                *charged += 1;
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap_or_else(|never| match never {})
        },
        memory,
        pool,
        (limits, counters, location),
    )?
    else {
        return Ok(None);
    };
    Ok(Some((columns, heads)))
}

/// Dense ranks of `values` in the order `compare` defines, by canonical
/// position: equal positions share a rank and distinct values are compared
/// once per merge step, never per occurrence.
fn rank<T: Copy + Send + Sync>(
    values: &[T],
    position: impl Fn(&T) -> Option<usize>,
    compare: impl Fn(&T, &T, &mut u64) -> std::cmp::Ordering + Sync,
    memory: &mut Memory<'_>,
    pool: Option<&ThreadPool>,
    (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
) -> Result<Option<Vec<u32>>, FormulaFailure> {
    let mut ranks = Vec::new();
    memory.reserve(&mut ranks, values.len())?;
    if values.is_empty() {
        return Ok(Some(ranks));
    }
    let mut entries: Vec<Entry> = Vec::new();
    memory.reserve(&mut entries, values.len())?;
    for (index, value) in values.iter().enumerate() {
        counters.work(limits, location)?;
        let Some(position) = position(value) else {
            return Ok(None);
        };
        entries.push((position, u32::try_from(index).expect("checked count")));
    }
    // An integer sort, charged one permit per comparison it makes.
    counters.charge_work(u128::from(counted_sort(&mut entries)), limits, location)?;
    // One representative per distinct position, in typed order.
    let mut distinct: Vec<u32> = Vec::new();
    memory.reserve(&mut distinct, entries.len())?;
    for (index, &(position, first)) in entries.iter().enumerate() {
        if index == 0 || entries[index - 1].0 != position {
            distinct.push(first);
        }
    }
    let mut scratch = Vec::new();
    let mut runs = Vec::new();
    memory.reserve(&mut scratch, distinct.len())?;
    memory.reserve(&mut runs, distinct.len() + 1)?;
    crate::formula_support::sort::merge_by(
        &mut distinct,
        &mut scratch,
        &mut runs,
        pool,
        GroundingWork::new(limits, counters, location),
        |left, right, charged| compare(&values[*left as usize], &values[*right as usize], charged),
    )?;
    // Rank by position, then hand each occurrence its position's rank.
    let mut table: Vec<Entry> = Vec::new();
    memory.reserve(&mut table, distinct.len())?;
    for (rank, &first) in distinct.iter().enumerate() {
        let position = position(&values[first as usize]).expect("admitted above");
        table.push((position, u32::try_from(rank).expect("checked count")));
    }
    counters.charge_work(u128::from(counted_sort(&mut table)), limits, location)?;
    ranks.resize(values.len(), 0);
    counters.charge_work(entries.len() as u128, limits, location)?;
    let mut cursor = 0;
    for &(position, index) in &entries {
        while table[cursor].0 != position {
            cursor += 1;
        }
        ranks[index as usize] = table[cursor].1;
    }
    Ok(Some(ranks))
}

/// Sort integer entries in place, returning the comparisons made; the
/// count depends only on the entries.
fn counted_sort(entries: &mut [Entry]) -> u64 {
    let mut compared = 0;
    entries.sort_unstable_by(|left, right| {
        compared += 1;
        left.cmp(right)
    });
    compared
}
