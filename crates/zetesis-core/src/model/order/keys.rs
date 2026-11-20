//! Semantic order from term ranks.
//!
//! An atom's order is its predicate's order, then its arguments' orders from
//! the first column on. Each distinct term and predicate the occurrences use
//! is therefore ranked once by content, and the occurrences are then ordered
//! by those integer ranks alone, with a stable least-significant-digit radix
//! sort over one columnar key lane: each argument column from the last, then
//! the predicate. Equal ranks mean equal content because one canonical scope
//! interns each typed term once, so no atom payload is compared again.
//!
//! A checked merge sort over the same integer keys, read from the rows at
//! each comparison, was measured beside this one: it holds fewer cells, but
//! the radix passes ordered the order-bound cells 2.3 to 5.8 times faster,
//! within the selection peak.

use std::cmp::Ordering;

use super::{Account, ModelFailure, bytes, cells, reserve};
use crate::catalog::storage::{Atom, AtomId, PredicateId, Read, TermId};
use crate::catalog::{AtomCatalog, PredicateRef, TermRef};

const UNUSED: u32 = u32::MAX;
const USED: u32 = 0;

/// Positions in semantic order, and each position's equivalence rank.
type Order = (Vec<usize>, Vec<usize>);

/// The occurrences in semantic order, and each occurrence's equivalence rank.
/// Equal atoms keep ascending positions.
pub(super) fn semantic_order<E>(
    catalog: &AtomCatalog,
    base: u128,
    account: &mut Account,
    checked: &mut impl FnMut() -> Result<(), ModelFailure<E>>,
) -> Result<Order, ModelFailure<E>> {
    let (read, ids) = catalog.atoms().rows();
    let count = ids.len();
    let mut live = base;
    let mut semantic = Vec::new();
    let mut spare = Vec::new();
    if count < 2 {
        fill(&mut semantic, count, 0, &mut live, account, checked)?;
        fill(&mut spare, count, 0, &mut live, account, checked)?;
        return Ok((semantic, spare));
    }
    let used = Used::rank(read, ids, &mut live, account, checked)?;
    fill(&mut semantic, count, 0, &mut live, account, checked)?;
    fill(&mut spare, count, 0, &mut live, account, checked)?;
    for (position, slot) in semantic.iter_mut().enumerate() {
        *slot = position;
    }
    used.radix(&mut semantic, &mut spare, live, account, checked)?;
    drop(used);
    equivalence(ids, &semantic, &mut spare, checked)?;
    Ok((semantic, spare))
}

fn row(read: Read<'_>, id: AtomId) -> Atom<'_> {
    read.atom(id).expect("catalog occurrence is admitted")
}

fn argument(atom: Atom<'_>, column: usize) -> usize {
    atom.argument(column)
        .expect("column is within the row's arity")
        .position()
}

/// Rank tables indexed by term and predicate identity, over the identities
/// the occurrences use, beside the occurrences they rank.
struct Used<'a> {
    read: Read<'a>,
    ids: &'a [AtomId],
    term_rank: Vec<u32>,
    predicate_rank: Vec<u32>,
    terms: usize,
    predicates: usize,
    widest: usize,
}

impl<'a> Used<'a> {
    fn rank<E>(
        read: Read<'a>,
        ids: &'a [AtomId],
        live: &mut u128,
        account: &mut Account,
        checked: &mut impl FnMut() -> Result<(), ModelFailure<E>>,
    ) -> Result<Self, ModelFailure<E>> {
        let mut used = Self {
            read,
            ids,
            term_rank: Vec::new(),
            predicate_rank: Vec::new(),
            terms: 0,
            predicates: 0,
            widest: 0,
        };
        fill(
            &mut used.term_rank,
            read.term_count(),
            UNUSED,
            live,
            account,
            checked,
        )?;
        let predicates = read.predicate_count();
        fill(
            &mut used.predicate_rank,
            predicates,
            UNUSED,
            live,
            account,
            checked,
        )?;
        for &id in ids {
            checked()?;
            let atom = row(read, id);
            used.predicates += mark(&mut used.predicate_rank[atom.predicate().position()]);
            used.widest = used.widest.max(atom.arity());
            for column in 0..atom.arity() {
                checked()?;
                used.terms += mark(&mut used.term_rank[argument(atom, column)]);
            }
        }
        rank_used(
            &mut used.term_rank,
            used.terms,
            *live,
            account,
            checked,
            |left, right, permit| {
                let term = |position| {
                    TermRef::new(read, TermId::from_position(position)).expect("used term resolves")
                };
                term(left).compare_ref_with(term(right), permit)
            },
        )?;
        let (table, count) = (&mut used.predicate_rank, used.predicates);
        rank_used(
            table,
            count,
            *live,
            account,
            checked,
            |left, right, permit| {
                let predicate = |position| {
                    PredicateRef::new(read, PredicateId::from_position(position))
                        .expect("used predicate resolves")
                };
                predicate(left).compare_ref_with(predicate(right), permit)
            },
        )?;
        Ok(used)
    }

