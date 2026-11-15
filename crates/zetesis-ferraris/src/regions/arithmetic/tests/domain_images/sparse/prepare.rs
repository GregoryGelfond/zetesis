//! Original-source authentication for the private sparse image experiment.

use super::{Adjacency, Budget, BudgetError, Sparse, Stop, Storage, Theory, Words};
use crate::regions::arithmetic::{cardinality_source, uniform::BoundSide};

pub(super) fn read(
    theory: &Theory,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
) -> Result<Option<Sparse>, BudgetError> {
    let mut groups = storage.reserve::<Words<u64>, _>(0, budget)?;
    for ordinal in 0..theory.roots().len() {
        let mut proposals = cardinality_source::read_root(theory, ordinal, storage, budget)?;
        for bound in proposals.drain(..) {
            let accepted = cardinality_source::check(theory, theory, &bound, storage, budget)?;
            budget.charge(1)?;
            if !accepted {
                return Err(BudgetError::Stopped(Stop::InvalidProgram));
            }
            budget.charge(1)?;
            let lower = bound.side == BoundSide::Lower;
            budget.charge(1)?;
            let mandatory = bound.value >= 1;
            budget.charge(1)?;
            let unguarded = bound.guard.is_empty();
            budget.charge(2)?;
            if lower & mandatory & unguarded {
                let width = theory.atom_count().div_ceil(64);
                let mut values = storage.reserve::<u64, _>(width, budget)?;
                for &word in &bound.support[..width] {
                    // Copy one initialized word and advance this reserved vector's length.
                    budget.charge(2)?;
                    values.push(word);
                }
                storage.push(&mut groups, Words::new(values), 3, budget)?;
                storage.embed::<u64>()?;
            }
            cardinality_source::release(bound, storage)?;
        }
        storage.release(proposals)?;
    }
    budget.charge(1)?;
    if groups.is_empty() {
        storage.release(groups)?;
        budget.charge(1)?;
        return Ok(None);
    }
    let forbidden = pairs(theory, storage, budget)?;
    budget.charge(1)?;
    Ok(Some(Sparse { groups, forbidden }))
}

pub(super) fn pairs(
    theory: &Theory,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
) -> Result<Option<Adjacency>, BudgetError> {
    crate::regions::arithmetic::images::pairs::read(theory, storage, budget).map(|(pairs, _)| pairs)
}
