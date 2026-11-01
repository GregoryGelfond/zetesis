//! Completion patterns from an exact owner's complete normal definitions.
//!
//! Bodies are read twice: first for their distinct atom support, then for their
//! truth masks. Every nested conjunction occurrence is visited on each pass;
//! repeated or contradictory literals are never skipped during decoding.

use std::mem::size_of;

use zetesis_algebra::Stop;
use zetesis_algebra::words;

use crate::{AdmissionError, Producers, Theory};

use super::{
    budget::{Budget, BudgetError},
    conjunction::Conjunction,
    source::{MAX_SUPPORT, Support, canonical_truth, literal},
    storage::Storage,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Completion {
    pub head: usize,
    pub support: Support,
    /// Exactly the patterns that disagree with the head's completion.
    /// Unused bits and words are zero.
    pub forbidden: [u64; 4],
}

fn include(
    support: &mut Support,
    atom: usize,
    budget: &mut Budget<'_, '_>,
) -> Result<bool, BudgetError> {
    for known in &support.atoms[..support.len] {
        budget.charge(1)?;
        if *known == atom {
            budget.charge(1)?;
            return Ok(true);
        }
    }
    budget.charge(1)?;
    if support.len == MAX_SUPPORT {
        budget.charge(1)?;
        return Ok(false);
    }
    budget.charge(3)?;
    support.atoms[support.len] = atom;
    support.len += 1;
    Ok(true)
}

fn sort(support: &mut Support, budget: &mut Budget<'_, '_>) -> Result<(), BudgetError> {
    for position in 1..support.len {
        budget.charge(1)?;
        let atom = support.atoms[position];
        let mut destination = position;
        while destination > 0 {
            budget.charge(1)?;
            if support.atoms[destination - 1] <= atom {
                break;
            }
            budget.charge(1)?;
            support.atoms[destination] = support.atoms[destination - 1];
            destination -= 1;
        }
        budget.charge(1)?;
        support.atoms[destination] = atom;
    }
    Ok(())
}

fn local_bit(
    support: &Support,
    atom: usize,
    budget: &mut Budget<'_, '_>,
) -> Result<u64, BudgetError> {
    for (position, known) in support.atoms[..support.len].iter().enumerate() {
        budget.charge(1)?;
        if *known == atom {
            budget.charge(2)?;
            return Ok(1 << position);
        }
    }
    budget.charge(1)?;
    Err(BudgetError::Source(AdmissionError::Atom))
}

/// Reconstruct all forbidden patterns of a head's complete normal definition.
/// The returned table proposes consequences of answer sets, not arbitrary
/// classical models. A trusted row check must call this again for each proposal.
pub(super) fn completion(
    theory: &Theory,
    producers: &Producers,
    head: usize,
    live_bytes: u128,
    budget: &mut Budget<'_, '_>,
) -> Result<Option<Completion>, BudgetError> {
    budget.charge(1)?;
    if !producers.belongs_to(theory) {
        budget.charge(1)?;
        return Ok(None);
    }
    budget.charge(1)?;
    if head >= theory.atom_count() {
        budget.charge(1)?;
        return Err(BudgetError::Source(AdmissionError::Atom));
    }
    budget.charge(4)?; // two adjacency offsets and the borrowed span
    let definitions = &producers.by_head[head];
    let base = live_bytes
        .checked_add(size_of::<Completion>() as u128)
        .and_then(|bytes| bytes.checked_add(2 * size_of::<u64>() as u128))
        .ok_or(BudgetError::Stopped(Stop::Allocation))?;
    budget.admit_bytes(base)?;
    let mut storage = Storage::with_live_bytes(base);
    budget.charge(15)?; // head, support's nine words, initial head, four table words
    let mut output = Completion {
        head,
        support: Support {
            atoms: [0; MAX_SUPPORT],
            len: 1,
        },
        forbidden: [0; 4],
    };
    output.support.atoms[0] = head;
    let mut frames = Conjunction::new(&mut storage, budget)?;
    if !collect_support(
        theory,
        producers,
        definitions,
        &mut output,
        &mut frames,
        &mut storage,
        budget,
    )? {
        frames.release(&mut storage)?;
        budget.charge(1)?;
        return Ok(None);
    }
    sort(&mut output.support, budget)?;
    // The table scratch belongs only to this pass, alongside the retained
    // borrowed frames. Admission precedes its initialization and every use.
    let decoded =
        storage.with_record(size_of::<[u64; 4]>() as u128, budget, |storage, meter| {
            completion_table(
                theory,
                producers,
                definitions,
                &mut output,
                &mut frames,
                storage,
                meter,
            )
        })?;
    if !decoded {
        frames.release(&mut storage)?;
        budget.charge(1)?;
        return Ok(None);
    }
    frames.release(&mut storage)?;
    budget.charge(1)?;
    Ok(Some(output))
}

fn completion_table<'a>(
    theory: &'a Theory,
    producers: &Producers,
    definitions: &[usize],
    output: &mut Completion,
    frames: &mut Conjunction<'a>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
) -> Result<bool, BudgetError> {
    let head_bit = local_bit(&output.support, output.head, budget)?;
    budget.charge(1)?;
    let head_coordinate = head_bit.trailing_zeros() as usize;
    budget.charge(1)?;
    let extent = 1_usize << output.support.len;
    let word_count = extent.div_ceil(64);
    budget.charge(1)?;
    let valid = if extent < 64 {
        budget.charge(2)?;
        (1_u64 << extent) - 1
    } else {
        budget.charge(1)?;
        u64::MAX
    };
    budget.charge(4)?;
    let mut body_table = [0_u64; 4];
    for rule in definitions {
        budget.charge(1)?; // read the retained rule identity
        let rule = *rule;
        budget.charge(1)?; // inspect this validated, immutable producer's body
        budget.charge(2)?;
        let mut positive = 0_u64;
        let mut negative = 0_u64;
        if let Some(body) = &producers.rules[rule].body {
            budget.charge(1)?;
            let body_view = budget.node(theory, *body)?;
            let decoded = canonical_truth(theory, body_view, budget)?
                || frames.walk_view(
                    theory,
                    (*body, body_view),
                    storage,
                    budget,
                    |_, view, _, meter| {
                        let Some(value) = literal(theory, view, meter)? else {
                            return Ok(false);
                        };
                        let bit = local_bit(&output.support, value.atom, meter)?;
                        meter.charge(2)?;
                        if value.positive {
                            positive |= bit;
                        } else {
                            negative |= bit;
                        }
                        Ok(true)
                    },
                )?;
            if !decoded {
                budget.charge(1)?;
                return Ok(false);
            }
        }
        // Source decoding is complete before this idempotent reduction. A
        // contradiction never skips the remaining source occurrences or rules.
        budget.charge(word_count as u64)?;
        body_table[..word_count].fill(valid);
        intersect_literals(&mut body_table[..word_count], positive, true, budget)?;
        intersect_literals(&mut body_table[..word_count], negative, false, budget)?;
        words::union_assign(
            &mut output.forbidden[..word_count],
            &body_table[..word_count],
            &mut |work| budget.charge(work),
        )?;
    }
    // Reuse the body scratch for the head column. Both operands have zero
    // padding, so their symmetric difference denotes exactly disagreement.
    for (word, value) in body_table[..word_count].iter_mut().enumerate() {
        let head = truth_column(head_coordinate, word, budget)?;
        budget.charge(1)?;
        *value = head & valid;
    }
    words::xor_assign(
        &mut output.forbidden[..word_count],
        &body_table[..word_count],
        &mut || budget.charge(1),
    )?;
    budget.charge(1)?;
    Ok(true)
}