    /// Order `semantic` by a stable least-significant-digit radix sort over
    /// one key lane: each argument column from the last, then the predicate.
    /// A column beyond an atom's arity reads zero; atoms of one predicate
    /// share their arity, and the predicate is the most significant key.
    fn radix<E>(
        &self,
        semantic: &mut Vec<usize>,
        spare: &mut Vec<usize>,
        mut live: u128,
        account: &mut Account,
        checked: &mut impl FnMut() -> Result<(), ModelFailure<E>>,
    ) -> Result<(), ModelFailure<E>> {
        let (read, ids) = (self.read, self.ids);
        let count = ids.len();
        let mut lane: Vec<u32> = Vec::new();
        fill(&mut lane, count, 0, &mut live, account, checked)?;
        let bits = digit_bits(count);
        let mut digits: Vec<u32> = Vec::new();
        fill(&mut digits, 1 << bits, 0, &mut live, account, checked)?;
        for column in (0..self.widest).rev() {
            for (key, &id) in lane.iter_mut().zip(ids) {
                checked()?;
                let atom = row(read, id);
                *key = if column < atom.arity() {
                    self.term_rank[argument(atom, column)]
                } else {
                    0
                };
            }
            sort_lane(
                &lane,
                self.terms,
                bits,
                semantic,
                spare,
                &mut digits,
                checked,
            )?;
        }
        for (key, &id) in lane.iter_mut().zip(ids) {
            checked()?;
            *key = self.predicate_rank[row(read, id).predicate().position()];
        }
        sort_lane(
            &lane,
            self.predicates,
            bits,
            semantic,
            spare,
            &mut digits,
            checked,
        )
    }
}

/// Digit width for a lane of `count` keys: up to a byte, narrower for a
/// small lane so its digit table stays below its cells.
fn digit_bits(count: usize) -> u32 {
    ((usize::BITS - count.leading_zeros()) / 2).clamp(1, 8)
}

/// Stable passes of `order`, one per digit of keys below `values`, keyed by
/// `lane` at each position.
fn sort_lane<E>(
    lane: &[u32],
    values: usize,
    bits: u32,
    order: &mut Vec<usize>,
    spare: &mut Vec<usize>,
    digits: &mut [u32],
    checked: &mut impl FnMut() -> Result<(), ModelFailure<E>>,
) -> Result<(), ModelFailure<E>> {
    let width = usize::BITS - values.saturating_sub(1).leading_zeros();
    let mask = (1_u32 << bits) - 1;
    for pass in 0..width.div_ceil(bits) {
        let shift = pass * bits;
        let digit = |position: usize| ((lane[position] >> shift) & mask) as usize;
        digits.fill(0);
        for &position in order.iter() {
            checked()?;
            digits[digit(position)] += 1;
        }
        let mut total = 0;
        for slot in digits.iter_mut() {
            total += std::mem::replace(slot, total);
        }
        for &position in order.iter() {
            checked()?;
            let slot = &mut digits[digit(position)];
            spare[*slot as usize] = position;
            *slot += 1;
        }
        std::mem::swap(order, spare);
    }
    Ok(())
}

/// Mark an unused identity used, counting it once.
fn mark(cell: &mut u32) -> usize {
    if *cell == UNUSED {
        *cell = USED;
        1
    } else {
        0
    }
}

/// Write each position's equivalence rank along `semantic` into `ranks`:
/// equal atoms share one identity and sit together in it.
fn equivalence<E>(
    ids: &[AtomId],
    semantic: &[usize],
    ranks: &mut [usize],
    checked: &mut impl FnMut() -> Result<(), ModelFailure<E>>,
) -> Result<(), ModelFailure<E>> {
    let mut rank = 0;
    let mut previous = None;
    for &position in semantic {
        checked()?;
        let id = ids[position];
        if previous.is_some_and(|previous| previous != id) {
            rank += 1;
        }
        ranks[position] = rank;
        previous = Some(id);
    }
    Ok(())
}

/// Reserve `buffer` for `count` cells of `value`, adding its capacity to
/// `live`.
fn fill<T: Copy, E>(
    buffer: &mut Vec<T>,
    count: usize,
    value: T,
    live: &mut u128,
    account: &mut Account,
    checked: &mut impl FnMut() -> Result<(), ModelFailure<E>>,
) -> Result<(), ModelFailure<E>> {
    reserve(buffer, count, *live, account, checked)?;
    checked()?;
    buffer.resize(count, value);
    *live += bytes::<T>(buffer.capacity());
    account.record(*live);
    Ok(())
}

/// Replace each `USED` cell of `table` by the rank of its position among the
/// used positions, ordered by `compare`. Distinct identities of one scope
/// never compare equal.
fn rank_used<E>(
    table: &mut [u32],
    used: usize,
    live: u128,
    account: &mut Account,
    checked: &mut impl FnMut() -> Result<(), ModelFailure<E>>,
    mut compare: impl FnMut(
        usize,
        usize,
        &mut dyn FnMut() -> Result<(), ModelFailure<E>>,
    ) -> Result<Ordering, ModelFailure<E>>,
) -> Result<(), ModelFailure<E>> {
    let mut positions = Vec::new();
    let mut scratch = Vec::new();
    let mut held = live;
    reserve(&mut positions, used, held, account, checked)?;
    held += cells(positions.capacity());
    fill(&mut scratch, used, 0, &mut held, account, checked)?;
    positions.extend(
        table
            .iter()
            .enumerate()
            .filter(|&(_, &cell)| cell == USED)
            .map(|(position, _)| position),
    );
    crate::checked_sort::sort(
        &mut positions,
        &mut scratch,
        |values, left, right, permit| compare(values[left], values[right], permit),
        checked,
    )?;
    for (rank, &position) in positions.iter().enumerate() {
        checked()?;
        table[position] = u32::try_from(rank).expect("ranks fit u32");
    }
    Ok(())
}
