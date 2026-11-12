//! Private image closure borrowing the original preparation's checked bounds.
//! This has no public entry point and installs no production root or receipt.

mod tests;

use super::{
    Attempt, Budget, BudgetError, Cancellation, Limits, Meter, Region, Stop, Storage, Theory,
    copy_region, kernel, prepare,
};
use crate::regions::arithmetic::uniform::BoundSide;
use crate::{CardinalitySystem, ParitySystem, RootPreparation};

/// The seed must already have been transferred out of its preparation owner.
/// Both that owner and the borrowed input remain live throughout this attempt.
pub(super) fn run(
    owner: &Theory,
    prepared: &RootPreparation,
    input: &Region,
    limits: Limits,
    cancellation: &Cancellation,
) -> Attempt {
    let mut meter = Meter::local(limits.global_work);
    let mut budget = Budget::new(&mut meter, limits.work, limits.bytes, cancellation);
    let mut phases = [0; 3];
    let result = (|| {
        budget.poll()?;
        let before = budget.work();
        let validated = identity(owner, prepared, input, &mut budget);
        phases[0] = budget.work() - before;
        validated?;
        let baseline = retained(prepared) + input.retained_bytes();
        budget.admit_bytes(baseline)?;
        let mut storage = Storage::with_live_bytes(baseline);
        let before = budget.work();
        let selected = select(prepared, input.mask_words(), &mut storage, &mut budget);
        phases[0] += budget.work() - before;
        let Some(groups) = selected? else {
            return Ok(None);
        };
        let before = budget.work();
        let indexed = prepare::pairs(owner, &mut storage, &mut budget);
        phases[0] += budget.work() - before;
        let pairs = indexed?;
        let edges: usize = pairs.as_ref().map_or(0, |pairs| {
            (0..owner.atom_count()).map(|atom| pairs[atom].len()).sum()
        });
        assert_eq!(edges % 2, 0);
        let before = budget.work();
        let copied = copy_region(input, &mut storage, &mut budget);
        phases[1] = budget.work() - before;
        let mut region = copied?;
        let before = budget.work();
        let narrowed = kernel::close(
            &groups.iter().copied(),
            pairs.as_ref(),
            &mut region,
            &mut storage,
            &mut budget,
        )
        .and_then(|outcome| {
            storage.release(groups)?;
            crate::regions::arithmetic::images::pairs::release(pairs, &mut storage, &mut budget)?;
            Ok(outcome)
        });
        phases[2] = budget.work() - before;
        let narrowed = narrowed?;
        assert_eq!(storage.bytes(), baseline + region.retained_bytes());
        Ok(Some((region, narrowed)))
    })();
    budget.finish();
    Attempt {
        result,
        preparation: phases[0],
        copying: phases[1],
        closure: phases[2],
        work: budget.work(),
        peak: budget.peak_bytes(),
    }
}

fn identity(
    owner: &Theory,
    prepared: &RootPreparation,
    input: &Region,
    budget: &mut Budget<'_, '_>,
) -> Result<(), BudgetError> {
    budget.charge(1)?;
    let foreign_parity = !prepared.parity().belongs_to(owner);
    budget.charge(1)?;
    let foreign_cardinality = if let Some(cardinality) = prepared.cardinality() {
        budget.charge(1)?;
        !cardinality.belongs_to(owner)
    } else {
        false
    };
    budget.charge(1)?;
    let wrong_size = input.len() != owner.atom_count();
    budget.charge(1)?;
    let untransferred_seed = prepared.seeded_root.is_some();
    budget.charge(3)?;
    if foreign_parity | foreign_cardinality | wrong_size | untransferred_seed {
        return Err(BudgetError::Stopped(Stop::InvalidProgram));
    }
    Ok(())
}

/// The header owns the empty seed slot and each embedded system header once.
/// Capacity reads are an observation, as in the existing retained-byte methods.
fn retained(prepared: &RootPreparation) -> u128 {
    size_of::<RootPreparation>() as u128 + prepared.parity().retained_bytes()
        - size_of::<ParitySystem>() as u128
        + prepared.cardinality().map_or(0, |cardinality| {
            cardinality.retained_bytes() - size_of::<CardinalitySystem>() as u128
        })
}

fn select<'a>(
    prepared: &'a RootPreparation,
    width: usize,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
) -> Result<Option<Vec<&'a [u64]>>, BudgetError> {
    budget.charge(1)?;
    let Some(cardinality) = prepared.cardinality() else {
        return Ok(None);
    };
    let mut groups = storage.reserve::<&[u64], _>(0, budget)?;
    for bound in &cardinality.bounds {
        budget.charge(1)?;
        let lower = bound.side == BoundSide::Lower;
        budget.charge(1)?;
        let mandatory = bound.value >= 1;
        budget.charge(1)?;
        let unguarded = bound.guard.is_empty();
        budget.charge(2)?;
        if lower & mandatory & unguarded {
            budget.charge(1)?; // checked width of the borrowed support prefix
            let support = &bound.support[..width];
            storage.push(&mut groups, support, 2, budget)?;
        }
    }
    budget.charge(1)?;
    if groups.is_empty() {
        storage.release(groups)?;
        return Ok(None);
    }
    Ok(Some(groups))
}
