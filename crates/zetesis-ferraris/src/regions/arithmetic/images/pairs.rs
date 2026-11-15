//! Independently reread original forbidden-pair evidence and its sparse index.

use super::super::source;
use super::{Adjacency, Budget, BudgetError, IndexConstruction, Quota, Stop, Storage, Theory};
use zetesis_algebra::reduction::{Any, Reduction};

#[derive(Clone, Copy)]
struct Pair {
    left: usize,
    right: usize,
}

pub(in crate::regions::arithmetic) fn read<Q: Quota + ?Sized>(
    theory: &Theory,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(Option<Adjacency>, usize), BudgetError<Q::Error>> {
    let mut pairs = storage.reserve::<Pair, _>(0, budget)?;
    for ordinal in 0..theory.roots().len() {
        if let Some(pair) = pair(theory, ordinal, storage, budget)? {
            storage.push(&mut pairs, pair, 2, budget)?;
        }
    }
    budget.charge(1)?; // inspect the complete authenticated pair population
    if pairs.is_empty() {
        storage.release(pairs)?;
        // None owns its representation even though it has no index buffers.
        storage.admit_record(size_of::<Option<Adjacency>>() as u128, budget)?;
        budget.charge(1)?; // publish the empty-index tag
        return Ok((None, 0));
    }
    let edges = pairs.iter().flat_map(|pair| {
        [
            Ok(Some((pair.left, pair.right))),
            Ok(Some((pair.right, pair.left))),
        ]
    });
    let forbidden = Adjacency::try_build_in(
        theory.atom_count(),
        edges,
        &mut IndexConstruction { storage, budget },
    )?;
    let count = pairs.len();
    storage.release(pairs)?;
    // The two vector headers already own Adjacency. Account only any extra
    // metadata that Option adds, rather than counting those headers twice.
    storage.admit_record(index_tag_bytes(), budget)?;
    budget.charge(1)?;
    Ok((Some(forbidden), count))
}

const fn index_tag_bytes() -> u128 {
    (size_of::<Option<Adjacency>>() - size_of::<Adjacency>()) as u128
}

/// Release the exact owner returned by the completed pair scan.
pub(in crate::regions::arithmetic) fn release<Q: Quota + ?Sized>(
    pairs: Option<Adjacency>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    budget.charge(1)?;
    match pairs {
        Some(pairs) => {
            pairs.release_in(&mut IndexConstruction { storage, budget })?;
            storage.release_record(index_tag_bytes())?;
        }
        None => storage.release_record(size_of::<Option<Adjacency>>() as u128)?,
    }
    Ok(())
}

fn pair<Q: Quota + ?Sized>(
    theory: &Theory,
    ordinal: usize,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<Pair>, BudgetError<Q::Error>> {
    // One retained proposal exists alongside the independently reconstructed
    // clause and its literal scratch. The original source is read both times.
    storage.with_record(
        size_of::<source::Clause>() as u128,
        budget,
        |storage, budget| {
            let proposed = storage.with_record(
                source::CLAUSE_SCRATCH_BYTES as u128,
                budget,
                |storage, budget| source::positive_pair_clause(theory, ordinal, storage, budget),
            )?;
            let Some(proposed) = proposed else {
                return Ok(None);
            };
            budget.charge(3)?;
            if Any.combine(proposed.support.len != 2, proposed.forbidden != 0b11) {
                return Ok(None);
            }
            let checked = storage.with_record(
                source::CLAUSE_SCRATCH_BYTES as u128,
                budget,
                |storage, budget| source::clause(theory, proposed.root, storage, budget),
            )?;
            let Some(checked) = checked else {
                return Err(BudgetError::Stopped(Stop::InvalidProgram));
            };
            budget.charge(5)?;
            if Any.combine(
                Any.combine(
                    proposed.root != checked.root,
                    proposed.support.len != checked.support.len,
                ),
                proposed.forbidden != checked.forbidden,
            ) {
                return Err(BudgetError::Stopped(Stop::InvalidProgram));
            }
            for (&left, &right) in proposed.support.atoms.iter().zip(&checked.support.atoms) {
                budget.charge(1)?;
                if left != right {
                    return Err(BudgetError::Stopped(Stop::InvalidProgram));
                }
            }
            budget.charge(3)?;
            Ok(Some(Pair {
                left: proposed.support.atoms[0],
                right: proposed.support.atoms[1],
            }))
        },
    )
}
