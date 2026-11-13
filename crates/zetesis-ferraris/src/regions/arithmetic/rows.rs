//! Parity proposals accepted only after an independent source check.
//!
//! Clause records are sorted by canonical support with a charged stable merge
//! sort. Definition records name an exact-owner complete producer span. Every
//! proposed equation is checked by rereading its own sources, and only a
//! checked equation is kept.
//!
//! An equation over two or more atoms is materialized in the original atom
//! coordinates and kept as a row for elimination. An equation over one atom
//! decides that atom in every answer set: recognition keeps it as a [`Unit`]
//! and writes no row for it, so a theory's facts cost what they are, one
//! decision each, and not a row of the theory's width each. The inner
//! preparation keeps its one-atom equations as rows, because its rows are
//! read at each region and none is seeded at a root.

use zetesis_algebra::Quota;

use std::cmp::Ordering;
use std::mem::size_of;

use zetesis_algebra::words::Words;

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

/// A source the checker rereads from the formulas: a clause range or a
/// definition. A probed row's source is its refuted probes, checked by its
/// preparation against the retained tables and never reread, so it has no
/// reading here.
#[derive(Clone, Copy)]
enum Reread {
    Clauses { first: usize, count: usize },
    Definition { head: usize },
}

/// What the checker rereads: the checked root facts, the producers when the
/// theory's roots are rules, and the clause records.
struct Sources<'a> {
    facts: &'a CheckedRootFacts,
    producers: Option<&'a Producers>,
    clauses: &'a [Clause],
}

/// An independently checked consequence of the exact owner's answer sets.
/// Row addition is the only subsequent coefficient/constant mutation.
#[derive(Debug)]
pub(super) struct Equation {
    pub words: Words<u64>,
    pub constant: bool,
}

/// A checked equation over one atom: `atom` has `value` in every answer set
/// of the exact owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Unit {
    pub atom: usize,
    pub value: bool,
}

