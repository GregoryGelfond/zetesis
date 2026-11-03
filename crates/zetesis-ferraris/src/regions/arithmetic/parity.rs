//! Root echelon preparation over independently checked owner-coordinate rows.
//!
//! This is forward elimination, not reduced row echelon form. Source fields
//! describe each row's initial check; trusted addition establishes subsequent
//! consequences of those checked premises. No region or owner is changed.

mod runtime;
pub use runtime::ParityKnowledge;

use zetesis_algebra::Quota;

use std::convert::Infallible;
use std::mem::size_of;

use zetesis_algebra::words;
use zetesis_algebra::{Stop, words::Words};

use super::{
    budget::{Budget, BudgetError},
    rows::Equation,
    storage::Storage,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Unit {
    pub atom: usize,
    pub value: bool,
}

#[derive(Debug, Default)]
pub(super) struct Counts {
    pub additions: u64,
    pub rank: u64,
    pub units: u64,
    pub incidences: u64,
}

#[derive(Debug)]
pub(super) struct Prepared {
    pub basis: Vec<usize>,
    pub units: Vec<Unit>,
    pub index: Option<Index>,
    pub root_refuted: bool,
    pub counts: Counts,
}

impl Prepared {
    /// The one retained record and actual payload capacities, with embedded
    /// vector/index headers already included in the record layout.
    pub(super) fn retained_bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + self.basis.capacity() as u128 * size_of::<usize>() as u128
            + self.units.capacity() as u128 * size_of::<Unit>() as u128
            + self.index.as_ref().map_or(0, |index| {
                index.words.capacity() as u128 * size_of::<u64>() as u128
            })
    }
}

/// Atom-major incidence bits; row identifiers are the input enumeration order.
#[derive(Debug)]
pub(super) struct Index {
    pub words: Words<u64>,
    pub row_words: usize,
    pub rows: usize,
    /// Executed insertions, including repeated insertion of the same bit.
    pub insertions: u64,
}

