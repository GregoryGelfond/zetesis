//! Root echelon preparation over independently checked owner-coordinate rows.
//!
//! This is forward elimination, not reduced row echelon form. Source fields
//! describe each row's initial check; trusted addition establishes subsequent
//! consequences of those checked premises. No region or owner is changed.
//!
//! The original theory's one-atom equations arrive as units and are no row
//! ([`prepare_after_units`]). Each decides its atom, so it is added to every
//! row over that atom before elimination: afterwards no row names a decided
//! atom, and the units and the rows are independent parts of one system.

mod runtime;
pub use runtime::ParityKnowledge;

use zetesis_algebra::Quota;

use std::convert::Infallible;
use std::mem::size_of;

use zetesis_algebra::words;
use zetesis_algebra::{Stop, words::Words};

use super::{
    budget::{Budget, BudgetError},
    rows::{Equation, Unit},
    storage::Storage,
};

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
    /// The atoms the system decides: the direct units, one for each atom
    /// they name and in the order recognized, then the one-atom echelon rows
    /// in basis order. No two name one atom. Empty when the root is refuted.
    pub units: Vec<Unit>,
    pub index: Option<Index>,
    pub root_refuted: bool,
    pub counts: Counts,
}

impl Prepared {
    /// The units that were recognized as one-atom equations and are no row:
    /// those before the `counts.units` collected from the echelon rows.
    pub(super) fn direct(&self) -> &[Unit] {
        let collected = usize::try_from(self.counts.units).expect("units were pushed");
        &self.units[..self.units.len() - collected]
    }

    /// The rank of the checked system: its direct units and echelon pivots.
    /// A refuted root has no unit, so this is the pivots retained before the
    /// refutation.
    pub(super) fn rank(&self) -> usize {
        self.direct().len() + self.basis.len()
    }

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
    eliminate(rows, None, atom_count, storage, budget)
}

/// Prepare `rows` after the one-atom equations `direct`, which are checked
/// consequences of the same owner and whose vector `storage` already
/// accounts.
///
/// The units are read in order into two masks of the rows' width: the atoms
/// decided and, of those, the atoms held. A unit that repeats an earlier one
/// is dropped; one that contradicts an earlier one refutes the root. Each
/// row then has the units over its atoms added to it: its decided atoms are
/// cleared and its constant takes the parity of those held, in one chunk of
/// `W + 2`. That is the sum of checked equations, so the rows and the kept
/// units have exactly the answer sets' consequences the rows and units given
/// had. [`prepare`]'s elimination follows on rows that name no decided atom,
/// so the units it collects from the echelon are over other atoms than the
/// direct ones.
///
/// A refuted root publishes no unit, and its rank counts no unit either: the
/// pivots retained before the refutation, and none when two units disagree.
/// As in [`prepare`], a refutation is published in the chunk that proves it:
/// the result's record is admitted before the first unit is read, and a
/// unit's chunk covers the refutation it may prove.
///
/// Without a unit this is [`prepare`] after one test, less the units'
/// header, which its reservation initialized. Otherwise the masks cost
/// `2 * (3 + 2W)`, reading `u` units `13u` and adding them to `r` rows
/// `r * (W + 2)`: the work grows with the units and with the rows, and no
/// longer with the product of the units and the theory's width, as it did
/// when each unit was a row.
///
/// On any error the caller must discard this preparation and `storage`.
///
/// # Panics
/// Panics if a unit names an atom outside `atom_count`.
pub(super) fn prepare_after_units<Q: Quota + ?Sized>(
    direct: Vec<Unit>,
    rows: &mut [Equation],
    atom_count: usize,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Prepared, BudgetError<Q::Error>> {
    eliminate(rows, Some(direct), atom_count, storage, budget)
}

/// Read the direct units of `prepared` and add them to `rows`, as
/// [`prepare_after_units`] describes. Returns whether the root survives: two
/// units that disagree set `root_refuted` and leave no unit.
fn read_units<Q: Quota + ?Sized>(
    rows: &mut [Equation],
    prepared: &mut Prepared,
    atom_count: usize,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    budget.charge(1)?;
    if prepared.units.is_empty() {
        return Ok(true);
    }
    let width = atom_count.div_ceil(128) * 2;
    let mut decided = Words::new(storage.filled(width, 0_u64, 1, budget)?);
    let mut held = Words::new(storage.filled(width, 0_u64, 1, budget)?);
    let mut kept = 0;
    for position in 0..prepared.units.len() {
        // One chunk a unit: read it; locate its atom; test the decided bit;
        // then either compare the held bit and, when they disagree, publish
        // the refutation with no unit, or set both bits and store the kept
        // unit. The lengths the vector is cut to are part of the same chunk.
        budget.charge(13)?;
        let unit = prepared.units[position];
        assert!(unit.atom < atom_count, "a unit names an admitted atom");
        if decided.contains(unit.atom) {
            if held.contains(unit.atom) != unit.value {
                prepared.root_refuted = true;
                prepared.units.clear();
                storage.release(held.into_storage())?;
                storage.release(decided.into_storage())?;
                return Ok(false);
            }
            continue;
        }
        decided.insert(unit.atom);
        if unit.value {
            held.insert(unit.atom);
        }
        prepared.units[kept] = unit;
        kept += 1;
    }
    prepared.units.truncate(kept);
    for row in rows.iter_mut() {
        add_units(row, &decided, &held, budget)?;
    }
    storage.release(held.into_storage())?;
    storage.release(decided.into_storage())?;
    Ok(true)
}

/// Add to a checked row every unit over one of its atoms: clear the row's
/// decided atoms and add the parity of those held to its constant. One
/// `W + 2` chunk covers every word's two ANDs and its parity, the constant
/// and the chunk's admission, so a refusal leaves the row whole.
fn add_units<Q: Quota + ?Sized>(
    row: &mut Equation,
    decided: &[u64],
    held: &[u64],
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    assert_eq!(
        row.words.len(),
        decided.len(),
        "a row has the owner's width"
    );
    let width = u64::try_from(row.words.len()).expect("a row length fits a work word");
    budget.charge(width + 2)?;
    let mut odd = false;
    for ((word, decided), held) in row.words.iter_mut().zip(decided).zip(held) {
        odd ^= (*word & held).count_ones() & 1 == 1;
        *word &= !decided;
    }
    row.constant ^= odd;
    Ok(())
}

/// Forward elimination of `rows`. With `direct`, those units are read and
/// added to the rows first and are published before the units the echelon
/// yields; without it the units' vector is reserved here.
fn eliminate<Q: Quota + ?Sized>(
    rows: &mut [Equation],
    direct: Option<Vec<Unit>>,
    atom_count: usize,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Prepared, BudgetError<Q::Error>> {
    let mut pivots = storage.filled(atom_count, 0usize, 1, budget)?;
    let basis = storage.reserve(rows.len().min(atom_count), budget)?;
    let (units, given) = match direct {
        Some(units) => (units, true),
        None => (storage.reserve(0, budget)?, false),
    };
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
    if given && !read_units(rows, &mut prepared, atom_count, storage, budget)? {
        storage.release(pivots)?;
        return Ok(prepared);
    }
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
                    // A refuted root publishes no unit: the refutation's
                    // chunk covers the length of the direct units' vector,
                    // which keeps its capacity.
                    prepared.units.clear();
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
