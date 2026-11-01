//! One authoritative representation of a learned row: signed conditions and
//! signed outputs.
//!
//! Scalar lists remain preferable for dispersed atom IDs. Otherwise ordered
//! sparse words replace both lists. Premise words precede output words in one
//! allocation; no premise scan visits an output-only word. Each span has
//! strictly increasing word indices and nonempty words. Premise signs are
//! disjoint, as every source guarantees for a row's conditions. An output word
//! keeps an output required present in `held` and one required absent in
//! `cut`; both bits for one atom mean the row's condition admits no member of
//! its class. Logical iterators decode canonical atom IDs in increasing order.
//!
//! Preparation compares actual retained capacities, including the common enum
//! header, and publishes only a complete row. Conversion and subsumption
//! share the ledger's optional capture allowance. Refusal loses retention only;
//! allocation and global work failures preserve their typed stop. Peak storage
//! includes the simultaneous scalar witness and packed allocation. Matching
//! charges each read scalar entry or sparse word; no machine SIMD or full
//! atom-universe buffer is required.

use zetesis_algebra::{Failure, Quota};

use super::{Deduction, Literals, capture_read};
use crate::regions::{Meter, bit};
use zetesis_algebra::{Stop, words::Words};

mod merge;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Word {
    index: usize,
    held: Words<u64, [u64; 1]>,
    cut: Words<u64, [u64; 1]>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Row {
    Scalar(Literals),
    Packed {
        words: Vec<Word>,
        premise_words: usize,
        conditions: usize,
        outputs: usize,
    },
}

/// An owned cursor does not borrow the ledger while the caller publishes an
/// atom event. The caller must retain the same row until traversal finishes.
#[derive(Default)]
pub(in crate::regions) struct OutputCursor {
    next: usize,
    word: usize,
    present: Words<u64, [u64; 1]>,
    absent: Words<u64, [u64; 1]>,
}

impl Row {
    pub(super) fn prepare<Q: Quota + ?Sized>(
        literals: Literals,
        remaining: &mut u64,
        work: &mut Meter<'_, Q>,
        peak: &mut u128,
    ) -> Result<Option<Self>, Failure<Q::Error>> {
        let scalar_bytes = literals.retained_bytes();
        let simultaneous = size_of::<Self>() as u128 + size_of::<Literals>() as u128 + scalar_bytes;
        *peak = (*peak).max(simultaneous);
        let Some(premise_words) = count_words(
            literals.conditions.iter().map(|&(atom, _)| atom),
            remaining,
            work,
        )?
        else {
            return Ok(None);
        };
        let Some(output_words) = count_words(
            literals.outputs.iter().map(|&(atom, _)| atom),
            remaining,
            work,
        )?
        else {
            return Ok(None);
        };
        let count = premise_words
            .checked_add(output_words)
            .ok_or(Stop::Allocation)?;
        let requested = count as u128 * size_of::<Word>() as u128;
        if requested >= scalar_bytes {
            return Ok(Some(Self::Scalar(literals)));
        }
        let mut words = Vec::new();
        words
            .try_reserve_exact(count)
            .map_err(|_| Stop::Allocation)?;
        let packed_bytes = words.capacity() as u128 * size_of::<Word>() as u128;
        *peak = (*peak).max(simultaneous + packed_bytes);
        // Allocator capacity is authoritative, not merely the requested count.
        if packed_bytes >= scalar_bytes {
            return Ok(Some(Self::Scalar(literals)));
        }
        for &(atom, held) in &literals.conditions {
            if !capture_read(remaining, work)? {
                return Ok(None);
            }
            append(&mut words, 0, atom, held);
        }
        for &(atom, present) in &literals.outputs {
            if !capture_read(remaining, work)? {
                return Ok(None);
            }
            append(&mut words, premise_words, atom, present);
        }
        debug_assert_eq!(words.len(), count);
        Ok(Some(Self::Packed {
            words,
            premise_words,
            conditions: literals.conditions.len(),
            outputs: literals.outputs.len(),
        }))
    }