impl Index {
    /// Insert an admitted atom/row incidence: locate costs three, OR and its
    /// observation counter cost two. Callers establish both coordinate bounds.
    pub(super) fn insert<Q: Quota + ?Sized>(
        &mut self,
        atom: usize,
        row: usize,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<(), BudgetError<Q::Error>> {
        assert!(row < self.rows, "index row is admitted");
        budget.charge(3)?;
        let (word, bit) = Words::<u64>::locate(row);
        self.insert_bit(atom, word, bit, budget)
    }

    fn insert_bit<Q: Quota + ?Sized>(
        &mut self,
        atom: usize,
        word: usize,
        bit: u64,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<(), BudgetError<Q::Error>> {
        budget.charge(2)?;
        self.words[atom * self.row_words + word] |= bit;
        self.insertions += 1;
        Ok(())
    }
}

/// Eliminate checked rows in place, preserving their original atom coordinates.
///
/// All rows have `W = 2*ceil(atom_count/128)` words, with zero universe padding,
/// and belong to the caller's one checked owner. A directory entry is a row
/// identifier plus one; zero denotes absence. Each addition clears the least
/// atom, so it strictly increases the next pivot and there are at most
/// `r*min(r,n)` additions. Only `add` changes coefficients or constants.
///
/// A conclusive zero-row test publishes refutation in that same admitted chunk.
/// It returns before counting units or building an index. Units otherwise stay
/// private in this result until the entire caller preparation succeeds. On any
/// error the caller must discard this preparation; no result prefix is returned.
/// Full-chunk additions still preserve the checked equations on a refused run.
///
/// The input rows are borrowed, so there is no reference-algorithm row-copy
/// term. Setup costs `15+2n`: three vector headers, directory cells plus each
/// pushed length, four counters and two result flags. With `A` additions and
/// rank `rho`, scans cost at most `(r+A)*(W+4)`, additions and directory decoding
/// `A*(W+3)`, and retaining basis identifiers `6rho`. A complete nonrefuting
/// path adds `rho*(2W+3)`, at most `u*(W+23)` for units, the index bound below,
/// and three final result writes. The unit bound includes at most `u` vector
/// growths and fewer than `2u` moved two-field records, under Storage's doubling
/// schedule. Actual capacities, including simultaneous old/new buffers, enter
/// Storage; actual charges are made by that helper, never by this upper bound.
/// A refuting path omits those suffix terms. The shared first-bit primitive's
/// result tag, counter increments, vector lengths and headers explain the
/// differences from the scalar reference coefficients.
pub(super) fn prepare<Q: Quota + ?Sized>(
    rows: &mut [Equation],
    atom_count: usize,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Prepared, BudgetError<Q::Error>> {
    let mut pivots = storage.filled(atom_count, 0usize, 1, budget)?;
    let basis = storage.reserve(rows.len().min(atom_count), budget)?;
    let units = storage.reserve(0, budget)?;
    storage.admit_record(
        (size_of::<Prepared>() - size_of::<Vec<usize>>() - size_of::<Vec<Unit>>()) as u128,
        budget,
    )?;
    budget.charge(6)?;
    let mut prepared = Prepared {
        basis,
        units,
        index: None,
        root_refuted: false,
        counts: Counts::default(),
    };
    for target in 0..rows.len() {
        loop {
            let Some(atom) = words::first(&rows[target].words, &mut |work| budget.charge(work))?
            else {
                let refuted = budget.bounded(3, || {
                    if rows[target].constant {
                        prepared.root_refuted = true;
                        (true, 3) // constant test, refutation flag, terminal tag
                    } else {
                        (false, 1)
                    }
                })?;
                if refuted {
                    storage.release(pivots)?;
                    return Ok(prepared);
                }
                break;
            };
            budget.charge(1)?;
            let previous = pivots[atom];
            if previous == 0 {
                budget.charge(3)?;
                pivots[atom] = target + 1;
                prepared.counts.rank += 1;
                storage.push(&mut prepared.basis, target, 1, budget)?;
                break;
            }
            budget.charge(1)?;
            let source = previous - 1;
            add(rows, target, source, &mut prepared.counts, budget)?;
        }
    }
    storage.release(pivots)?;
    collect_units(rows, &mut prepared, storage, budget)?;
    let index = budget.in_phase(super::work::Phase::Index, |budget| {
        build_index(
            atom_count,
            prepared.basis.iter().map(|&row| &rows[row].words[..]),
            storage,
            budget,
        )
    })?;
    budget.charge(2)?;
    prepared.counts.incidences = index.insertions;
    prepared.index = Some(index);
    // The standalone index moved into the already-admitted Option slot. Its
    // payload remains live; its header and scalar record now overlap that slot.
    storage.release_record(size_of::<Index>() as u128)?;
    budget.charge(1)?; // terminal result tag
    Ok(prepared)
}

/// The sole mutation of a checked row: a complete equation addition. Its
/// `W+2` chunk covers every XOR, the constant XOR and the observation counter.
/// Equal padded lengths and an earlier source are established by preparation;
/// the shared kernel's callbacks cannot refuse inside this admitted chunk.
fn add<Q: Quota + ?Sized>(
    rows: &mut [Equation],
    target: usize,
    source: usize,
    counts: &mut Counts,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    assert!(source < target, "a pivot belongs to an earlier checked row");
    let (earlier, remaining) = rows.split_at_mut(target);
    let source = &earlier[source];
    let target = &mut remaining[0];
    let width = u64::try_from(target.words.len()).expect("a row length fits a work word");
    budget.charge(width + 2)?;
    words::xor_assign(&mut target.words, &source.words, &mut || {
        Ok::<_, Infallible>(())
    })
    .unwrap_or_else(|never| match never {});
    target.constant ^= source.constant;
    counts.additions += 1;
    Ok(())
}

fn collect_units<Q: Quota + ?Sized>(
    rows: &[Equation],
    prepared: &mut Prepared,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    for row in &prepared.basis {
        budget.charge(1)?;
        let row = &rows[*row];
        let count = words::count(&row.words, &mut |work| budget.charge(work))?;
        budget.charge(1)?;
        if count == 1 {
            let atom = words::first(&row.words, &mut |work| budget.charge(work))?
                .expect("the unchanged row has one atom");
            storage.push(
                &mut prepared.units,
                Unit {
                    atom,
                    value: row.constant,
                },
                2,
                budget,
            )?;
            budget.charge(1)?;
            prepared.counts.units += 1;
        }
    }
    Ok(())
}

/// Build the same padded incidence representation for parity rows or family
/// occurrences. Supports use the exact owner's `W` words with cleared padding.
/// Their iterator supplies borrowed views in constant time; it performs no
/// source rereads or uncharged semantic transformation.
///
/// For `t` supports and `I` set bits, the complete bound is
/// `2*n*B(t) + 6 + 3*t + t*W + 7*I`: initialized payload plus vector lengths,
/// header/metadata, one row location, shared set-bit enumeration, and OR plus
/// an insertion counter per incidence. Guard insertions later cost five each.
/// Matrix/byte dimension checks precede allocation; unrepresentable allocation
/// dimensions remain an allocation failure, never a logical contradiction.
pub(super) fn build_index<'a, Q: Quota + ?Sized>(
    atom_count: usize,
    supports: impl ExactSizeIterator<Item = &'a [u64]>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Index, BudgetError<Q::Error>> {
    budget.charge(0)?; // global control precedes allocation-dimension refusal
    let rows = supports.len();
    let row_words = rows.div_ceil(128) * 2;
    let length = atom_count
        .checked_mul(row_words)
        .ok_or(BudgetError::Stopped(Stop::Allocation))?;
    let words = Words::new(storage.filled(length, 0, 1, budget)?);
    storage.admit_record((size_of::<Index>() - size_of::<Vec<u64>>()) as u128, budget)?;
    budget.charge(3)?;
    let mut index = Index {
        words,
        row_words,
        rows,
        insertions: 0,
    };
    for (row, support) in supports.enumerate() {
        budget.charge(3)?;
        let (word, bit) = Words::<u64>::locate(row);
        words::visit_set_bits(
            support,
            budget,
            &mut |budget, work| budget.charge(work),
            &mut |budget, atom| index.insert_bit(atom, word, bit, budget),
        )?;
    }
    Ok(index)
}
