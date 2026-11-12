//! Existing packed, adjacency and named-storage adapters, with one quota owner.

use super::super::{CardinalitySystem, ParitySystem};
use super::{
    Budget, BudgetError, Construction, Quota, Region, RootPreparation, Storage, Words, words,
};

pub(super) fn retained(prepared: &RootPreparation) -> u128 {
    size_of::<RootPreparation>() as u128 + prepared.parity.retained_bytes()
        - size_of::<ParitySystem>() as u128
        + prepared.cardinality.as_ref().map_or(0, |system| {
            system.retained_bytes() - size_of::<CardinalitySystem>() as u128
        })
        + prepared.seeded_root.as_ref().map_or(0, |root| {
            root.retained_bytes() - size_of::<Region>() as u128
        })
}

pub(super) fn copy_root<Q: Quota + ?Sized>(
    atoms: usize,
    input: Option<&Region>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Region, BudgetError<Q::Error>> {
    let baseline = storage.bytes();
    let mut region = Region::try_all_open_with(
        atoms,
        budget,
        &mut |budget, work| budget.charge(work),
        &mut |budget, bytes| budget.admit_bytes(baseline + bytes),
    )?;
    storage.admit_record(region.retained_bytes(), budget)?;
    budget.charge(1)?; // inspect the optional already-seeded input
    if let Some(input) = input {
        copy_decisions(input, &mut region, budget)?;
    }
    Ok(region)
}

fn copy_decisions<Q: Quota + ?Sized>(
    input: &Region,
    region: &mut Region,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    let (held, cut) = input.decision_words();
    for (mask, value) in [(held, true), (cut, false)] {
        words::visit_set_bits(
            mask,
            &mut (&mut *budget, &mut *region),
            &mut |(budget, _), work| budget.charge(work),
            &mut |(budget, region), atom| {
                budget.charge(12)?;
                let applied = if value {
                    region.hold(atom)
                } else {
                    region.cut(atom)
                };
                assert!(applied, "a copied disjoint decision selects an open atom");
                Ok(())
            },
        )?;
    }
    Ok(())
}

pub(in crate::regions::arithmetic) fn contains<Q: Quota + ?Sized>(
    mask: &[u64],
    atom: usize,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    budget.charge(5)?;
    Ok(Words::new(mask).contains(atom))
}

pub(in crate::regions::arithmetic) struct IndexConstruction<'a, 'b, 'q, Q: Quota + ?Sized> {
    pub storage: &'a mut Storage,
    pub budget: &'a mut Budget<'b, 'q, Q>,
}

impl<Q: Quota + ?Sized> Construction for IndexConstruction<'_, '_, '_, Q> {
    type Error = BudgetError<Q::Error>;
    fn tick(&mut self) -> Result<(), Self::Error> {
        self.budget.charge(1)
    }
    fn zeros(&mut self, count: usize) -> Result<Vec<usize>, Self::Error> {
        self.storage.filled(count, 0, 1, self.budget)
    }
    fn reserve(&mut self, count: usize) -> Result<Vec<usize>, Self::Error> {
        self.storage.reserve(count, self.budget)
    }
    fn release(&mut self, values: Vec<usize>) -> Result<(), Self::Error> {
        self.storage.release(values).map_err(BudgetError::Stopped)
    }
}
