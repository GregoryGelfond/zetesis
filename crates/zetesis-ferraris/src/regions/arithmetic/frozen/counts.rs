//! Count bounds of the countermodel class from the probed tables and the
//! clauses too wide for a group.
//!
//! A table's least and greatest allowed counts bound its support
//! (`bounds_of_allowed`), the least one when the empty pattern is forbidden,
//! a clause's (`lower_of_clause`), the greatest one when it is two or more
//! and below the support's size; and the pairs no allowed pattern holds
//! together, read from every table, grow into cliques, each an upper bound
//! of one (`upper_of_clique`), which carry a greatest count of one, as the
//! hull's unit rows carry one of nothing. A clause too wide for a group
//! whose pattern cuts its whole support bounds that support from below by
//! one with no table (`lower_of_clause`): no countermodel in the root cuts
//! it whole, by the static refutations' argument. Atoms one side covers and
//! the other does not get the other side's trivial bound, nothing at least
//! or everything at most, so that the families partition one atom set both
//! ways. The bounds carry no guard: a table's guard is cut at the closed
//! root, by a saturation unit or already, which makes the bound a row of the
//! class as the parity rows are.

use zetesis_algebra::{
    Quota, Stop,
    words::{self, Words},
};

use super::super::{
    CardinalityStatistics,
    budget::{Budget, BudgetError},
    cardinality::sort,
    cardinality_source::{Bound, Kind, Source, push_bound},
    source::MAX_SUPPORT,
    storage::Storage,
    uniform::BoundSide,
};
use super::{ProbedGroup, probes::Table, proposals::Wide};
use crate::Theory;