/// What the checker reads of a proposal, which is what recognition keeps of
/// it: its row in the owner's atom coordinates, or the atom of a unit, which
/// has no row. Either is read against the support its source has when read
/// again, so the checker trusts no coordinate the proposal computed.
#[derive(Clone, Copy)]
enum Coefficients<'a> {
    Row(&'a Words<u64>),
    Unit(usize),
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

/// What recognition found in the original theory: its checked rows, and its
/// checked one-atom equations in the order found. Two units may name one
/// atom; elimination keeps one of them, or refutes the root when they
/// disagree. `counts` covers both: a unit is a checked row over one atom.
#[derive(Debug)]
pub(super) struct Recognition {
    pub rows: Recognized,
    pub units: Vec<Unit>,
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

pub(super) fn recognize<Q: Quota + ?Sized>(
    theory: &Theory,
    producers: Option<&Producers>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Recognition, BudgetError<Q::Error>> {
    storage.admit_record(
        (size_of::<Recognized>() - size_of::<CheckedRootFacts>() - 2 * size_of::<Vec<Clause>>())
            as u128,
        budget,
    )?;
    let facts = root_facts::build(theory, storage, budget)?;
    let mut clauses = storage.reserve(0, budget)?;
    let mut equations = storage.reserve(0, budget)?;
    // The units' header is accounted here and passes to the prepared system
    // with them; the recognized record holds none.
    let mut units = storage.reserve(0, budget)?;
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
        (&mut equations, &mut units),
        &mut counts,
        storage,
        budget,
    )?;
    from_definitions(
        &facts,
        producers,
        &clauses,
        (&mut equations, &mut units),
        &mut counts,
        storage,
        budget,
    )?;
    budget.charge(1)?; // publish the successful tag; facts already retain the owner
    Ok(Recognition {
        rows: Recognized {
            facts,
            clauses,
            equations,
            counts,
        },
        units,
    })
}

/// Propose and independently authenticate affine rows from equal-support clauses.
fn from_clauses<Q: Quota + ?Sized>(
    facts: &CheckedRootFacts,
    producers: Option<&Producers>,
    clauses: &[Clause],
    (equations, units): (&mut Vec<Equation>, &mut Vec<Unit>),
    counts: &mut Counts,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
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
        let mut forbidden = Words::<u64, _>::new([0; 4]);
        for clause in &clauses[first..end] {
            budget.charge(4)?;
            forbidden.insert(usize::from(clause.forbidden));
        }
        let proposed = storage.with_record(hull::SCRATCH_BYTES as u128, budget, |_, budget| {
            propose(support, forbidden.storage(), counts, budget)
        })?;
        for &local in proposed.equations() {
            let sources = Sources {
                facts,
                producers,
                clauses,
            };
            let reread = Reread::Clauses {
                first,
                count: end - first,
            };
            if let Some(atom) = sole_atom(support, local, budget)? {
                budget.charge(5)?; // the constant; the reread source's tag, range start and length
                let valid = budget.in_phase(Phase::Provenance, |budget| {
                    storage.with_record(
                        source::CHECK_SCRATCH_BYTES as u128,
                        budget,
                        |storage, budget| {
                            check(
                                &sources,
                                reread,
                                Coefficients::Unit(atom),
                                local.constant,
                                support,
                                storage.bytes(),
                                budget,
                            )
                        },
                    )
                })?;
                let unit = Unit {
                    atom,
                    value: local.constant,
                };
                retain_unit(valid, unit, units, counts, storage, budget)?;
                continue;
            }
            let words = materialize(theory, support, local, storage, budget)?;
            budget.charge(5)?; // the constant; the reread source's tag, range start and length
            let valid = budget.in_phase(Phase::Provenance, |budget| {
                storage.with_record(
                    source::CHECK_SCRATCH_BYTES as u128,
                    budget,
                    |storage, budget| {
                        check(
                            &sources,
                            reread,
                            Coefficients::Row(&words),
                            local.constant,
                            support,
                            storage.bytes(),
                            budget,
                        )
                    },
                )
            })?;
            let candidate = Equation {
                words,
                constant: local.constant,
            };
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
fn from_definitions<Q: Quota + ?Sized>(
    facts: &CheckedRootFacts,
    producers: Option<&Producers>,
    clauses: &[Clause],
    (equations, units): (&mut Vec<Equation>, &mut Vec<Unit>),
    counts: &mut Counts,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
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
                    propose(group.support, group.forbidden.storage(), counts, budget)
                })?;
            for &local in proposed.equations() {
                let sources = Sources {
                    facts,
                    producers: Some(producers),
                    clauses,
                };
                let reread = Reread::Definition { head };
                if let Some(atom) = sole_atom(group.support, local, budget)? {
                    budget.charge(3)?; // the constant; the reread source's tag and head
                    let valid = budget.in_phase(Phase::Provenance, |budget| {
                        check(
                            &sources,
                            reread,
                            Coefficients::Unit(atom),
                            local.constant,
                            group.support,
                            storage.bytes(),
                            budget,
                        )
                    })?;
                    let unit = Unit {
                        atom,
                        value: local.constant,
                    };
                    retain_unit(valid, unit, units, counts, storage, budget)?;
                    continue;
                }
                let words = materialize(theory, group.support, local, storage, budget)?;
                budget.charge(3)?; // the constant; the reread source's tag and head
                let valid = budget.in_phase(Phase::Provenance, |budget| {
                    check(
                        &sources,
                        reread,
                        Coefficients::Row(&words),
                        local.constant,
                        group.support,
                        storage.bytes(),
                        budget,
                    )
                })?;
                let candidate = Equation {
                    words,
                    constant: local.constant,
                };
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

pub(super) fn propose<Q: Quota + ?Sized>(
    support: Support,
    forbidden: &[u64; 4],
    counts: &mut Counts,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<hull::Hull, BudgetError<Q::Error>> {
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

pub(super) fn materialize<Q: Quota + ?Sized>(
    theory: &Theory,
    support: Support,
    local: LocalEquation,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Words<u64>, BudgetError<Q::Error>> {
    let mut words =
        Words::new(storage.filled(theory.atom_count().div_ceil(128) * 2, 0_u64, 1, budget)?);
    for (coordinate, &atom) in support.atoms[..support.len].iter().enumerate() {
        budget.charge(3)?;
        if local.coefficients & (1 << coordinate) != 0 {
            budget.charge(4)?;
            words.insert(atom);
        }
    }
    Ok(words)
}

/// The atom of an equation over exactly one coordinate of `support`.
fn sole_atom<Q: Quota + ?Sized>(
    support: Support,
    local: LocalEquation,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<usize>, BudgetError<Q::Error>> {
    budget.charge(2)?; // population count and its comparison
    if local.coefficients.count_ones() != 1 {
        return Ok(None);
    }
    budget.charge(2)?; // the coordinate and its atom
    let coordinate = local.coefficients.trailing_zeros() as usize;
    Ok(support.atoms[..support.len].get(coordinate).copied())
}

/// Whether an equation over `support` with `coefficients` and `constant`
/// follows from `reread`, read again from the formulas: a clause range's
/// table, or a definition's completion when the theory's roots are rules.
/// A unit's constant is its value.
fn check<Q: Quota + ?Sized>(
    sources: &Sources<'_>,
    reread: Reread,
    coefficients: Coefficients<'_>,
    constant: bool,
    support: Support,
    live_bytes: u128,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    let theory = sources.facts.owner();
    budget.charge(1)?;
    match reread {
        Reread::Clauses { first, count } => {
            let end = first
                .checked_add(count)
                .ok_or(BudgetError::Stopped(zetesis_algebra::Stop::InvalidProgram))?;
            let roots = sources
                .clauses
                .get(first..end)
                .ok_or(BudgetError::Stopped(zetesis_algebra::Stop::InvalidProgram))?;
            let mut storage = Storage::with_live_bytes(live_bytes);
            match coefficients {
                Coefficients::Row(words) => source::check_clause_equation(
                    theory,
                    sources.facts,
                    roots,
                    support,
                    (words, constant),
                    &mut storage,
                    budget,
                ),
                Coefficients::Unit(atom) => source::check_clause_unit(
                    theory,
                    sources.facts,
                    roots,
                    support,
                    (atom, constant),
                    &mut storage,
                    budget,
                ),
            }
        }
        Reread::Definition { head } => {
            let Some(producers) = sources.producers else {
                return Ok(false);
            };
            let Some(definition) =
                completion::completion(theory, producers, head, live_bytes, budget)?
            else {
                return Ok(false);
            };
            let order = compare_support(support, definition.support, budget)?;
            budget.charge(1)?;
            if order != Ordering::Equal {
                return Ok(false);
            }
            let coefficients = match coefficients {
                Coefficients::Row(words) => {
                    let Some(local) = source::local_coefficients(theory, support, words, budget)?
                    else {
                        return Ok(false);
                    };
                    local
                }
                Coefficients::Unit(atom) => {
                    let Some(local) = source::unit_coefficients(support, atom, budget)? else {
                        return Ok(false);
                    };
                    local
                }
            };
            source::implied_by_table(
                definition.forbidden.storage(),
                support.len,
                coefficients,
                constant,
                budget,
            )
        }
    }
}

pub(super) fn retain<Q: Quota + ?Sized>(
    valid: bool,
    candidate: Equation,
    coefficients: u8,
    equations: &mut Vec<Equation>,
    counts: &mut Counts,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    budget.charge(1)?;
    if !valid {
        budget.charge(1)?;
        counts.provenance_rejected += 1;
        storage.release(candidate.words.into_storage())?;
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
    storage.embed::<u64>().map_err(BudgetError::Stopped)
}

/// Keep a checked one-atom equation as a unit. It counts as a checked row
/// over one atom, as [`retain`] counts one.
fn retain_unit<Q: Quota + ?Sized>(
    valid: bool,
    unit: Unit,
    units: &mut Vec<Unit>,
    counts: &mut Counts,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    budget.charge(1)?;
    if !valid {
        budget.charge(1)?;
        counts.provenance_rejected += 1;
        debug_assert!(valid, "a proposed equation must follow from its sources");
        return Ok(());
    }
    budget.charge(4)?; // the row counter; the width's test and its counter
    counts.rows += 1;
    counts.one += 1;
    storage.push(units, unit, 2, budget)
}

fn compare_support<Q: Quota + ?Sized>(
    left: Support,
    right: Support,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Ordering, BudgetError<Q::Error>> {
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
fn sort_clauses<Q: Quota + ?Sized>(
    clauses: &mut Vec<Clause>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
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
    storage.release(scratch).map_err(BudgetError::Stopped)
}
