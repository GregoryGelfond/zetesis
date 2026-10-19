//! One authoritative representation of a conditional unsupported set.
//!
//! Scalar lists remain preferable for dispersed atom IDs. Otherwise ordered
//! sparse words replace both lists. Premise words precede output words in one
//! allocation; no premise scan visits an output-only word. Each span has
//! strictly increasing word indices and nonempty words. Premise signs are
//! disjoint, as guaranteed by the completed collector; output words use only
//! `held`. Logical iterators decode canonical atom IDs in increasing order.
//!
//! Preparation compares actual retained capacities, including the common enum
//! header, and publishes only a complete row. Conversion and subsumption
//! share the ledger's optional capture allowance. Refusal loses retention only;
//! allocation and global work failures preserve their typed stop. Peak storage
//! includes the simultaneous scalar witness and packed allocation. Matching
//! charges each read scalar entry or sparse word; no machine SIMD or full
//! atom-universe buffer is required.

use super::{Deduction, capture_read};
use crate::regions::founded::explanations::Explanation;
use crate::regions::{Work, bit};
use zetesis_cpu::Stop;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Word {
    index: usize,
    held: u64,
    cut: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Row {
    Scalar(Explanation),
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
    pending: u64,
}

impl Row {
    pub(super) fn prepare(
        explanation: Explanation,
        remaining: &mut u64,
        work: &mut Work<'_>,
        peak: &mut u128,
    ) -> Result<Option<Self>, Stop> {
        let scalar_bytes = explanation.retained_bytes();
        let simultaneous =
            size_of::<Self>() as u128 + size_of::<Explanation>() as u128 + scalar_bytes;
        *peak = (*peak).max(simultaneous);
        let Some(premise_words) = count_words(
            explanation.conditions.iter().map(|&(atom, _)| atom),
            remaining,
            work,
        )?
        else {
            return Ok(None);
        };
        let Some(output_words) =
            count_words(explanation.unsupported.iter().copied(), remaining, work)?
        else {
            return Ok(None);
        };
        let count = premise_words
            .checked_add(output_words)
            .ok_or(Stop::Allocation)?;
        let requested = count as u128 * size_of::<Word>() as u128;
        if requested >= scalar_bytes {
            return Ok(Some(Self::Scalar(explanation)));
        }
        let mut words = Vec::new();
        words
            .try_reserve_exact(count)
            .map_err(|_| Stop::Allocation)?;
        let packed_bytes = words.capacity() as u128 * size_of::<Word>() as u128;
        *peak = (*peak).max(simultaneous + packed_bytes);
        // Allocator capacity is authoritative, not merely the requested count.
        if packed_bytes >= scalar_bytes {
            return Ok(Some(Self::Scalar(explanation)));
        }
        for &(atom, held) in &explanation.conditions {
            if !capture_read(remaining, work)? {
                return Ok(None);
            }
            append(&mut words, 0, atom, held);
        }
        for &atom in &explanation.unsupported {
            if !capture_read(remaining, work)? {
                return Ok(None);
            }
            append(&mut words, premise_words, atom, true);
        }
        debug_assert_eq!(words.len(), count);
        Ok(Some(Self::Packed {
            words,
            premise_words,
            conditions: explanation.conditions.len(),
            outputs: explanation.unsupported.len(),
        }))
    }

    /// Owned buffers only; the ledger accounts for every containing row header.
    pub(super) fn retained_bytes(&self) -> u128 {
        match self {
            Self::Scalar(row) => row.retained_bytes(),
            Self::Packed { words, .. } => words.capacity() as u128 * size_of::<Word>() as u128,
        }
    }

    fn lengths(&self) -> (usize, usize) {
        match self {
            Self::Scalar(row) => (row.conditions.len(), row.unsupported.len()),
            Self::Packed {
                conditions,
                outputs,
                ..
            } => (*conditions, *outputs),
        }
    }

    /// This row requires a subset of the proposed signed conditions and excludes
    /// every proposed output. It therefore makes the proposed row redundant.
    /// Both logical streams are canonical; physical packing is immaterial.
    /// None means optional comparison allowance was exhausted, not a verdict.
    pub(super) fn subsumes(
        &self,
        other: &Explanation,
        remaining: &mut u64,
        work: &mut Work<'_>,
    ) -> Result<Option<bool>, Stop> {
        let (conditions, outputs) = self.lengths();
        if conditions > other.conditions.len() || outputs < other.unsupported.len() {
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
                other.unsupported.iter().copied(),
                remaining,
                work,
            ),
            declined_or_false => Ok(declined_or_false),
        }
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
            held: 0,
            cut: 0,
        };
        std::iter::from_fn(move || {
            if let Some(values) = &mut scalar {
                return values.next().copied();
            }
            if current.held | current.cut == 0 {
                current = *groups.next()?;
            }
            let bit = (current.held | current.cut).trailing_zeros() as usize;
            let mask = 1 << bit;
            let held = current.held & mask != 0;
            current.held &= !mask;
            current.cut &= !mask;
            Some((current.index * 64 + bit, held))
        })
    }

    pub(super) fn output_count(&self) -> usize {
        self.lengths().1
    }

    pub(super) fn outputs(&self) -> impl Iterator<Item = usize> + '_ {
        let mut cursor = OutputCursor::default();
        std::iter::from_fn(move || self.next_output(&mut cursor))
    }

    pub(super) fn next_output(&self, cursor: &mut OutputCursor) -> Option<usize> {
        match self {
            Self::Scalar(row) => {
                let atom = row.unsupported.get(cursor.next).copied()?;
                cursor.next += 1;
                Some(atom)
            }
            Self::Packed {
                words,
                premise_words,
                ..
            } => {
                if cursor.pending == 0 {
                    let group = words.get(*premise_words + cursor.next)?;
                    cursor.next += 1;
                    cursor.word = group.index;
                    cursor.pending = group.held;
                }
                let bit = cursor.pending.trailing_zeros() as usize;
                cursor.pending &= cursor.pending - 1;
                Some(cursor.word * 64 + bit)
            }
        }
    }

    pub(super) fn deduction(
        &self,
        held: &[u64],
        cut: &[u64],
        work: &mut Work<'_>,
    ) -> Result<Deduction, Stop> {
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

/// Ordered inclusion uses constant scratch and linear comparisons. Each visited
/// candidate consumes the existing capture allowance; exhaustion is undecided.
fn includes<T: Ord>(
    mut candidates: impl Iterator<Item = T>,
    required: impl Iterator<Item = T>,
    remaining: &mut u64,
    work: &mut Work<'_>,
) -> Result<Option<bool>, Stop> {
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

fn count_words(
    atoms: impl Iterator<Item = usize>,
    remaining: &mut u64,
    work: &mut Work<'_>,
) -> Result<Option<usize>, Stop> {
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
            held: 0,
            cut: 0,
        });
    }
    let word = words
        .last_mut()
        .expect("the input appended or matched one word");
    let mask = 1 << (atom % 64);
    if held {
        word.held |= mask;
    } else {
        word.cut |= mask;
    }
}

