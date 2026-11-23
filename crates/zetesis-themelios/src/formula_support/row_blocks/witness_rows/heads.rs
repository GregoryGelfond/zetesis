//! Workers project charged head-key words; ordered publication interns them.
//!
//! Fixed logical tasks produce the same head projections as `RowHead`. Their
//! word buffers and projection scratch use the ordinary worker byte accounts.
//! After all tasks settle, the coordinator interns keys in staged order under
//! its remaining publication allowance, freezes on the counted grounding pool,
//! and publishes each identity once. No shared allocation is attributed to the
//! worker that happened to win a shard lock. Task geometry, projected values,
//! atom discovery order, and `HeadKeyPublication`'s semantic premises are unchanged.
//!
//! Insertion is serial in staged order. Projection and freeze retain their
//! counted pools. Transport and projection buffers use the ordinary logical
//! work tariffs; each row also prepays the existing fixed key insertion tariff.
//! `proofs/Zetesis/HeadKeyPublication.lean` states the unchanged skip law.

use zetesis_core::TemplateTerm;
use zetesis_core::atom_registry::RowColumn;
use zetesis_core::intern::{ContentInterner, Generation, Handle, Pending, Storage, Words, Work};

use super::{
    Batch, Buffer, Computation, Context, Counters, FormulaFailure, GroundingWork, StorageLease,
    WitnessPlan, WitnessRow, owner,
};
use crate::formula_ir::HeadIr;
use crate::formula_support::heads::row_column;

/// A publication whose workers stage head-key projections.
pub(crate) struct HeadKeys;

impl HeadKeys {
    pub(crate) fn new() -> Self {
        Self
    }

    /// Intern complete staged batches in their publication order. The caller
    /// passes the reduced publication allowance while keeping worker output
    /// reservations live. Storage refusals retain the actual capacity peak and
    /// reach the existing final freeze-work charge before propagating.
    pub(crate) fn seen<'batch, 'plan: 'batch, 'source: 'plan, I>(
        batches: impl Fn() -> I,
        pool: Option<&zetesis_algebra::threads::Pool>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Seen>, FormulaFailure>
    where
        I: Iterator<Item = &'batch Batch<'plan, 'source>>,
    {
        freeze_keys(
            || {
                batches().map(|batch| KeyRows {
                    rows: batch.completions.len(),
                    width: batch.key_width,
                    words: batch.keys.as_ref().map(Buffer::slice),
                })
            },
            pool,
            context,
        )
    }
}

/// A borrowed fixed-width projection beside a complete batch's rows. The
/// enclosing `Batch` authenticates its source; this view allocates nothing.
#[derive(Clone, Copy)]
struct KeyRows<'a> {
    rows: usize,
    width: usize,
    words: Option<&'a [u32]>,
}