/// The bounds the tables establish, the tables' first, the cliques' after
/// and the completing trivial bounds last, counted into `statistics` as
/// lower and upper bounds.
pub(super) fn propose<Q: Quota + ?Sized>(
    theory: &Theory,
    tables: &[ProbedGroup],
    wide: &[&Wide],
    statistics: &mut CardinalityStatistics,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Vec<Bound>, BudgetError<Q::Error>> {
    let width = theory.atom_count().div_ceil(128) * 2;
    let mut bounds = storage.reserve(0, budget)?;
    let mut exclusions: Vec<(usize, usize)> = storage.reserve(0, budget)?;
    for (group, table) in tables.iter().enumerate() {
        let atoms = &table.support.atoms[..table.support.len];
        storage.admit_record(size_of::<Counted>() as u128, budget)?;
        let Some(Counted {
            least,
            greatest,
            together,
        }) = counts(atoms.len(), &table.forbidden, budget)?
        else {
            storage.release_record(size_of::<Counted>() as u128)?;
            continue;
        };
        if least >= 1 {
            let bound = bound(
                (BoundSide::Lower, least),
                atoms.iter().copied(),
                (group, 0),
                width,
                storage,
                budget,
            )?;
            push_bound(&mut bounds, bound, storage, budget)?;
            statistics.lower += 1;
        }
        budget.charge(1)?;
        // A greatest count below two is carried elsewhere: of one, every pair
        // of the support is an exclusion, so the cliques bound the support,
        // or a superset of it, by one; of nothing, the one allowed pattern's
        // hull is the unit rows cutting every atom. The table's own bound
        // would duplicate them.
        if greatest >= 2 && greatest < atoms.len() as u64 {
            let bound = bound(
                (BoundSide::Upper, greatest),
                atoms.iter().copied(),
                (group, 1),
                width,
                storage,
                budget,
            )?;
            push_bound(&mut bounds, bound, storage, budget)?;
            statistics.upper += 1;
        }
        for first in 0..atoms.len() {
            for second in first + 1..atoms.len() {
                budget.charge(1)?;
                if together[first] & (1 << second) == 0 {
                    storage.push(&mut exclusions, (atoms[first], atoms[second]), 1, budget)?;
                    storage.push(&mut exclusions, (atoms[second], atoms[first]), 1, budget)?;
                }
            }
        }
        storage.release_record(size_of::<Counted>() as u128)?;
    }
    // A clause too wide for a group, its pattern cutting its whole support:
    // no countermodel in the root cuts every atom of it (`lower_of_clause`).
    for (index, clause) in wide.iter().enumerate() {
        budget.charge(1)?;
        let bound = bound(
            (BoundSide::Lower, 1),
            clause.support.iter().copied(),
            (tables.len() + index, 2),
            width,
            storage,
            budget,
        )?;
        push_bound(&mut bounds, bound, storage, budget)?;
        statistics.lower += 1;
    }
    cliques(
        &mut exclusions,
        &mut bounds,
        statistics,
        width,
        storage,
        budget,
    )?;
    storage.release(exclusions)?;
    complete(theory, &mut bounds, statistics, width, storage, budget)?;
    Ok(bounds)
}

/// The trivial bounds completing each side's cover of the other's atoms: a
/// lower bound of nothing over the atoms only upper bounds cover, an upper
/// bound of everything over the atoms only lower bounds cover.
fn complete<Q: Quota + ?Sized>(
    theory: &Theory,
    bounds: &mut Vec<Bound>,
    statistics: &mut CardinalityStatistics,
    width: usize,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    let mut lower = Words::new(storage.filled(width, 0_u64, 1, budget)?);
    let mut upper = Words::new(storage.filled(width, 0_u64, 1, budget)?);
    for bound in bounds.iter() {
        let side = match bound.side {
            BoundSide::Lower => &mut lower,
            BoundSide::Upper => &mut upper,
        };
        words::union_assign(side.storage_mut(), &bound.support, &mut |work| {
            budget.charge(work)
        })?;
    }
    for (side, covered, other) in [
        (BoundSide::Lower, &lower, &upper),
        (BoundSide::Upper, &upper, &lower),
    ] {
        // Two passes over the atoms, each charged: the count names the
        // bound's value, the second pass its support.
        let missing = || {
            (0..theory.atom_count()).filter(|&atom| other.contains(atom) && !covered.contains(atom))
        };
        budget.charge(theory.atom_count() as u64)?;
        let count = missing().count();
        if count == 0 {
            continue;
        }
        let value = match side {
            BoundSide::Lower => 0,
            BoundSide::Upper => count as u64,
        };
        budget.charge(theory.atom_count() as u64)?;
        let bound = bound(
            (side, value),
            missing(),
            (bounds.len(), 0),
            width,
            storage,
            budget,
        )?;
        push_bound(bounds, bound, storage, budget)?;
        match side {
            BoundSide::Lower => statistics.lower += 1,
            BoundSide::Upper => statistics.upper += 1,
        }
    }
    storage.release(lower.into_storage())?;
    storage.release(upper.into_storage())?;
    Ok(())
}

/// What a table's allowed patterns count: the least and greatest held, and
/// per position the positions some allowed pattern holds with it.
struct Counted {
    least: u64,
    greatest: u64,
    together: [u8; MAX_SUPPORT],
}

/// The counts of a table's allowed patterns; `None` when no pattern is
/// allowed, which the parity rows refute.
fn counts<Q: Quota + ?Sized>(
    len: usize,
    forbidden: &Table,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<Counted>, BudgetError<Q::Error>> {
    let mut least = u64::MAX;
    let mut greatest = 0;
    let mut together = [0_u8; MAX_SUPPORT];
    for pattern in 0..(1_usize << len) {
        budget.charge(1)?;
        if forbidden.contains(pattern) {
            continue;
        }
        let count = u64::from(pattern.count_ones());
        least = least.min(count);
        greatest = greatest.max(count);
        let held = u8::try_from(pattern).map_err(|_| BudgetError::Stopped(Stop::InvalidProgram))?;
        for position in (0..len).filter(|&position| held & (1 << position) != 0) {
            together[position] |= held;
        }
    }
    Ok((least != u64::MAX).then_some(Counted {
        least,
        greatest,
        together,
    }))
}

/// A bound over `atoms`, unguarded, with its probed source.
fn bound<Q: Quota + ?Sized>(
    (side, value): (BoundSide, u64),
    atoms: impl Iterator<Item = usize>,
    (root, node): (usize, usize),
    width: usize,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Bound, BudgetError<Q::Error>> {
    let mut support = Words::new(storage.filled(width, 0_u64, 1, budget)?);
    let mut cardinality = 0;
    for atom in atoms {
        budget.charge(1)?;
        support.insert(atom);
        cardinality += 1;
    }
    let guard = storage.reserve(0, budget)?;
    Ok(Bound {
        support,
        guard,
        side,
        value,
        cardinality,
        source: Source {
            kind: Kind::Probed,
            root,
            node,
        },
    })
}

/// The exclusions, both ways round and sorted, grown into cliques: each pair
/// seeds one, joined in turn by every excluded neighbour of its first atom
/// that every member excludes, so a hole's pairs grow into the hole's clique
/// and a pigeon's own into the pigeon's. Each clique found once is an upper
/// bound of one.
fn cliques<Q: Quota + ?Sized>(
    edges: &mut Vec<(usize, usize)>,
    bounds: &mut Vec<Bound>,
    statistics: &mut CardinalityStatistics,
    width: usize,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    let mut scratch = storage.filled(edges.len(), (0, 0), 2, budget)?;
    sort(
        edges,
        &mut scratch,
        2,
        |a, b, budget| {
            budget.charge(1)?;
            Ok(a.cmp(b))
        },
        budget,
    )?;
    storage.release(scratch)?;
    budget.charge(edges.len() as u64)?;
    edges.dedup();
    let mut found: Vec<Vec<usize>> = storage.reserve(0, budget)?;
    let mut start = 0;
    while start < edges.len() {
        let atom = edges[start].0;
        let mut end = start;
        while end < edges.len() && edges[end].0 == atom {
            budget.charge(1)?;
            end += 1;
        }
        let neighbours = &edges[start..end];
        for &(_, seed) in neighbours {
            budget.charge(1)?;
            if seed < atom {
                continue;
            }
            let mut clique: Vec<usize> = storage.reserve(2, budget)?;
            storage.push(&mut clique, atom, 1, budget)?;
            storage.push(&mut clique, seed, 1, budget)?;
            for &(_, next) in neighbours {
                budget.charge(1)?;
                if next == seed {
                    continue;
                }
                let mut excluded = true;
                for &member in &clique {
                    budget.charge(1)?;
                    if edges.binary_search(&(member, next)).is_err() {
                        excluded = false;
                        break;
                    }
                }
                if excluded {
                    storage.push(&mut clique, next, 1, budget)?;
                }
            }
            let mut scratch = storage.filled(clique.len(), 0, 1, budget)?;
            sort(
                &mut clique,
                &mut scratch,
                1,
                |a, b, budget| {
                    budget.charge(1)?;
                    Ok(a.cmp(b))
                },
                budget,
            )?;
            storage.release(scratch)?;
            let mut known = false;
            for earlier in &found {
                budget.charge(earlier.len() as u64)?;
                if *earlier == clique {
                    known = true;
                    break;
                }
            }
            if known {
                storage.release(clique)?;
                continue;
            }
            let bound = bound(
                (BoundSide::Upper, 1),
                clique.iter().copied(),
                (clique[0], clique[1]),
                width,
                storage,
                budget,
            )?;
            push_bound(bounds, bound, storage, budget)?;
            statistics.upper += 1;
            storage.push(&mut found, clique, 3, budget)?;
            storage.embed::<usize>()?;
        }
        start = end;
    }
    for clique in found.drain(..) {
        storage.release_embedded(clique)?;
    }
    storage.release(found)?;
    Ok(())
}