    /// This row's literals as owned lists, for retention into another tier or
    /// the store. Each decoded literal consumes the caller's allowance; None
    /// means it ran out. The copy's buffers join the caller's peak.
    pub(super) fn to_literals<Q: Quota + ?Sized>(
        &self,
        remaining: &mut u64,
        work: &mut Meter<'_, Q>,
        peak: &mut u128,
    ) -> Result<Option<Literals>, Failure<Q::Error>> {
        let (conditions, outputs) = self.lengths();
        let mut literals = Literals {
            conditions: Vec::new(),
            outputs: Vec::new(),
        };
        literals
            .conditions
            .try_reserve_exact(conditions)
            .map_err(|_| Stop::Allocation)?;
        literals
            .outputs
            .try_reserve_exact(outputs)
            .map_err(|_| Stop::Allocation)?;
        *peak = (*peak).max(size_of::<Literals>() as u128 + literals.retained_bytes());
        for condition in self.conditions() {
            if !capture_read(remaining, work)? {
                return Ok(None);
            }
            literals.conditions.push(condition);
        }
        for output in self.outputs() {
            if !capture_read(remaining, work)? {
                return Ok(None);
            }
            literals.outputs.push(output);
        }
        Ok(Some(literals))
    }

    /// Owned buffers only; the ledger accounts for every containing row header.
    pub(super) fn retained_bytes(&self) -> u128 {
        match self {
            Self::Scalar(row) => row.retained_bytes(),
            Self::Packed { words, .. } => words.capacity() as u128 * size_of::<Word>() as u128,
        }
    }

    /// The numbers of signed conditions and of signed outputs.
    pub(super) fn lengths(&self) -> (usize, usize) {
        match self {
            Self::Scalar(row) => (row.conditions.len(), row.outputs.len()),
            Self::Packed {
                conditions,
                outputs,
                ..
            } => (*conditions, *outputs),
        }
    }

    /// This row requires a subset of the proposed signed conditions and gives
    /// every proposed output. It therefore makes the proposed row redundant.
    /// Both logical streams are canonical; physical packing is immaterial.
    /// None means optional comparison allowance was exhausted, not a verdict.
    pub(super) fn subsumes<Q: Quota + ?Sized>(
        &self,
        other: &Literals,
        remaining: &mut u64,
        work: &mut Meter<'_, Q>,
    ) -> Result<Option<bool>, Failure<Q::Error>> {
        let (conditions, outputs) = self.lengths();
        if conditions > other.conditions.len() || outputs < other.outputs.len() {
            return Ok(Some(false));
        }
        match includes(
            other.conditions.iter().copied(),
            self.conditions(),
            remaining,
            work,
        )? {
            Some(true) => includes(
                self.outputs(),
                other.outputs.iter().copied(),
                remaining,
                work,
            ),
            declined_or_false => Ok(declined_or_false),
        }
    }

    /// The proposed row requires no more premises and gives all of this row's
    /// outputs. Retiring this row then preserves its deductions, or lets the
    /// stronger row refute the region before a formerly unit deduction.
    pub(super) fn subsumed_by<Q: Quota + ?Sized>(
        &self,
        other: &Literals,
        remaining: &mut u64,
        work: &mut Meter<'_, Q>,
    ) -> Result<Option<bool>, Failure<Q::Error>> {
        let (conditions, outputs) = self.lengths();
        if conditions < other.conditions.len() || outputs > other.outputs.len() {
            return Ok(Some(false));
        }
        match includes(
            self.conditions(),
            other.conditions.iter().copied(),
            remaining,
            work,
        )? {
            Some(true) => includes(
                other.outputs.iter().copied(),
                self.outputs(),
                remaining,
                work,
            ),
            declined_or_false => Ok(declined_or_false),
        }
    }

    /// Extend a temporary row with this row's outputs when their signed
    /// conditions are identical and the distinct union stays within maximum.
    /// Optional refusal or a typed stop leaves both inputs unchanged.
    pub(super) fn union_outputs<Q: Quota + ?Sized>(
        &self,
        other: &mut Literals,
        maximum: usize,
        remaining: &mut u64,
        work: &mut Meter<'_, Q>,
        peak: &mut u128,
    ) -> Result<Option<bool>, Failure<Q::Error>> {
        merge::union_outputs(self, other, maximum, remaining, work, peak)
    }

    pub(super) fn conditions(&self) -> impl Iterator<Item = (usize, bool)> + '_ {
        let mut scalar = match self {
            Self::Scalar(row) => Some(row.conditions.iter()),
            Self::Packed { .. } => None,
        };
        let words = match self {
            Self::Packed {
                words,
                premise_words,
                ..
            } => &words[..*premise_words],
            Self::Scalar(_) => &[],
        };
        let mut groups = words.iter();
        let mut current = Word {
            index: 0,
            held: Words::new([0]),
            cut: Words::new([0]),
        };
        std::iter::from_fn(move || {
            if let Some(values) = &mut scalar {
                return values.next().copied();
            }
            if current.held[0] | current.cut[0] == 0 {
                current = *groups.next()?;
            }
            let bit = Words::<u64>::first_bit(current.held[0] | current.cut[0]);
            let mask = Words::<u64>::locate(bit).1;
            let held = current.held[0] & mask != 0;
            current.held[0] &= !mask;
            current.cut[0] &= !mask;
            Some((current.index * 64 + bit, held))
        })
    }

