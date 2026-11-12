//! Count bounds of the countermodel class from the probed tables and the
//! clauses too wide for a group.
//!
//! A table's least and greatest allowed counts bound its support
//! (`bounds_of_allowed`), the least one when the empty pattern is forbidden,
//! a clause's (`lower_of_clause`), the greatest one when it is two or more
//! and below the support's size; and the pairs no allowed pattern holds
//! together, read from every table, grow into cliques, each an upper bound
//! of one (`upper_of_clique`), which carry a greatest count of one, as the
//! hull's unit rows carry one of nothing. Dually, the pairs no allowed
//! pattern cuts together grow into cliques of which at most one atom is cut,
//! each a lower bound of its size less one (`lower_of_cover`). Which of the
//! two a pairwise clause yields depends only on the polarity its base atoms
//! were read in, so both are taken; a pair that only forbids holding one
//! atom while cutting the other is neither, and grows no clique: these bounds
//! count held atoms, not signed occurrences. A table keeps its own least count beside
//! the covering cliques: a clique grown past a table's support no longer
//! partitions the atoms as that support does. A clause too wide for a group
//! whose pattern cuts its whole support bounds that support from below by
//! one with no table (`lower_of_clause`): no countermodel in the root cuts
//! it whole, by the static refutations' argument. Atoms one side covers and
//! the other does not get the other side's trivial bound, nothing at least
//! or everything at most, so that the families partition one atom set both
//! ways. The bounds carry no guard: a table's guard is cut at the closed
//! root, by a saturation unit or already, which makes the bound a row of the
//! class as the parity rows are.

use std::cmp::Ordering;

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

/// The bounds proposed so far, at the support width they are made with and
/// counted by side into the preparation's statistics.
struct Proposed<'a> {
    bounds: Vec<Bound>,
    statistics: &'a mut CardinalityStatistics,
    width: usize,
}

impl Proposed<'_> {
    /// Add an unguarded bound over `atoms` and count its side.
    fn push<Q: Quota + ?Sized>(
        &mut self,
        (side, value): (BoundSide, u64),
        atoms: impl Iterator<Item = usize>,
        source: (usize, usize),
        storage: &mut Storage,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<(), BudgetError<Q::Error>> {
        let bound = bound((side, value), atoms, source, self.width, storage, budget)?;
        push_bound(&mut self.bounds, bound, storage, budget)?;
        match side {
            BoundSide::Lower => self.statistics.lower += 1,
            BoundSide::Upper => self.statistics.upper += 1,
        }
        Ok(())
    }
}

/// The pairs the probed tables establish, each both ways round.
struct TablePairs {
    /// Pairs no allowed pattern holds together.
    excluded: Vec<(usize, usize)>,
    /// Pairs no allowed pattern cuts together.
    covered: Vec<(usize, usize)>,
    /// The two-atom supports whose own least count already states their
    /// pair's covering bound: a clique of just that pair would repeat it.
    stated: Vec<(usize, usize)>,
}

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
    let mut proposed = Proposed {
        bounds: storage.reserve(0, budget)?,
        statistics,
        width: theory.atom_count().div_ceil(128) * 2,
    };
    let mut pairs = TablePairs {
        excluded: storage.reserve(0, budget)?,
        covered: storage.reserve(0, budget)?,
        stated: storage.reserve(0, budget)?,
    };
    for (group, table) in tables.iter().enumerate() {
        table_bounds((group, table), &mut pairs, &mut proposed, storage, budget)?;
    }
    // A clause too wide for a group, its pattern cutting its whole support:
    // no countermodel in the root cuts every atom of it (`lower_of_clause`).
    for (index, clause) in wide.iter().enumerate() {
        budget.charge(1)?;
        proposed.push(
            (BoundSide::Lower, 1),
            clause.support.iter().copied(),
            (tables.len() + index, 2),
            storage,
            budget,
        )?;
    }
    clique_bounds(pairs, &mut proposed, storage, budget)?;
    let Proposed {
        mut bounds,
        statistics,
        width,
    } = proposed;
    complete(theory, &mut bounds, statistics, width, storage, budget)?;
    Ok(bounds)
}