fn freeze_keys<'keys, I>(
    batches: impl Fn() -> I,
    pool: Option<&zetesis_algebra::threads::Pool>,
    context: Context<'_, &Computation<'_, '_>>,
) -> Result<Option<Seen>, FormulaFailure>
where
    I: Iterator<Item = KeyRows<'keys>>,
{
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    if !batches().any(|batch| batch.words.is_some() && batch.rows != 0) {
        return Ok(None);
    }
    let count = batches().try_fold(0usize, |count, batch| {
        count.checked_add(batch.rows).ok_or_else(|| owner(location))
    })?;
    let header = size_of::<Seen>()
        - size_of::<Generation>()
        - size_of::<Buffer<Option<Pending>>>()
        - size_of::<Buffer<u64>>();
    let mut lease = computation.lease();
    lease.observe(header, location)?;
    computation.storage_observed(&lease, 0, header, limits, counters, location)?;
    // Reserve all coordinator-owned output before granting the interner
    // its disjoint remainder. Every possible identity has a mark slot.
    let mut pending = Buffer::new(computation, limits, counters, location)?;
    pending.resize(count, None, computation, limits, counters, location)?;
    let mut marks = Buffer::new(computation, limits, counters, location)?;
    marks.resize(
        count.div_ceil(64),
        0_u64,
        computation,
        limits,
        counters,
        location,
    )?;
    let allowance = computation.allowance(&lease, limits, location)?;
    let available = allowance
        .checked_sub(header)
        .ok_or_else(|| owner(location))?;
    let outside = limits.max_support_bytes as u128 - available as u128;
    let map = |error| refusal(error, outside, limits, location);
    let storage = Storage::new(available).map_err(map)?;
    let mut freeze_work = Work::default();
    let result = intern_rows(
        batches(),
        pool,
        &storage,
        pending.slice_mut(),
        &mut freeze_work,
        map,
        location,
    );
    let charged = counters.charge_work(u128::from(freeze_work.total()), limits, location);
    let observed = computation.storage_peak(
        &lease,
        header as u128 + storage.peak_bytes(),
        limits,
        counters,
        location,
    );
    let retained = header as u128 + storage.current_bytes();
    let retained = usize::try_from(retained).map_err(|_| FormulaFailure::Limit {
        resource: crate::FormulaResource::SupportBytes,
        observed: outside - header as u128 + retained,
        limit: limits.max_support_bytes as u128,
        location,
    });
    // Settle capacity even when the next work/control boundary refused.
    let retained = retained.and_then(|bytes| lease.observe(bytes, location));
    charged?;
    observed?;
    retained?;
    Ok(Some(Seen {
        generation: result?,
        pending,
        marks,
        next: 0,
        _lease: lease,
    }))
}

/// Intern in one fixed staged order. Its fixed row tariff was prepaid by the
/// workers; the returned freeze tally is settled by the coordinator.
fn intern_rows<'keys>(
    batches: impl Iterator<Item = KeyRows<'keys>>,
    pool: Option<&zetesis_algebra::threads::Pool>,
    storage: &Storage,
    pending: &mut [Option<Pending>],
    work: &mut Work,
    map: impl Fn(zetesis_core::intern::Refusal) -> FormulaFailure + Copy,
    location: crate::ProgramSite,
) -> Result<Generation, FormulaFailure> {
    let mut interner = ContentInterner::in_storage(Words, storage.clone()).map_err(map)?;
    let mut at = 0;
    for batch in batches {
        let rows = batch.rows;
        if let Some(keys) = batch.words {
            let expected = rows
                .checked_mul(batch.width)
                .ok_or_else(|| map(zetesis_core::intern::Refusal::Exhausted))?;
            if keys.len() != expected {
                return Err(owner(location));
            }
            for row in 0..rows {
                let start = row * batch.width;
                let key = &keys[start..start + batch.width];
                // The worker's fixed key tariff covers this insertion.
                // Freeze retains its independent content-determined tally.
                let mut tariff = Work::default();
                let handle = interner.intern(key, &mut tariff).map_err(map)?;
                let Handle::Pending(id) = handle else {
                    return Err(owner(location));
                };
                pending[at + row] = Some(id);
            }
        }
        at += rows;
    }
    interner.freeze(pool, work).map_err(map)
    // Only the returned remap is needed for publication. The frozen
    // arena and tables drop here, after their peak has been retained.
}

fn refusal(
    error: zetesis_core::intern::Refusal,
    outside: u128,
    limits: &crate::FormulaLimits,
    location: crate::ProgramSite,
) -> FormulaFailure {
    match error {
        zetesis_core::intern::Refusal::Storage { required, .. } => FormulaFailure::Limit {
            resource: crate::FormulaResource::SupportBytes,
            observed: outside + required,
            limit: limits.max_support_bytes as u128,
            location,
        },
        zetesis_core::intern::Refusal::Allocation => crate::formula_binding::assignment(
            zetesis_core::catalog::AssignmentError::Storage(
                zetesis_core::catalog::Error::Allocation,
            ),
            location,
        ),
        zetesis_core::intern::Refusal::Exhausted => crate::formula_binding::assignment(
            zetesis_core::catalog::AssignmentError::Storage(zetesis_core::catalog::Error::Overflow),
            location,
        ),
    }
}

/// One task's charged head projection, prepared at its first staged row.
pub(super) struct Keyer {
    slots: Buffer<RowColumn>,
    scratch: Buffer<u32>,
    prepared: bool,
    keyed: bool,
    _lease: StorageLease,
}