    /// Every atom the row mentions, conditions first, each output atom once.
    pub(super) fn atoms(&self) -> impl Iterator<Item = usize> + '_ {
        let mut previous = None;
        self.conditions()
            .map(|(atom, _)| atom)
            .chain(self.outputs().filter_map(move |(atom, _)| {
                let fresh = previous != Some(atom);
                previous = Some(atom);
                fresh.then_some(atom)
            }))
    }

    #[cfg(test)]
    pub(super) fn output_count(&self) -> usize {
        self.lengths().1
    }

    /// Signed outputs in ascending (atom, sign) order.
    pub(super) fn outputs(&self) -> impl Iterator<Item = (usize, bool)> + '_ {
        let mut cursor = OutputCursor::default();
        std::iter::from_fn(move || self.next_output(&mut cursor))
    }

    pub(super) fn next_output(&self, cursor: &mut OutputCursor) -> Option<(usize, bool)> {
        match self {
            Self::Scalar(row) => {
                let output = row.outputs.get(cursor.next).copied()?;
                cursor.next += 1;
                Some(output)
            }
            Self::Packed {
                words,
                premise_words,
                ..
            } => {
                if cursor.absent[0] | cursor.present[0] == 0 {
                    let group = words.get(*premise_words + cursor.next)?;
                    cursor.next += 1;
                    cursor.word = group.index;
                    cursor.absent[0] = group.cut[0];
                    cursor.present[0] = group.held[0];
                }
                Some(cursor.take())
            }
        }
    }

    /// Visit outputs after a complete premise match. Scalar rows retain their
    /// one-read-per-output traversal. Packed words omit outputs already
    /// satisfied before decoding atom IDs; an output contradicted by its atom's
    /// opposite decision remains a visit, so the caller refutes the region.
    ///
    /// Each source word and each returned packed output is charged before its
    /// read. The cursor owns only this traversal's remaining bits, not knowledge
    /// reusable by another row or region. Between visits the caller may decide
    /// the returned atom; other mask changes require starting a fresh cursor.
    pub(super) fn next_publication_output<Q: Quota + ?Sized>(
        &self,
        cursor: &mut OutputCursor,
        held: &[u64],
        cut: &[u64],
        work: &mut Meter<'_, Q>,
    ) -> Result<Option<(usize, bool)>, Failure<Q::Error>> {
        match self {
            Self::Scalar(row) => {
                let Some(output) = row.outputs.get(cursor.next) else {
                    return Ok(None);
                };
                work.tick()?;
                cursor.next += 1;
                Ok(Some(*output))
            }
            Self::Packed {
                words,
                premise_words,
                ..
            } => {
                while cursor.absent[0] | cursor.present[0] == 0 {
                    let Some(group) = words.get(*premise_words + cursor.next) else {
                        return Ok(None);
                    };
                    work.tick()?;
                    cursor.word = group.index;
                    cursor.absent[0] = group.cut[0] & (held[group.index] | !cut[group.index]);
                    cursor.present[0] = group.held[0] & (cut[group.index] | !held[group.index]);
                    cursor.next += 1;
                }
                work.tick()?;
                Ok(Some(cursor.take()))
            }
        }
    }

    pub(super) fn deduction<Q: Quota + ?Sized>(
        &self,
        held: &[u64],
        cut: &[u64],
        work: &mut Meter<'_, Q>,
    ) -> Result<Deduction, Failure<Q::Error>> {
        work.tick()?;
        match self {
            Self::Scalar(row) => scalar_deduction(row, held, cut, work),
            Self::Packed {
                words,
                premise_words,
                ..
            } => packed_deduction(
                &words[..*premise_words],
                &words[*premise_words..],
                held,
                cut,
                work,
            ),
        }
    }
}

impl OutputCursor {
    /// The lowest pending output of the current word, an absence before a
    /// presence on the same atom, as the canonical order sorts them.
    fn take(&mut self) -> (usize, bool) {
        let absent = Words::<u64>::lowest_bit(self.absent[0]);
        let present = Words::<u64>::lowest_bit(self.present[0]);
        let take_absent = present == 0 || (absent != 0 && absent <= present);
        let mask = if take_absent { absent } else { present };
        if take_absent {
            self.absent[0] &= !mask;
        } else {
            self.present[0] &= !mask;
        }
        (self.word * 64 + Words::<u64>::first_bit(mask), !take_absent)
    }
}