/// One table's own bounds and pairs: its least allowed count from below, its
/// greatest from above where no clique carries it, and the pairs its allowed
/// patterns never hold together or never cut together.
fn table_bounds<Q: Quota + ?Sized>(
    (group, table): (usize, &ProbedGroup),
    pairs: &mut TablePairs,
    proposed: &mut Proposed<'_>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    let atoms = &table.support.atoms[..table.support.len];
    storage.admit_record(size_of::<Counted>() as u128, budget)?;
    let Some(Counted {
        least,
        greatest,
        together,
        apart,
    }) = counts(atoms.len(), &table.forbidden, budget)?
    else {
        storage.release_record(size_of::<Counted>() as u128)?;
        return Ok(());
    };
    if least >= 1 {
        proposed.push(
            (BoundSide::Lower, least),
            atoms.iter().copied(),
            (group, 0),
            storage,
            budget,
        )?;
        budget.charge(1)?;
        if let [first, second] = *atoms {
            let pair = (first.min(second), first.max(second));
            storage.push(&mut pairs.stated, pair, 1, budget)?;
        }
    }
    budget.charge(1)?;
    // A greatest count below two is carried elsewhere: of one, every pair
    // of the support is an exclusion, so the cliques bound the support,
    // or a superset of it, by one; of nothing, the one allowed pattern's
    // hull is the unit rows cutting every atom. The table's own bound
    // would duplicate them.
    if greatest >= 2 && greatest < atoms.len() as u64 {
        proposed.push(
            (BoundSide::Upper, greatest),
            atoms.iter().copied(),
            (group, 1),
            storage,
            budget,
        )?;
    }
    for first in 0..atoms.len() {
        for second in first + 1..atoms.len() {
            budget.charge(1)?;
            if together[first] & (1 << second) == 0 {
                storage.push(
                    &mut pairs.excluded,
                    (atoms[first], atoms[second]),
                    1,
                    budget,
                )?;
                storage.push(
                    &mut pairs.excluded,
                    (atoms[second], atoms[first]),
                    1,
                    budget,
                )?;
            }
            budget.charge(1)?;
            if apart[first] & (1 << second) == 0 {
                storage.push(&mut pairs.covered, (atoms[first], atoms[second]), 1, budget)?;
                storage.push(&mut pairs.covered, (atoms[second], atoms[first]), 1, budget)?;
            }
        }
    }
    storage.release_record(size_of::<Counted>() as u128)?;
    Ok(())
}

