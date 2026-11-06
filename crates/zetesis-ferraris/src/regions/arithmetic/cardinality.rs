//! Bounded proposals for disjoint lower/upper partitions of one atom carrier.
//!
//! Ordering discovers candidates; only the independent partition and effective
//! bound checks justify them. Original guards and source occurrences remain in
//! the checked input inventory. No guarded bound is assumed active at the root.

use zetesis_algebra::Quota;

use std::cmp::Ordering;
use std::mem::size_of;
use std::ops::Range;

use zetesis_algebra::Stop;
use zetesis_algebra::words::{self, Words};

use crate::AdmissionError;

use super::{
    budget::{Budget, BudgetError},
    cardinality_source::{Bound, Kind},
    parity::{self, Index},
    source::Literal,
    storage::Storage,
    uniform::BoundSide,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Counts {
    pub carriers: u64,
    pub attempts: u64,
    pub accepted: u64,
    pub rejected: u64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Occurrence {
    pub row: usize,
    pub pair: usize,
    pub side: BoundSide,
    pub value: u64,
}

#[derive(Debug)]
pub(super) struct Pair {
    pub lower: Range<usize>,
    pub upper: Range<usize>,
    pub lower_sum: u64,
    pub upper_sum: u64,
}

#[derive(Debug)]
pub(super) struct Families {
    pub pairs: Vec<Pair>,
    pub occurrences: Vec<Occurrence>,
    pub index: Option<Index>,
    pub counts: Counts,
}

#[derive(Debug)]
pub(super) enum Prepared {
    RootRefuted { counts: Counts },
    Families(Families),
}

impl Prepared {
    pub(super) fn counts(&self) -> &Counts {
        match self {
            Self::RootRefuted { counts } => counts,
            Self::Families(families) => &families.counts,
        }
    }

    /// Retained record plus actual payload capacities, excluding input bounds.
    pub(super) fn retained_bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + match self {
                Self::RootRefuted { .. } => 0,
                Self::Families(families) => {
                    families.pairs.capacity() as u128 * size_of::<Pair>() as u128
                        + families.occurrences.capacity() as u128 * size_of::<Occurrence>() as u128
                        + families.index.as_ref().map_or(0, |index| {
                            index.words.capacity() as u128 * size_of::<u64>() as u128
                        })
                }
            }
    }
}

/// Independent of discovery: authenticate directions and rebuild both unions.
/// Equality of union cardinality and the sum of row cardinalities establishes
/// disjointness, including duplicate row occurrences. XOR checks equal carriers.
fn check_pair<Q: Quota + ?Sized>(
    bounds: &[Bound],
    lower: &[usize],
    upper: &[usize],
    unions: (&mut [u64], &mut [u64]),
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    let (left, right) = unions;
    clear(left, budget)?;
    clear(right, budget)?;
    let Some(lower_cardinality) = union_side(bounds, lower, BoundSide::Lower, left, budget)? else {
        return Ok(false);
    };
    let Some(upper_cardinality) = union_side(bounds, upper, BoundSide::Upper, right, budget)?
    else {
        return Ok(false);
    };
    let cardinality = u64::try_from(words::count(left, &mut |work| budget.charge(work))?)
        .map_err(|_| BudgetError::Stopped(Stop::WorkLimit))?;
    budget.charge(1)?;
    if lower_cardinality != cardinality {
        return Ok(false);
    }
    budget.charge(1)?;
    if upper_cardinality != cardinality {
        return Ok(false);
    }
    words::xor_assign(left, right, &mut || budget.charge(1))?;
    let remaining = words::first(left, &mut |work| budget.charge(work))?;
    budget.charge(1)?;
    if remaining.is_some() {
        return Ok(false);
    }
    Ok(true)
}

fn union_side<Q: Quota + ?Sized>(
    bounds: &[Bound],
    rows: &[usize],
    side: BoundSide,
    union: &mut [u64],
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<u64>, BudgetError<Q::Error>> {
    budget.charge(1)?;
    let mut cardinality = 0_u64;
    for &row in rows {
        budget.charge(1)?;
        let bound = bounds
            .get(row)
            .ok_or(BudgetError::Source(AdmissionError::Edge))?;
        budget.charge(1)?;
        if bound.side != side {
            return Ok(None);
        }
        words::union_assign(union, &bound.support, &mut |work| budget.charge(work))?;
        budget.charge(2)?;
        let Some(next) = cardinality.checked_add(bound.cardinality) else {
            return Ok(None);
        };
        cardinality = next;
    }
    budget.charge(1)?;
    Ok(Some(cardinality))
}

fn effective<Q: Quota + ?Sized>(
    bound: &Bound,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<u64, BudgetError<Q::Error>> {
    budget.charge(2)?; // original guard emptiness and authenticated side
    let fallback = match bound.side {
        BoundSide::Lower => 0,
        BoundSide::Upper => bound.cardinality,
    };
    if bound.guard.is_empty() {
        budget.charge(2)?; // bound comparison and selected value
        Ok(match bound.side {
            BoundSide::Lower => bound.value.max(fallback),
            BoundSide::Upper => bound.value.min(fallback),
        })
    } else {
        budget.charge(1)?;
        Ok(fallback)
    }
}

fn reduce_side<Q: Quota + ?Sized>(
    bounds: &[Bound],
    occurrences: &mut [Occurrence],
    pair: usize,
    side: BoundSide,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<u64>, BudgetError<Q::Error>> {
    budget.charge(1)?;
    let mut sum = 0_u64;
    for occurrence in occurrences {
        budget.charge(1)?;
        let bound = bounds
            .get(occurrence.row)
            .ok_or(BudgetError::Source(AdmissionError::Edge))?;
        budget.charge(1)?;
        if occurrence.pair != pair {
            return Err(BudgetError::Stopped(Stop::InvalidProgram));
        }
        budget.charge(1)?;
        if occurrence.side != side {
            return Err(BudgetError::Stopped(Stop::InvalidProgram));
        }
        let value = effective(bound, budget)?;
        budget.charge(2)?;
        occurrence.value = value;
        if occurrence.value > bound.cardinality {
            return Ok(None);
        }
        budget.charge(2)?;
        sum = sum
            .checked_add(occurrence.value)
            .ok_or(BudgetError::Stopped(Stop::InvalidProgram))?;
    }
    budget.charge(1)?;
    Ok(Some(sum))
}

/// C0 reads the same occurrence cache and pair ranges retained for later C1.
fn reduce_pair<Q: Quota + ?Sized>(
    families: &mut Families,
    bounds: &[Bound],
    pair: usize,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    budget.charge(4)?;
    let lower = families.pairs[pair].lower.clone();
    let upper = families.pairs[pair].upper.clone();
    let Some(lower) = reduce_side(
        bounds,
        &mut families.occurrences[lower],
        pair,
        BoundSide::Lower,
        budget,
    )?
    else {
        return Ok(true);
    };
    let Some(upper) = reduce_side(
        bounds,
        &mut families.occurrences[upper],
        pair,
        BoundSide::Upper,
        budget,
    )?
    else {
        return Ok(true);
    };
    budget.charge(4)?;
    families.pairs[pair].lower_sum = lower;
    families.pairs[pair].upper_sum = upper;
    Ok(families.pairs[pair].lower_sum > families.pairs[pair].upper_sum)
}

fn clear<Q: Quota + ?Sized>(
    words: &mut [u64],
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    Words::<u64, _>::new(words).fill_with(0, || budget.charge(1))
}

#[derive(Clone, Copy)]
struct GuardKey {
    start: usize,
    len: usize,
}

/// Flat guard-key spans and the signed literals they index.
struct GuardKeys {
    keys: Vec<GuardKey>,
    literals: Vec<Literal>,
}

#[derive(Clone)]
struct Proposal {
    lower: Range<usize>,
    upper: Range<usize>,
}

/// Selected row indices and partition proposals over ranges of those indices.
struct PartitionProposals {
    selected: Vec<usize>,
    proposals: Vec<Proposal>,
}

/// Stable bottom-up merging, reusing one initialized caller-owned buffer.
/// Each level moves each record once; an odd final level copies it back once.
pub(super) fn sort<'a, T: Copy, Q: Quota + ?Sized>(
    values: &'a mut [T],
    scratch: &'a mut [T],
    words: u64,
    mut compare: impl FnMut(&T, &T, &mut Budget<'_, '_, Q>) -> Result<Ordering, BudgetError<Q::Error>>,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    let len = values.len();
    if len < 2 {
        return Ok(());
    }
    let mut source = values;
    let mut target = &mut scratch[..len];
    budget.charge(1)?;
    let mut copied = false;
    let mut width = 1usize;
    while width < len {
        for first in (0..len).step_by(width.saturating_mul(2)) {
            let middle = first.saturating_add(width).min(len);
            let end = middle.saturating_add(width).min(len);
            let (mut left, mut right) = (first, middle);
            for slot in &mut target[first..end] {
                let take_left = if right == end {
                    true
                } else if left == middle {
                    false
                } else {
                    let order = compare(&source[left], &source[right], budget)?;
                    budget.charge(1)?;
                    order != Ordering::Greater
                };
                budget.charge(words)?;
                if take_left {
                    *slot = source[left];
                    left += 1;
                } else {
                    *slot = source[right];
                    right += 1;
                }
            }
        }
        budget.charge(5)?; // two borrowed slice handles and the parity flag
        std::mem::swap(&mut source, &mut target);
        copied = !copied;
        width = width.saturating_mul(2);
    }
    if copied {
        for (target, source) in target.iter_mut().zip(source.iter()) {
            budget.charge(words)?;
            *target = *source;
        }
    }
    Ok(())
}

fn scalar<T: Copy + Ord, Q: Quota + ?Sized>(
    left: T,
    right: T,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Ordering, BudgetError<Q::Error>> {
    budget.charge(1)?;
    if left < right {
        return Ok(Ordering::Less);
    }
    budget.charge(1)?;
    Ok(if left > right {
        Ordering::Greater
    } else {
        Ordering::Equal
    })
}

fn literal_order<Q: Quota + ?Sized>(
    left: &Literal,
    right: &Literal,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Ordering, BudgetError<Q::Error>> {
    let atom = scalar(left.atom, right.atom, budget)?;
    budget.charge(1)?;
    if atom == Ordering::Equal {
        scalar(left.positive, right.positive, budget)
    } else {
        Ok(atom)
    }
}

/// Sorting keys alone remove repeated signed occurrences. Source guards stay
/// untouched, including both polarities of an atom and all original repeats.
fn guard_keys<Q: Quota + ?Sized>(
    bounds: &[Bound],
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<GuardKeys, BudgetError<Q::Error>> {
    budget.charge(2)?;
    let (mut total, mut maximum) = (0usize, 0usize);
    for row in bounds {
        budget.charge(2)?;
        total = total
            .checked_add(row.guard.len())
            .ok_or(BudgetError::Stopped(Stop::Allocation))?;
        budget.charge(2)?;
        maximum = maximum.max(row.guard.len());
    }
    let mut keys = storage.reserve(bounds.len(), budget)?;
    let mut literals = storage.reserve(total, budget)?;
    let mut scratch = storage.filled(
        maximum,
        Literal {
            atom: 0,
            positive: false,
        },
        2,
        budget,
    )?;
    for row in bounds {
        budget.charge(1)?;
        let start = literals.len();
        for literal in &row.guard {
            budget.charge(3)?;
            literals.push(*literal);
        }
        sort(
            &mut literals[start..],
            &mut scratch,
            2,
            literal_order,
            budget,
        )?;
        budget.charge(1)?;
        let mut count = 0usize;
        for index in start..literals.len() {
            budget.charge(3)?; // copy the pair and test whether a predecessor exists
            let literal = literals[index];
            let duplicate = if count == 0 {
                false
            } else {
                budget.charge(2)?;
                let previous = literals[start + count - 1];
                budget.charge(1)?;
                if previous.atom == literal.atom {
                    budget.charge(1)?;
                    previous.positive == literal.positive
                } else {
                    false
                }
            };
            budget.charge(1)?;
            if !duplicate {
                budget.charge(3)?;
                literals[start + count] = literal;
                count += 1;
            }
        }
        budget.charge(3)?;
        keys.push(GuardKey { start, len: count });
    }
    storage.release(scratch)?;
    Ok(GuardKeys { keys, literals })
}

fn find<Q: Quota + ?Sized>(
    parents: &[usize],
    row: usize,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<usize, BudgetError<Q::Error>> {
    budget.charge(1)?;
    let mut current = row;
    loop {
        budget.charge(2)?;
        let next = parents[current];
        if next == current {
            return Ok(current);
        }
        budget.charge(1)?;
        current = next;
    }
}

fn unite<Q: Quota + ?Sized>(
    parents: &mut [usize],
    sizes: &mut [usize],
    left: usize,
    right: usize,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    let left = find(parents, left, budget)?;
    let right = find(parents, right, budget)?;
    budget.charge(1)?;
    if left == right {
        return Ok(());
    }
    budget.charge(3)?;
    let (winner, loser) = if sizes[left] < sizes[right] {
        (right, left)
    } else {
        (left, right)
    };
    budget.charge(2)?;
    parents[loser] = winner;
    sizes[winner] += sizes[loser]; // components partition at most bounds.len() rows
    Ok(())
}

fn components<Q: Quota + ?Sized>(
    bounds: &[Bound],
    atom_count: usize,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Vec<usize>, BudgetError<Q::Error>> {
    let mut first = storage.filled(atom_count, 0usize, 1, budget)?;
    let mut parents = storage.reserve(bounds.len(), budget)?;
    for row in 0..bounds.len() {
        budget.charge(2)?;
        parents.push(row);
    }
    let mut sizes = storage.filled(bounds.len(), 1usize, 1, budget)?;
    for (row, bound) in bounds.iter().enumerate() {
        budget.charge(2)?;
        let encoded = row
            .checked_add(1)
            .ok_or(BudgetError::Stopped(Stop::Allocation))?;
        words::visit_set_bits(
            &bound.support,
            budget,
            &mut |budget, work| budget.charge(work),
            &mut |budget, atom| {
                budget.charge(1)?;
                if atom >= atom_count {
                    return Err(BudgetError::Source(AdmissionError::Atom));
                }
                budget.charge(1)?;
                if first[atom] == 0 {
                    budget.charge(1)?;
                    first[atom] = encoded;
                } else {
                    budget.charge(1)?;
                    unite(&mut parents, &mut sizes, row, first[atom] - 1, budget)?;
                }
                Ok(())
            },
        )?;
    }
    storage.release(first)?;
    storage.release(sizes)?;
    let mut result = storage.reserve(bounds.len(), budget)?;
    for row in 0..bounds.len() {
        let component = find(&parents, row, budget)?;
        budget.charge(2)?;
        result.push(component);
    }
    storage.release(parents)?;
    Ok(result)
}

fn side(side: BoundSide) -> u8 {
    match side {
        BoundSide::Lower => 0,
        BoundSide::Upper => 1,
    }
}
fn kind(kind: Kind) -> u8 {
    match kind {
        Kind::UpperConstraint => 0,
        Kind::LowerConstraint => 1,
        Kind::Asserted => 2,
        Kind::ClauseLower => 3,
        Kind::Probed => 4,
    }
}

/// Equal-cardinality supports compare by their increasing atom sequences.
/// At the first differing word, the support containing its least differing bit
/// comes first. No secondary atom-sequence representation is constructed.
fn support_order<Q: Quota + ?Sized>(
    left: &[u64],
    right: &[u64],
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Ordering, BudgetError<Q::Error>> {
    for (&left, &right) in left.iter().zip(right) {
        budget.charge(1)?;
        if left != right {
            budget.charge(5)?;
            let first = 1u64 << Words::<u64>::first_bit(left ^ right);
            return Ok(if left & first != 0 {
                Ordering::Less
            } else {
                Ordering::Greater
            });
        }
    }
    Ok(Ordering::Equal)
}

struct Keys<'a> {
    bounds: &'a [Bound],
    guards: &'a [GuardKey],
    literals: &'a [Literal],
}

impl Keys<'_> {
    fn compare<Q: Quota + ?Sized>(
        &self,
        left: usize,
        right: usize,
        descending: bool,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<Ordering, BudgetError<Q::Error>> {
        let a = &self.bounds[left];
        let b = &self.bounds[right];
        let count = scalar(a.cardinality, b.cardinality, budget)?;
        budget.charge(1)?;
        if count != Ordering::Equal {
            return Ok(if descending { count.reverse() } else { count });
        }
        let support = support_order(&a.support, &b.support, budget)?;
        budget.charge(1)?;
        if support != Ordering::Equal {
            return Ok(support);
        }
        budget.charge(4)?;
        let a_key = self.guards[left];
        let b_key = self.guards[right];
        for (a, b) in self.literals[a_key.start..a_key.start + a_key.len]
            .iter()
            .zip(&self.literals[b_key.start..b_key.start + b_key.len])
        {
            let order = literal_order(a, b, budget)?;
            budget.charge(1)?;
            if order != Ordering::Equal {
                return Ok(order);
            }
        }
        let guard = scalar(a_key.len, b_key.len, budget)?;
        budget.charge(1)?;
        if guard != Ordering::Equal {
            return Ok(guard);
        }
        let value = scalar(a.value, b.value, budget)?;
        budget.charge(1)?;
        if value != Ordering::Equal {
            return Ok(value);
        }
        let kind = scalar(kind(a.source.kind), kind(b.source.kind), budget)?;
        budget.charge(1)?;
        if kind != Ordering::Equal {
            return Ok(kind);
        }
        let root = scalar(a.source.root, b.source.root, budget)?;
        budget.charge(1)?;
        if root != Ordering::Equal {
            return Ok(root);
        }
        scalar(a.source.node, b.source.node, budget)
    }
}

fn greedy<Q: Quota + ?Sized>(
    bounds: &[Bound],
    rows: &[usize],
    selected: &mut Vec<usize>,
    union: &mut [u64],
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Range<usize>, BudgetError<Q::Error>> {
    clear(union, budget)?;
    budget.charge(1)?;
    let first = selected.len();
    for &row in rows {
        let disjoint =
            words::disjoint(union, &bounds[row].support, &mut |work| budget.charge(work))?;
        budget.charge(1)?;
        if disjoint {
            words::union_assign(union, &bounds[row].support, &mut |work| budget.charge(work))?;
            storage.push(selected, row, 1, budget)?;
        }
    }
    budget.charge(2)?;
    Ok(first..selected.len())
}

/// Build the four lists in lower-ascending, lower-descending, upper-ascending,
/// upper-descending order, then publish their prescribed Cartesian attempts.
/// The caller already admitted the four-range workspace; all borrowed order
/// slices refer to the same carrier and this helper retains no new owner.
fn propose_carrier<Q: Quota + ?Sized>(
    bounds: &[Bound],
    orders: [&[usize]; 4],
    selected: &mut Vec<usize>,
    proposals: &mut Vec<Proposal>,
    union: &mut [u64],
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    let lists = [
        greedy(bounds, orders[0], selected, union, storage, budget)?,
        greedy(bounds, orders[1], selected, union, storage, budget)?,
        greedy(bounds, orders[2], selected, union, storage, budget)?,
        greedy(bounds, orders[3], selected, union, storage, budget)?,
    ];
    for (lower, upper) in [(0, 2), (0, 3), (1, 2), (1, 3)] {
        storage.push(
            proposals,
            Proposal {
                lower: lists[lower].clone(),
                upper: lists[upper].clone(),
            },
            4,
            budget,
        )?;
    }
    Ok(())
}

/// Allocate the source-order row indices before carrier and bound ordering.
fn row_indices<Q: Quota + ?Sized>(
    count: usize,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Vec<usize>, BudgetError<Q::Error>> {
    let mut rows = storage.reserve(count, budget)?;
    for row in 0..count {
        budget.charge(2)?;
        rows.push(row);
    }
    Ok(rows)
}

/// Complete the four deterministic proposals for every support carrier before
/// checking them. Neither a guarded atom nor greedy success changes a carrier.
fn discover<Q: Quota + ?Sized>(
    bounds: &[Bound],
    atom_count: usize,
    counts: &mut Counts,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<PartitionProposals, BudgetError<Q::Error>> {
    let components = components(bounds, atom_count, storage, budget)?;
    let guards = guard_keys(bounds, storage, budget)?;
    let mut members = row_indices(bounds.len(), storage, budget)?;
    let mut scratch = storage.filled(bounds.len(), 0usize, 1, budget)?;
    sort(
        &mut members,
        &mut scratch,
        1,
        |&a, &b, budget| {
            let order = scalar(components[a], components[b], budget)?;
            budget.charge(1)?;
            if order == Ordering::Equal {
                scalar(side(bounds[a].side), side(bounds[b].side), budget)
            } else {
                Ok(order)
            }
        },
        budget,
    )?;
    let mut descending = storage.reserve(bounds.len(), budget)?;
    for &row in &members {
        budget.charge(2)?;
        descending.push(row);
    }
    let mut selected = storage.reserve(0, budget)?;
    let mut proposals = storage.reserve(0, budget)?;
    let mut union = Words::new(storage.filled(atom_count.div_ceil(128) * 2, 0u64, 1, budget)?);
    storage.admit_record(size_of::<[Range<usize>; 4]>() as u128, budget)?;
    let keys = Keys {
        bounds,
        guards: &guards.keys,
        literals: &guards.literals,
    };
    let mut first = 0;
    while first < members.len() {
        budget.charge(2)?;
        counts.carriers += 1;
        let component = components[members[first]];
        let mut end = first;
        while end < members.len() {
            budget.charge(1)?;
            if components[members[end]] != component {
                break;
            }
            end += 1;
        }
        let mut middle = first;
        while middle < end {
            budget.charge(1)?;
            if bounds[members[middle]].side != BoundSide::Lower {
                break;
            }
            middle += 1;
        }
        for range in [first..middle, middle..end] {
            sort(
                &mut members[range.clone()],
                &mut scratch,
                1,
                |&a, &b, budget| keys.compare(a, b, false, budget),
                budget,
            )?;
            sort(
                &mut descending[range],
                &mut scratch,
                1,
                |&a, &b, budget| keys.compare(a, b, true, budget),
                budget,
            )?;
        }
        propose_carrier(
            bounds,
            [
                &members[first..middle],
                &descending[first..middle],
                &members[middle..end],
                &descending[middle..end],
            ],
            &mut selected,
            &mut proposals,
            &mut union,
            storage,
            budget,
        )?;
        first = end;
    }
    storage.release_record(size_of::<[Range<usize>; 4]>() as u128)?;
    storage.release(union.into_storage())?;
    storage.release(descending)?;
    storage.release(members)?;
    storage.release(scratch)?;
    storage.release(components)?;
    storage.release(guards.keys)?;
    storage.release(guards.literals)?;
    Ok(PartitionProposals {
        selected,
        proposals,
    })
}

fn retain<Q: Quota + ?Sized>(
    families: &mut Families,
    lower: &[usize],
    upper: &[usize],
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<usize, BudgetError<Q::Error>> {
    budget.charge(2)?;
    let pair = families.pairs.len();
    let first = families.occurrences.len();
    for &row in lower {
        storage.push(
            &mut families.occurrences,
            Occurrence {
                row,
                pair,
                side: BoundSide::Lower,
                value: 0,
            },
            4,
            budget,
        )?;
    }
    budget.charge(1)?;
    let middle = families.occurrences.len();
    for &row in upper {
        storage.push(
            &mut families.occurrences,
            Occurrence {
                row,
                pair,
                side: BoundSide::Upper,
                value: 0,
            },
            4,
            budget,
        )?;
    }
    budget.charge(1)?;
    let end = families.occurrences.len();
    storage.push(
        &mut families.pairs,
        Pair {
            lower: first..middle,
            upper: middle..end,
            lower_sum: 0,
            upper_sum: 0,
        },
        6,
        budget,
    )?;
    Ok(pair)
}

/// Prepare the approved two-order greedy policy, then independently check each
/// of its four pair attempts per carrier. This is deliberately incomplete for
/// general partition discovery; rejected proposals have no semantic effect.
///
/// Union by size without compression bounds each find by log2(row count).
/// Sorting uses stable merges and exact canonical ties. Temporary guard keys
/// are flat offsets over copied literals. Every copy, numeric reduction and
/// allocated capacity is charged; no unmetered library sort or search runs.
/// Shared disjointness stops at its first overlap, so rejected scans can cost
/// less than the reference's complete-word overlap reduction. Effective values
/// and pair sums are computed directly in the retained occurrence records.
///
/// Input rows are independently source-checked, with support cardinalities and
/// zero padding for this atom universe. Any error discards the whole result.
/// The first checked root contradiction returns before later checks or index
/// construction. All remaining family occurrences get the shared incidence
/// index, including every original guard occurrence and each repeated pair use.
pub(super) fn prepare<Q: Quota + ?Sized>(
    bounds: &[Bound],
    atom_count: usize,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Prepared, BudgetError<Q::Error>> {
    let width = atom_count.div_ceil(128) * 2;
    for bound in bounds {
        budget.charge(1)?;
        if bound.support.len() != width {
            return Err(BudgetError::Source(AdmissionError::Span));
        }
    }
    storage.admit_record(size_of::<Prepared>() as u128, budget)?;
    let pairs = storage.reserve(0, budget)?;
    let occurrences = storage.reserve(0, budget)?;
    storage.embed::<Pair>()?;
    storage.embed::<Occurrence>()?;
    budget.charge(5)?; // index absence and four observation counters
    let mut families = Families {
        pairs,
        occurrences,
        index: None,
        counts: Counts::default(),
    };
    budget.charge(1)?;
    if bounds.is_empty() {
        return Ok(Prepared::Families(families));
    }
    let PartitionProposals {
        selected,
        proposals,
    } = budget.in_phase(super::work::Phase::Discovery, |budget| {
        discover(bounds, atom_count, &mut families.counts, storage, budget)
    })?;
    let mut lower_union = Words::new(storage.filled(width, 0u64, 1, budget)?);
    let mut upper_union = Words::new(storage.filled(width, 0u64, 1, budget)?);
    for proposal in &proposals {
        budget.charge(1)?;
        families.counts.attempts += 1;
        let lower = &selected[proposal.lower.clone()];
        let upper = &selected[proposal.upper.clone()];
        let valid = budget.in_phase(super::work::Phase::CandidateChecks, |budget| {
            check_pair(
                bounds,
                lower,
                upper,
                (&mut lower_union, &mut upper_union),
                budget,
            )
        })?;
        budget.charge(1)?;
        if !valid {
            budget.charge(1)?;
            families.counts.rejected += 1;
            continue;
        }
        budget.charge(1)?;
        families.counts.accepted += 1;
        let pair = retain(&mut families, lower, upper, storage, budget)?;
        let refuted = reduce_pair(&mut families, bounds, pair, budget)?;
        budget.charge(1)?;
        if refuted {
            budget.charge(1)?;
            storage.release(lower_union.into_storage())?;
            storage.release(upper_union.into_storage())?;
            storage.release(selected)?;
            storage.release(proposals)?;
            storage.release_embedded(families.pairs)?;
            storage.release_embedded(families.occurrences)?;
            return Ok(Prepared::RootRefuted {
                counts: families.counts,
            });
        }
    }
    storage.release(lower_union.into_storage())?;
    storage.release(upper_union.into_storage())?;
    storage.release(selected)?;
    storage.release(proposals)?;
    let index = budget.in_phase(super::work::Phase::Index, |budget| {
        let mut index = parity::build_index(
            atom_count,
            families
                .occurrences
                .iter()
                .map(|occurrence| &bounds[occurrence.row].support[..]),
            storage,
            budget,
        )?;
        for (position, occurrence) in families.occurrences.iter().enumerate() {
            for literal in &bounds[occurrence.row].guard {
                budget.charge(1)?;
                if literal.atom >= atom_count {
                    return Err(BudgetError::Source(AdmissionError::Atom));
                }
                index.insert(literal.atom, position, budget)?;
            }
        }
        Ok(index)
    })?;
    storage.release_record(size_of::<Index>() as u128)?;
    budget.charge(2)?;
    families.index = Some(index);
    Ok(Prepared::Families(families))
}

#[cfg(test)]
mod tests;
