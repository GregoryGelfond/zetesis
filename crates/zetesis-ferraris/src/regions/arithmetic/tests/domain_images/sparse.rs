//! Private accounted sparse image experiment, over original source clauses.
//! No production Subject, public receipt, default or membership path is changed.

mod borrowed;
mod kernel;
mod prepare;
mod tests;

use super::{
    Budget, BudgetError, Cancellation, Meter, Narrowing, Region, Stop, Storage, Theory, Words,
};
use crate::regions::adjacency::Adjacency;
use zetesis_algebra::words;

struct Sparse {
    groups: Vec<Words<u64>>,
    forbidden: Option<Adjacency>,
}

#[derive(Clone, Copy)]
struct Limits {
    global_work: u64,
    work: u64,
    bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            global_work: u64::MAX,
            work: u64::MAX,
            bytes: usize::MAX,
        }
    }
}

struct Attempt {
    result: Result<Option<(Region, Narrowing)>, BudgetError>,
    preparation: u64,
    copying: u64,
    closure: u64,
    work: u64,
    peak: u128,
}

/// All mutations stay in an unpublished Region; a refused attempt returns none.
/// The input's already-owned storage is included as a live borrowed baseline.
fn run(
    owner: &Theory,
    theory: &Theory,
    input: &Region,
    limits: Limits,
    cancellation: &Cancellation,
) -> Attempt {
    let mut meter = Meter::local(limits.global_work);
    let mut budget = Budget::new(&mut meter, limits.work, limits.bytes, cancellation);
    let mut storage = Storage::with_live_bytes(input.retained_bytes());
    let mut phases = [0; 3];
    let result = (|| {
        budget.poll()?;
        budget.admit_bytes(storage.bytes())?;
        let before = budget.work();
        let prepared = (|| {
            budget.charge(3)?;
            if (!owner.same_instance(theory)) | (input.len() != theory.atom_count()) {
                return Err(BudgetError::Stopped(Stop::InvalidProgram));
            }
            prepare::read(theory, &mut storage, &mut budget)
        })();
        phases[0] = budget.work() - before;
        let Some(system) = prepared? else {
            return Ok(None);
        };
        // Private observation only: one bounded row-length scan, not part of
        // the logical kernel receipt or an end-to-end timing claim.
        let edges: usize = system.forbidden.as_ref().map_or(0, |pairs| {
            (0..theory.atom_count()).map(|atom| pairs[atom].len()).sum()
        });
        assert_eq!(edges % 2, 0);
        let before = budget.work();
        let copied = copy_region(input, &mut storage, &mut budget);
        phases[1] = budget.work() - before;
        let mut region = copied?;
        let before = budget.work();
        let narrowed = system
            .close(&mut region, &mut storage, &mut budget)
            .and_then(|outcome| {
                system.release(&mut storage, &mut budget)?;
                Ok(outcome)
            });
        phases[2] = budget.work() - before;
        let narrowed = narrowed?;
        assert_eq!(
            storage.bytes(),
            input.retained_bytes() + region.retained_bytes()
        );
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

fn copy_region(
    input: &Region,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
) -> Result<Region, BudgetError> {
    let baseline = storage.bytes();
    let mut context = (budget, baseline);
    let mut region = Region::try_all_open_with(
        input.len(),
        &mut context,
        &mut |(budget, _), count| budget.charge(count),
        &mut |(budget, baseline), bytes| budget.admit_bytes(*baseline + bytes),
    )?;
    let budget = context.0;
    storage.admit_record(region.retained_bytes(), budget)?;
    let (held, cut) = input.decision_words();
    for (mask, value) in [(held, true), (cut, false)] {
        words::visit_set_bits(
            mask,
            &mut (&mut *budget, &mut region),
            &mut |(budget, _), count| budget.charge(count),
            &mut |(budget, region), atom| {
                budget.charge(12)?; // polarity choice plus the open Region decision's eleven units
                let accepted = if value {
                    region.hold(atom)
                } else {
                    region.cut(atom)
                };
                assert!(
                    accepted,
                    "the copied disjoint source masks select an open atom"
                );
                Ok(())
            },
        )?;
    }
    Ok(region)
}

impl Sparse {
    fn close(
        &self,
        region: &mut Region,
        storage: &mut Storage,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Narrowing, BudgetError> {
        kernel::close(
            &self.groups.iter().map(|group| &group[..]),
            self.forbidden.as_ref(),
            region,
            storage,
            budget,
        )
    }

    fn release(
        self,
        storage: &mut Storage,
        budget: &mut Budget<'_, '_>,
    ) -> Result<(), BudgetError> {
        let mut groups = self.groups;
        for group in groups.drain(..) {
            storage.release_embedded(group.into_storage())?;
        }
        storage.release(groups)?;
        crate::regions::arithmetic::images::pairs::release(self.forbidden, storage, budget)
    }
}