/// The cliques of the tables' pairs: the excluding pairs' first, each an
/// upper bound, then the covering pairs', each a lower bound.
fn clique_bounds<Q: Quota + ?Sized>(
    pairs: TablePairs,
    proposed: &mut Proposed<'_>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    let TablePairs {
        mut excluded,
        mut covered,
        mut stated,
    } = pairs;
    cliques(&mut excluded, Pairs::Excluded, proposed, storage, budget)?;
    storage.release(excluded)?;
    let mut scratch = storage.filled(stated.len(), (0, 0), 2, budget)?;
    sort(&mut stated, &mut scratch, 2, ordered, budget)?;
    storage.release(scratch)?;
    cliques(
        &mut covered,
        Pairs::Covered { stated: &stated },
        proposed,
        storage,
        budget,
    )?;
    storage.release(covered)?;
    storage.release(stated)?;
    Ok(())
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
/// per position the positions some allowed pattern holds with it and those
/// some allowed pattern cuts with it.
struct Counted {
    least: u64,
    greatest: u64,
    together: [u8; MAX_SUPPORT],
    apart: [u8; MAX_SUPPORT],
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
    let mut apart = [0_u8; MAX_SUPPORT];
    let support = u8::try_from((1_usize << len) - 1)
        .map_err(|_| BudgetError::Stopped(Stop::InvalidProgram))?;
    for pattern in 0..(1_usize << len) {
        budget.charge(1)?;
        if forbidden.contains(pattern) {
            continue;
        }
        let count = u64::from(pattern.count_ones());
        least = least.min(count);
        greatest = greatest.max(count);
        let held = u8::try_from(pattern).map_err(|_| BudgetError::Stopped(Stop::InvalidProgram))?;
        let cut = !held & support;
        for position in 0..len {
            if held & (1 << position) == 0 {
                apart[position] |= cut;
            } else {
                together[position] |= held;
            }
        }
    }
    Ok((least != u64::MAX).then_some(Counted {
        least,
        greatest,
        together,
        apart,
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

/// What every pair of a clique's atoms establishes of the class.
#[derive(Clone, Copy)]
enum Pairs<'a> {
    /// No member holds both atoms: at most one atom of the clique is held.
    Excluded,
    /// No member cuts both atoms: at most one atom of the clique is cut. A
    /// clique of one pair among `stated`, the sorted two-atom supports whose
    /// own bound says as much, is not repeated.
    Covered { stated: &'a [(usize, usize)] },
}

/// The charged natural order of two records.
fn ordered<T: Ord, Q: Quota + ?Sized>(
    left: &T,
    right: &T,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Ordering, BudgetError<Q::Error>> {
    budget.charge(1)?;
    Ok(left.cmp(right))
}

/// The pairs, both ways round and sorted, grown into cliques: each pair
/// seeds one, so a hole's pairs grow into the hole's clique and a pigeon's
/// own into the pigeon's. Each clique found once is an upper bound of one
/// when its pairs exclude, and a lower bound of its size less one when they
/// cover.
fn cliques<Q: Quota + ?Sized>(
    edges: &mut Vec<(usize, usize)>,
    pairs: Pairs<'_>,
    proposed: &mut Proposed<'_>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    let mut scratch = storage.filled(edges.len(), (0, 0), 2, budget)?;
    sort(edges, &mut scratch, 2, ordered, budget)?;
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
            let clique = grow(edges, neighbours, (atom, seed), storage, budget)?;
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
            let side = match pairs {
                Pairs::Excluded => (BoundSide::Upper, 1),
                Pairs::Covered { stated } => {
                    budget.charge(1)?;
                    if clique.len() == 2 && stated.binary_search(&(clique[0], clique[1])).is_ok() {
                        storage.release(clique)?;
                        continue;
                    }
                    (BoundSide::Lower, clique.len() as u64 - 1)
                }
            };
            proposed.push(
                side,
                clique.iter().copied(),
                (clique[0], clique[1]),
                storage,
                budget,
            )?;
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

/// The clique seeded by the pair of `atom` and `seed`, in ascending order:
/// the pair, joined in turn by every neighbour of `atom` that every member
/// so far is paired with.
fn grow<Q: Quota + ?Sized>(
    edges: &[(usize, usize)],
    neighbours: &[(usize, usize)],
    (atom, seed): (usize, usize),
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Vec<usize>, BudgetError<Q::Error>> {
    let mut clique: Vec<usize> = storage.reserve(2, budget)?;
    storage.push(&mut clique, atom, 1, budget)?;
    storage.push(&mut clique, seed, 1, budget)?;
    for &(_, next) in neighbours {
        budget.charge(1)?;
        if next == seed {
            continue;
        }
        let mut paired = true;
        for &member in &clique {
            budget.charge(1)?;
            if edges.binary_search(&(member, next)).is_err() {
                paired = false;
                break;
            }
        }
        if paired {
            storage.push(&mut clique, next, 1, budget)?;
        }
    }
    let mut scratch = storage.filled(clique.len(), 0, 1, budget)?;
    sort(&mut clique, &mut scratch, 1, ordered, budget)?;
    storage.release(scratch)?;
    Ok(clique)
}
