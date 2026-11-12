//! Sparse pivot closure over authenticated mandatory groups and forbidden pairs.

use super::{
    Adjacency, Budget, BudgetError, Narrowing, Quota, Region, Storage, Words, contains, words,
};

#[derive(Default)]
pub(in crate::regions::arithmetic) struct Counts {
    pub rounds: u64,
    pub cuts: u64,
}

pub(in crate::regions::arithmetic) fn close<'a, Q: Quota + ?Sized>(
    groups: &(impl Iterator<Item = &'a [u64]> + Clone),
    pairs: Option<&Adjacency>,
    region: &mut Region,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
    counts: &mut Counts,
) -> Result<Narrowing, BudgetError<Q::Error>> {
    budget.poll()?;
    budget.charge(1)?; // inspect the completed source relation
    let Some(pairs) = pairs else {
        return empty_relation(groups, region, budget, counts);
    };
    let mut possible = Words::new(storage.filled(region.mask_words(), 0_u64, 1, budget)?);
    let mut forbidden = Words::new(storage.filled(region.mask_words(), 0_u64, 1, budget)?);
    let result = rounds(
        groups,
        pairs,
        region,
        &mut possible,
        &mut forbidden,
        budget,
        counts,
    );
    let release_possible = storage.release(possible.into_storage());
    let release_forbidden = storage.release(forbidden.into_storage());
    result.and_then(|result| {
        release_possible?;
        release_forbidden?;
        Ok(result)
    })
}

fn rounds<'a, Q: Quota + ?Sized>(
    groups: &(impl Iterator<Item = &'a [u64]> + Clone),
    pairs: &Adjacency,
    region: &mut Region,
    possible: &mut Words<u64>,
    forbidden: &mut Words<u64>,
    budget: &mut Budget<'_, '_, Q>,
    counts: &mut Counts,
) -> Result<Narrowing, BudgetError<Q::Error>> {
    budget.charge(1)?;
    let mut changed = false;
    loop {
        budget.charge(1)?;
        let mut round_changed = false;
        for group in groups.clone() {
            let (_, cut) = region.decision_words();
            for ((destination, &members), &cut) in possible.iter_mut().zip(group.iter()).zip(cut) {
                budget.charge(3)?; // NOT, AND, assignment
                *destination = members & !cut;
            }
            let pivot = words::first(possible, &mut |work| budget.charge(work))?;
            let Some(pivot) = pivot else {
                budget.charge(1)?;
                return Ok(Narrowing::Refuted);
            };
            for &target in &pairs[pivot] {
                budget.charge(1)?; // read this incidence occurrence, including duplicates
                if contains(region.decision_words().1, target, budget)? {
                    continue;
                }
                forbidden.fill_with(0, || budget.charge(1))?;
                for &source in &pairs[target] {
                    budget.charge(5)?; // occurrence read, locate's three operations, OR/store
                    forbidden.insert(source);
                }
                if possible.is_subset_of(Words::new(&forbidden[..]), &mut || budget.charge(1))? {
                    if contains(region.decision_words().0, target, budget)? {
                        budget.charge(1)?;
                        return Ok(Narrowing::Refuted);
                    }
                    budget.charge(11)?; // this coordinate is known open
                    assert!(region.cut(target));
                    counts.cuts += 1;
                    budget.charge(1)?;
                    round_changed = true;
                }
            }
        }
        counts.rounds += 1;
        budget.charge(1)?;
        changed |= round_changed;
        budget.charge(1)?;
        if !round_changed {
            budget.charge(2)?;
            return Ok(Narrowing::Fixed { changed });
        }
    }
}

/// No forbidden edge can cut a target, but a mandatory empty group still
/// refutes the original class. Group coordinates and the cut mask have the
/// same admitted universe; the shared packed subset check needs no scratch.
fn empty_relation<'a, Q: Quota + ?Sized>(
    groups: &(impl Iterator<Item = &'a [u64]> + Clone),
    region: &Region,
    budget: &mut Budget<'_, '_, Q>,
    counts: &mut Counts,
) -> Result<Narrowing, BudgetError<Q::Error>> {
    budget.charge(1)?; // borrow the current cut coordinates
    let (_, cut) = region.decision_words();
    for group in groups.clone() {
        budget.charge(1)?; // read this mandatory support descriptor
        if Words::new(group).is_subset_of(Words::new(cut), &mut || budget.charge(1))? {
            budget.charge(1)?;
            return Ok(Narrowing::Refuted);
        }
    }
    budget.charge(1)?; // complete the no-change round
    counts.rounds += 1;
    budget.charge(2)?;
    Ok(Narrowing::Fixed { changed: false })
}