impl Keyer {
    pub(super) fn new(context: Context<'_, &Computation<'_, '_>>) -> Result<Self, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let header = size_of::<Self>() - size_of::<Buffer<RowColumn>>() - size_of::<Buffer<u32>>();
        let mut lease = computation.lease();
        lease.observe(header, location)?;
        computation.storage_observed(&lease, 0, header, limits, counters, location)?;
        Ok(Self {
            slots: Buffer::new(computation, limits, counters, location)?,
            scratch: Buffer::new(computation, limits, counters, location)?,
            prepared: false,
            keyed: false,
            _lease: lease,
        })
    }

    pub(super) fn key<'a>(
        &'a mut self,
        plan: &WitnessPlan<'_, '_>,
        row: &WitnessRow<'_, '_>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<&'a [u32]>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        if !self.prepared {
            self.keyed = slots(
                &mut self.slots,
                plan,
                row,
                Context::new(computation, limits, counters, location),
            )?;
            self.prepared = true;
        }
        if !self.keyed {
            return Ok(None);
        }
        counters.charge_work(self.slots.len() as u128 + 2, limits, location)?;
        self.scratch.clear();
        for slot in self.slots.iter() {
            let (Some(matched), Some(source)) = (
                row.rows.get(slot.input),
                plan.sources
                    .slice()
                    .get(slot.input)
                    .and_then(|resolution| resolution.rows()),
            ) else {
                return Ok(None);
            };
            if !matched.belongs_to(&source.relation) {
                return Ok(None);
            }
            let Some(id) = matched.equality_id(slot.column) else {
                return Ok(None);
            };
            self.scratch
                .push(id, computation, limits, counters, location)?;
        }
        Ok(Some(self.scratch.slice()))
    }
}

fn slots(
    slots: &mut Buffer<RowColumn>,
    plan: &WitnessPlan<'_, '_>,
    row: &WitnessRow<'_, '_>,
    context: Context<'_, &Computation<'_, '_>>,
) -> Result<bool, FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    let HeadIr::Normal(Some(pattern)) = plan.base.rule.head else {
        return Ok(false);
    };
    let Some(components) = row.support.components() else {
        return Ok(false);
    };
    let pattern = pattern.get(components, limits, counters, location)?;
    let terms = pattern.terms();
    slots.reserve(terms.len(), computation, limits, counters, location)?;
    for index in 0..terms.len() {
        counters.work(limits, location)?;
        match terms.at(index) {
            Some(TemplateTerm::Constant(_)) => {}
            Some(TemplateTerm::Variable(variable)) => {
                let Some(column) = row_column(variable, row, limits, counters, location)? else {
                    return Ok(false);
                };
                slots.push(column, computation, limits, counters, location)?;
            }
            None => return Ok(false),
        }
    }
    Ok(true)
}

/// One frozen publication's remap, staged coordinates, and packed seen set.
pub(crate) struct Seen {
    generation: Generation,
    pending: Buffer<Option<Pending>>,
    marks: Buffer<u64>,
    next: usize,
    _lease: StorageLease,
}

impl Seen {
    /// Read exactly one staged row, retaining the first head of each key.
    /// Charge a keyed probe before advancing or changing its seen bit.
    pub(super) fn first(
        &mut self,
        limits: &crate::FormulaLimits,
        counters: &mut Counters,
        location: crate::ProgramSite,
    ) -> Result<Option<bool>, FormulaFailure> {
        let key = self
            .pending
            .slice()
            .get(self.next)
            .ok_or_else(|| owner(location))?;
        let Some(key) = *key else {
            self.next += 1;
            return Ok(None);
        };
        counters.work(limits, location)?;
        let index = self.generation.resolve(key).index() - self.generation.base();
        let (word, bit) = (index / 64, 1_u64 << (index % 64));
        let mark = &mut self.marks.slice_mut()[word];
        let first = *mark & bit == 0;
        *mark |= bit;
        self.next += 1;
        Ok(Some(first))
    }
}

#[cfg(test)]
mod tests;
