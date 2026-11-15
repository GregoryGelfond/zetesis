//! One optional transaction at the original `RootPreparation` seed.

use super::super::uniform::BoundSide;
use super::{
    Budget, BudgetError, Narrowing, Quota, Region, RootImageStatistics, RootPreparation, Stop,
    Storage, Theory, kernel, pairs, storage,
};
use zetesis_algebra::reduction::{All, Reduction};

pub(super) fn run<Q: Quota + ?Sized>(
    theory: &Theory,
    prepared: &mut RootPreparation,
    statistics: &mut RootImageStatistics,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    budget.poll()?;
    let baseline = storage::retained(prepared);
    budget.admit_bytes(baseline)?;
    let mut storage = Storage::with_live_bytes(baseline);
    let start = budget.work();
    let selected = select(theory, prepared, &mut storage, budget);
    statistics.preparation_work = budget.work() - start;
    let Some(groups) = selected? else {
        return Ok(false);
    };
    statistics.groups = groups.len();
    let start = budget.work();
    let indexed = pairs::read(theory, &mut storage, budget);
    statistics.preparation_work += budget.work() - start;
    let (pairs, pair_count) = indexed?;
    statistics.pairs = pair_count;
    let start = budget.work();
    let copied = storage::copy_root(
        theory.atom_count(),
        prepared.seeded_root.as_ref(),
        &mut storage,
        budget,
    );
    statistics.copying_work = budget.work() - start;
    let mut region = copied?;
    let start = budget.work();
    let mut counts = kernel::Counts::default();
    let narrowed = kernel::close(
        &groups.iter().copied(),
        pairs.as_ref(),
        &mut region,
        &mut storage,
        budget,
        &mut counts,
    );
    statistics.closure_work = budget.work() - start;
    statistics.completed_rounds = counts.rounds;
    statistics.applied_cuts = counts.cuts;
    let narrowed = narrowed?;
    statistics.root_refuted = narrowed == Narrowing::Refuted;
    storage.release(groups)?;
    let start = budget.work();
    let released = pairs::release(pairs, &mut storage, budget);
    statistics.closure_work += budget.work() - start;
    released?;
    assert_eq!(storage.bytes(), baseline + region.retained_bytes());
    let start = budget.work();
    let published = publish(prepared, region, narrowed, &mut storage, budget);
    statistics.closure_work += budget.work() - start;
    published?;
    assert_eq!(storage.bytes(), storage::retained(prepared));
    Ok(true)
}

fn select<'a, Q: Quota + ?Sized>(
    theory: &Theory,
    prepared: &'a RootPreparation,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<Vec<&'a [u64]>>, BudgetError<Q::Error>> {
    budget.charge(1)?;
    if !prepared.parity.belongs_to(theory) {
        return Err(BudgetError::Stopped(Stop::InvalidProgram));
    }
    budget.charge(1)?;
    let Some(cardinality) = prepared.cardinality.as_ref() else {
        return Ok(None);
    };
    budget.charge(1)?;
    if !cardinality.belongs_to(theory) {
        return Err(BudgetError::Stopped(Stop::InvalidProgram));
    }
    let mut groups = storage.reserve::<&[u64], _>(0, budget)?;
    for bound in &cardinality.bounds {
        budget.charge(1)?;
        let lower = bound.side == BoundSide::Lower;
        budget.charge(1)?;
        let mandatory = bound.value >= 1;
        budget.charge(1)?;
        let unguarded = bound.guard.is_empty();
        budget.charge(2)?;
        if All.combine(All.combine(lower, mandatory), unguarded) {
            budget.charge(1)?;
            let support = &bound.support[..theory.atom_count().div_ceil(64)];
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

fn publish<Q: Quota + ?Sized>(
    prepared: &mut RootPreparation,
    region: Region,
    narrowed: Narrowing,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    budget.charge(1)?; // select the completed result
    match narrowed {
        Narrowing::Fixed { changed: true } => {
            // Two boxed-slice descriptors, atom count, and the Some tag.
            budget.charge(6)?;
            let old = prepared.seeded_root.replace(region);
            let old_payload = old.as_ref().map_or(0, |root| {
                root.retained_bytes() - size_of::<Region>() as u128
            });
            drop(old);
            storage.release_record(old_payload + size_of::<Region>() as u128)?;
        }
        Narrowing::Fixed { changed: false } | Narrowing::Refuted => {
            if narrowed == Narrowing::Refuted {
                budget.charge(1)?;
                prepared.image_refuted = true;
            }
            let bytes = region.retained_bytes();
            drop(region);
            storage.release_record(bytes)?;
        }
    }
    Ok(())
}