fn scalar_deduction(
    row: &Explanation,
    held: &[u64],
    cut: &[u64],
    work: &mut Work<'_>,
) -> Result<Deduction, Stop> {
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
        return Ok(Deduction::ExcludeOutputs);
    };
    for &output in &row.unsupported {
        work.tick()?;
        if bit(held, output) {
            return Ok(Deduction::Require { atom, value });
        }
    }
    Ok(Deduction::None)
}

fn packed_deduction(
    premises: &[Word],
    outputs: &[Word],
    held: &[u64],
    cut: &[u64],
    work: &mut Work<'_>,
) -> Result<Deduction, Stop> {
    let mut open = None;
    for group in premises {
        work.tick()?;
        let present = held[group.index];
        let absent = cut[group.index];
        let matched = (group.held & present) | (group.cut & absent);
        let unknown = (group.held | group.cut) & !matched;
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
        let bit = unknown.trailing_zeros() as usize;
        open = Some((group.index * 64 + bit, group.cut & unknown != 0));
    }
    let Some((atom, value)) = open else {
        return Ok(Deduction::ExcludeOutputs);
    };
    for group in outputs {
        work.tick()?;
        if group.held & held[group.index] != 0 {
            return Ok(Deduction::Require { atom, value });
        }
    }
    Ok(Deduction::None)
}
