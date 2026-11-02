//! Parity proposals accepted only after an independent source check.
//!
//! Clause records are sorted by canonical support with a charged stable merge
//! sort. Definition records name an exact-owner complete producer span. Every
//! proposed equation is materialized in the original atom coordinates and then
//! checked by rereading its own sources. Only checked rows reach elimination.

use std::cmp::Ordering;
use std::mem::size_of;

use crate::{Theory, regions::Producers};

use super::{
    budget::{Budget, BudgetError},
    completion,
    hull::{self, LocalEquation},
    root_facts::{self, CheckedRootFacts},
    source::{self, Clause, Support},
    storage::Storage,
    work::Phase,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Source {
    Clauses { first: usize, count: usize },
    Definition { head: usize },
}

/// An independently checked consequence of the exact owner's answer sets.
/// Row addition is the only subsequent coefficient/constant mutation.
#[derive(Debug)]
pub(super) struct Equation {
    pub words: Vec<u64>,
    pub constant: bool,
    pub source: Source,
}

#[derive(Default, Debug)]
pub(super) struct Counts {
    pub groups: u64,
    pub rows: u64,
    pub inexact_hulls: u64,
    pub one: u64,
    pub two: u64,
    pub wider: u64,
    pub unsupported_clauses: u64,
    pub unsupported_definitions: u64,
    pub provenance_rejected: u64,
}

#[derive(Debug)]
pub(super) struct Recognized {
    pub facts: CheckedRootFacts,
    pub clauses: Vec<Clause>,
    pub equations: Vec<Equation>,
    pub counts: Counts,
}

impl Recognized {
    pub(super) fn retained_bytes(&self) -> u128 {
        (size_of::<Self>() - size_of::<CheckedRootFacts>()) as u128
            + self.facts.retained_bytes()
            + (self.clauses.capacity() * size_of::<Clause>()) as u128
            + (self.equations.capacity() * size_of::<Equation>()) as u128
            + self
                .equations
                .iter()
                .map(|row| row.words.capacity() as u128 * size_of::<u64>() as u128)
                .sum::<u128>()
    }
}

pub(super) fn recognize(
    theory: &Theory,
    producers: Option<&Producers>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
) -> Result<Recognized, BudgetError> {
    storage.admit_record(
        (size_of::<Recognized>() - size_of::<CheckedRootFacts>() - 2 * size_of::<Vec<Clause>>())
            as u128,
        budget,
    )?;
    let facts = root_facts::build(theory, storage, budget)?;
    let mut clauses = storage.reserve(0, budget)?;
    let mut equations = storage.reserve(0, budget)?;
    budget.charge(9)?;
    let mut counts = Counts::default();
    for root in 0..theory.roots().len() {
        let read = storage.with_record(
            source::CONDITIONED_SCRATCH_BYTES as u128,
            budget,
            |storage, budget| source::conditioned_clause(theory, &facts, root, storage, budget),
        )?;
        if let Some(clause) = read {
            storage.push(&mut clauses, clause, 11, budget)?;
        } else {
            budget.charge(1)?;
            counts.unsupported_clauses += 1;
        }
    }
    sort_clauses(&mut clauses, storage, budget)?;
    from_clauses(
        &facts,
        producers,
        &clauses,
        &mut equations,
        &mut counts,
        storage,
        budget,
    )?;
    from_definitions(
        &facts,
        producers,
        &clauses,
        &mut equations,
        &mut counts,
        storage,
        budget,
    )?;
    budget.charge(1)?; // publish the successful tag; facts already retain the owner
    Ok(Recognized {
        facts,
        clauses,
        equations,
        counts,
    })
}

/// Propose and independently authenticate affine rows from equal-support clauses.
fn from_clauses(
    facts: &CheckedRootFacts,
    producers: Option<&Producers>,
    clauses: &[Clause],
    equations: &mut Vec<Equation>,
    counts: &mut Counts,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
) -> Result<(), BudgetError> {
    let theory = facts.owner();
    let mut first = 0;
    while first < clauses.len() {
        let fixed =
            (size_of::<Support>() + size_of::<[u64; 4]>() + size_of::<hull::Hull>()) as u128;
        storage.admit_record(fixed, budget)?;
        budget.charge(9)?;
        let support = clauses[first].support;
        let mut end = first + 1;
        while end < clauses.len() {
            let order = compare_support(support, clauses[end].support, budget)?;
            budget.charge(1)?;
            if order != Ordering::Equal {
                break;
            }
            end += 1;
        }
        budget.charge(4)?;
        let mut forbidden = [0_u64; 4];
        for clause in &clauses[first..end] {
            budget.charge(4)?;
            forbidden[usize::from(clause.forbidden) / 64] |=
                1 << (usize::from(clause.forbidden) % 64);
        }
        let proposed = storage.with_record(hull::SCRATCH_BYTES as u128, budget, |_, budget| {
            propose(support, &forbidden, counts, budget)
        })?;
        for &local in proposed.equations() {
            let words = materialize(theory, support, local, storage, budget)?;
            budget.charge(5)?; // constant, source tag, range start, range length
            let candidate = Equation {
                words,
                constant: local.constant,
                source: Source::Clauses {
                    first,
                    count: end - first,
                },
            };
            let valid = budget.in_phase(Phase::Provenance, |budget| {
                storage.with_record(
                    source::CHECK_SCRATCH_BYTES as u128,
                    budget,
                    |storage, budget| {
                        check(
                            facts,
                            producers,
                            clauses,
                            &candidate,
                            support,
                            storage.bytes(),
                            budget,
                        )
                    },
                )
            })?;
            retain(
                valid,
                candidate,
                local.coefficients,
                equations,
                counts,
                storage,
                budget,
            )?;
        }
        first = end;
        storage.release_record(fixed)?;
    }
    Ok(())
}

/// Propose and independently authenticate affine rows from complete definitions.
fn from_definitions(
    facts: &CheckedRootFacts,
    producers: Option<&Producers>,
    clauses: &[Clause],
    equations: &mut Vec<Equation>,
    counts: &mut Counts,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
) -> Result<(), BudgetError> {
    let theory = facts.owner();
    if let Some(producers) = producers {
        for head in 0..theory.atom_count() {
            let Some(group) =
                completion::completion(theory, producers, head, storage.bytes(), budget)?
            else {
                budget.charge(1)?;
                counts.unsupported_definitions += 1;
                continue;
            };
            let fixed = (size_of::<completion::Completion>() + size_of::<hull::Hull>()) as u128;
            storage.admit_record(fixed, budget)?;
            let proposed =
                storage.with_record(hull::SCRATCH_BYTES as u128, budget, |_, budget| {
                    propose(group.support, &group.forbidden, counts, budget)
                })?;
            for &local in proposed.equations() {
                let words = materialize(theory, group.support, local, storage, budget)?;
                budget.charge(3)?; // constant, definition-source tag and head
                let candidate = Equation {
                    words,
                    constant: local.constant,
                    source: Source::Definition { head },
                };
                let valid = budget.in_phase(Phase::Provenance, |budget| {
                    check(
                        facts,
                        Some(producers),
                        clauses,
                        &candidate,
                        group.support,
                        storage.bytes(),
                        budget,
                    )
                })?;
                retain(
                    valid,
                    candidate,
                    local.coefficients,
                    equations,
                    counts,
                    storage,
                    budget,
                )?;
            }
            storage.release_record(fixed)?;
        }
    }
    Ok(())
}

fn propose(
    support: Support,
    forbidden: &[u64; 4],
    counts: &mut Counts,
    budget: &mut Budget<'_, '_>,
) -> Result<hull::Hull, BudgetError> {
    budget.charge(1)?;
    counts.groups += 1;
    budget.charge(2)?; // choose and publish the padded local-table width
    let words = if support.len == 8 { 4 } else { 2 };
    let proposed = hull::propose(support.len, &forbidden[..words], &mut |work| {
        budget.charge(work)
    })?;
    budget.charge(1)?;
    if !proposed.exact {
        budget.charge(1)?;
        counts.inexact_hulls += 1;
    }
    Ok(proposed)
}

fn materialize(
    theory: &Theory,
    support: Support,
    local: LocalEquation,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
) -> Result<Vec<u64>, BudgetError> {
    let mut words = storage.filled(theory.atom_count().div_ceil(128) * 2, 0, 1, budget)?;
    for (coordinate, &atom) in support.atoms[..support.len].iter().enumerate() {
        budget.charge(3)?;
        if local.coefficients & (1 << coordinate) != 0 {
            budget.charge(4)?;
            words[atom / 64] |= 1 << (atom % 64);
        }
    }
    Ok(words)
}

fn check(
    facts: &CheckedRootFacts,
    producers: Option<&Producers>,
    clauses: &[Clause],
    candidate: &Equation,
    support: Support,
    live_bytes: u128,
    budget: &mut Budget<'_, '_>,
) -> Result<bool, BudgetError> {
    let theory = facts.owner();
    budget.charge(1)?;
    match candidate.source {
        Source::Clauses { first, count } => {
            let end = first
                .checked_add(count)
                .ok_or(BudgetError::Stopped(zetesis_cpu::Stop::InvalidProgram))?;
            let roots = clauses
                .get(first..end)
                .ok_or(BudgetError::Stopped(zetesis_cpu::Stop::InvalidProgram))?;
            source::check_clause_equation(
                theory,
                facts,
                roots,
                support,
                (&candidate.words, candidate.constant),
                &mut Storage::with_live_bytes(live_bytes),
                budget,
            )
        }
        Source::Definition { head } => {
            let Some(producers) = producers else {
                return Ok(false);
            };
            let Some(reread) = completion::completion(theory, producers, head, live_bytes, budget)?
            else {
                return Ok(false);
            };
            let order = compare_support(support, reread.support, budget)?;
            budget.charge(1)?;
            if order != Ordering::Equal {
                return Ok(false);
            }
            let Some(coefficients) =
                source::local_coefficients(theory, support, &candidate.words, budget)?
            else {
                return Ok(false);
            };
            source::implied_by_table(
                &reread.forbidden,
                support.len,
                coefficients,
                candidate.constant,
                budget,
            )
        }
    }
}

fn retain(
    valid: bool,
    candidate: Equation,
    coefficients: u8,
    equations: &mut Vec<Equation>,
    counts: &mut Counts,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
) -> Result<(), BudgetError> {
    budget.charge(1)?;
    if !valid {
        budget.charge(1)?;
        counts.provenance_rejected += 1;
        storage.release(candidate.words)?;
        debug_assert!(valid, "a proposed equation must follow from its sources");
        return Ok(());
    }
    budget.charge(3)?;
    counts.rows += 1;
    match coefficients.count_ones() {
        0 => {}
        1 => {
            budget.charge(1)?;
            counts.one += 1;
        }
        2 => {
            budget.charge(1)?;
            counts.two += 1;
        }
        _ => {
            budget.charge(1)?;
            counts.wider += 1;
        }
    }
    storage.push(equations, candidate, 7, budget)?;
    storage.embed::<u64>()
}

fn compare_support(
    left: Support,
    right: Support,
    budget: &mut Budget<'_, '_>,
) -> Result<Ordering, BudgetError> {
    for (left, right) in std::iter::once(&left.len)
        .chain(left.atoms.iter())
        .zip(std::iter::once(&right.len).chain(right.atoms.iter()))
    {
        budget.charge(1)?;
        if left < right {
            budget.charge(1)?;
            return Ok(Ordering::Less);
        }
        budget.charge(1)?;
        if left > right {
            budget.charge(1)?;
            return Ok(Ordering::Greater);
        }
    }
    budget.charge(1)?;
    Ok(Ordering::Equal)
}

/// Stable bottom-up merges bound comparisons and their result use by
/// `20Q ceil(log2 Q)`: eighteen scalar tests, a tag and its comparison.
/// Every copied record and both initialized buffers enter work/storage receipts.
fn sort_clauses(
    clauses: &mut Vec<Clause>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
) -> Result<(), BudgetError> {
    budget.charge(1)?;
    if clauses.len() < 2 {
        return Ok(());
    }
    let mut scratch = storage.filled(clauses.len(), clauses[0], 11, budget)?;
    let mut width = 1;
    while width < clauses.len() {
        for first in (0..clauses.len()).step_by(width.saturating_mul(2)) {
            let middle = (first + width).min(clauses.len());
            let end = middle.saturating_add(width).min(clauses.len());
            let (mut left, mut right) = (first, middle);
            for slot in &mut scratch[first..end] {
                let take_left = if right == end {
                    true
                } else if left == middle {
                    false
                } else {
                    let order =
                        compare_support(clauses[left].support, clauses[right].support, budget)?;
                    budget.charge(1)?;
                    order != Ordering::Greater
                };
                budget.charge(11)?;
                if take_left {
                    *slot = clauses[left];
                    left += 1;
                } else {
                    *slot = clauses[right];
                    right += 1;
                }
            }
        }
        budget.charge(6)?;
        std::mem::swap(clauses, &mut scratch);
        width = width.saturating_mul(2);
    }
    storage.release(scratch)
}