/// Intersect the selected signed columns. The occurrence decoder has already
/// checked and charged every literal; duplicate masks preserve conjunction.
fn intersect_literals(
    table: &mut [u64],
    selected: u64,
    positive: bool,
    budget: &mut Budget<'_, '_>,
) -> Result<(), BudgetError> {
    words::visit_set_bits(
        std::slice::from_ref(&selected),
        budget,
        &mut |meter, work| meter.charge(work),
        &mut |meter, coordinate| {
            for (word, value) in table.iter_mut().enumerate() {
                let column = truth_column(coordinate, word, meter)?;
                meter.charge(1)?;
                if positive {
                    meter.charge(1)?;
                    *value &= column;
                } else {
                    meter.charge(2)?;
                    *value &= !column;
                }
            }
            Ok(())
        },
    )
}

/// Column `coordinate` has bit `pattern` iff that coordinate is true in the
/// pattern. Coordinates below six alternate within a word; six and seven are
/// constant within each word. The caller supplies coordinates below eight.
fn truth_column(
    coordinate: usize,
    word: usize,
    budget: &mut Budget<'_, '_>,
) -> Result<u64, BudgetError> {
    // Mathematical column constants, shared read-only rather than prepared
    // per definition. Universe padding is masked by the caller.
    static LOW_COLUMNS: [u64; 6] = [
        0xaaaa_aaaa_aaaa_aaaa,
        0xcccc_cccc_cccc_cccc,
        0xf0f0_f0f0_f0f0_f0f0,
        0xff00_ff00_ff00_ff00,
        0xffff_0000_ffff_0000,
        0xffff_ffff_0000_0000,
    ];
    budget.charge(1)?;
    if coordinate < 6 {
        budget.charge(2)?; // column read and successful result tag
        Ok(LOW_COLUMNS[coordinate])
    } else {
        budget.charge(4)?;
        let bit = (word >> (coordinate - 6)) & 1;
        let column = 0_u64.wrapping_sub(bit as u64);
        budget.charge(1)?;
        Ok(column)
    }
}

fn collect_support<'a>(
    theory: &'a Theory,
    producers: &Producers,
    definitions: &[usize],
    output: &mut Completion,
    frames: &mut Conjunction<'a>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
) -> Result<bool, BudgetError> {
    for rule in definitions {
        budget.charge(2)?; // rule identity and range check
        let rule = *rule;
        let producer = producers
            .rules
            .get(rule)
            .ok_or(BudgetError::Source(AdmissionError::Edge))?;
        budget.charge(1)?;
        if producer.choice {
            budget.charge(1)?;
            return Ok(false);
        }
        budget.charge(1)?;
        if producer.heads.len() != 1 {
            budget.charge(1)?;
            return Ok(false);
        }
        budget.charge(1)?;
        if producer.heads[0] != output.head {
            budget.charge(1)?;
            return Ok(false);
        }
        budget.charge(1)?;
        if let Some(body) = &producer.body {
            budget.charge(1)?;
            let body_view = budget.node(theory, *body)?;
            if !canonical_truth(theory, body_view, budget)?
                && !frames.walk_view(
                    theory,
                    (*body, body_view),
                    storage,
                    budget,
                    |_, view, _, meter| {
                        let Some(value) = literal(theory, view, meter)? else {
                            return Ok(false);
                        };
                        include(&mut output.support, value.atom, meter)
                    },
                )?
            {
                budget.charge(1)?;
                return Ok(false);
            }
        }
    }
    budget.charge(1)?;
    Ok(true)
}