/// Ordered inclusion uses constant scratch and linear comparisons. Each visited
/// candidate consumes the existing capture allowance; exhaustion is undecided.
fn includes<T: Ord, Q: Quota + ?Sized>(
    mut candidates: impl Iterator<Item = T>,
    required: impl Iterator<Item = T>,
    remaining: &mut u64,
    work: &mut Meter<'_, Q>,
) -> Result<Option<bool>, Failure<Q::Error>> {
    for wanted in required {
        loop {
            if !capture_read(remaining, work)? {
                return Ok(None);
            }
            let Some(candidate) = candidates.next() else {
                return Ok(Some(false));
            };
            match candidate.cmp(&wanted) {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => break,
                std::cmp::Ordering::Greater => return Ok(Some(false)),
            }
        }
    }
    Ok(Some(true))
}

fn count_words<Q: Quota + ?Sized>(
    atoms: impl Iterator<Item = usize>,
    remaining: &mut u64,
    work: &mut Meter<'_, Q>,
) -> Result<Option<usize>, Failure<Q::Error>> {
    let mut previous = None;
    let mut count = 0_usize;
    for atom in atoms {
        if !capture_read(remaining, work)? {
            return Ok(None);
        }
        let group = atom / 64;
        if previous != Some(group) {
            count = count.checked_add(1).ok_or(Stop::Allocation)?;
            previous = Some(group);
        }
    }
    Ok(Some(count))
}

/// The count pass reserved every group. Each charged input writes at most one
/// group; the separate spans never merge a premise group with an output group.
fn append(words: &mut Vec<Word>, start: usize, atom: usize, held: bool) {
    let index = atom / 64;
    if words.len() == start || words.last().is_none_or(|word| word.index != index) {
        words.push(Word {
            index,
            held: Words::new([0]),
            cut: Words::new([0]),
        });
    }
    let word = words
        .last_mut()
        .expect("the input appended or matched one word");
    let mask = Words::<u64>::locate(atom).1;
    if held {
        word.held[0] |= mask;
    } else {
        word.cut[0] |= mask;
    }
}

fn scalar_deduction<Q: Quota + ?Sized>(
    row: &Literals,
    held: &[u64],
    cut: &[u64],
    work: &mut Meter<'_, Q>,
) -> Result<Deduction, Failure<Q::Error>> {
    let mut open = None;
    for &(atom, value) in &row.conditions {
        work.tick()?;
        if bit(if value { held } else { cut }, atom) {
            continue;
        }
        if bit(if value { cut } else { held }, atom) || open.is_some() {
            return Ok(Deduction::None);
        }
        open = Some((atom, !value));
    }
    let Some((atom, value)) = open else {
        return Ok(Deduction::ForceOutputs);
    };
    for &(output, present) in &row.outputs {
        work.tick()?;
        if bit(if present { cut } else { held }, output) {
            return Ok(Deduction::Require { atom, value });
        }
    }
    Ok(Deduction::None)
}

fn packed_deduction<Q: Quota + ?Sized>(
    premises: &[Word],
    outputs: &[Word],
    held: &[u64],
    cut: &[u64],
    work: &mut Meter<'_, Q>,
) -> Result<Deduction, Failure<Q::Error>> {
    let mut open = None;
    for group in premises {
        work.tick()?;
        let present = held[group.index];
        let absent = cut[group.index];
        let matched = (group.held[0] & present) | (group.cut[0] & absent);
        let unknown = (group.held[0] | group.cut[0]) & !matched;
        // Preserve scalar precedence even if both knowledge bits are set:
        // an expected sign already present satisfies that premise.
        if unknown & (present | absent) != 0 {
            return Ok(Deduction::None);
        }
        if unknown == 0 {
            continue;
        }
        if open.is_some() || unknown & (unknown - 1) != 0 {
            return Ok(Deduction::None);
        }
        let bit = Words::<u64>::first_bit(unknown);
        open = Some((group.index * 64 + bit, group.cut[0] & unknown != 0));
    }
    let Some((atom, value)) = open else {
        return Ok(Deduction::ForceOutputs);
    };
    for group in outputs {
        work.tick()?;
        // An output's negation is decided: a required absence held, or a
        // required presence cut.
        if (group.cut[0] & held[group.index]) | (group.held[0] & cut[group.index]) != 0 {
            return Ok(Deduction::Require { atom, value });
        }
    }
    Ok(Deduction::None)
}
