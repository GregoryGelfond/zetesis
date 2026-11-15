//! One Builder's term coordinates and successively completed aggregate families.
//!
//! Queries and cursors share this owner, never a Builder borrow. A registry
//! borrow ends before a nested join or formula operation; failed construction
//! publishes no new stage. The support reference outlives every shared handle.

pub(super) mod tuples;

use std::cell::{Cell, Ref, RefCell, RefMut};
use std::ops::Deref;
use std::rc::Rc;

use super::CachedAggregate;
use super::cache::Contexts;
use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_binding::Binding;
use crate::formula_ir::AggregateIr;
use crate::formula_support::{
    Buffer, CompletedQueries, Computation, Context, GroundingWork, Ir, StorageLease, Support,
    TermTable,
};
use crate::{FormulaFailure, FormulaResource, ProgramSite};

pub(crate) struct Workspace<'query, 'source> {
    support: &'query Support<'source>,
    enabled: Cell<bool>,
    pub(super) terms: RefCell<TermTable>,
    pub(super) rows: RefCell<Contexts<Family<'source>>>,
    pub(super) cached_elements: Cell<usize>,
    pub(super) cached_roots: Cell<usize>,
    lease: StorageLease,
}

pub(super) struct Family<'source> {
    pub(super) source: Option<&'source AggregateIr>,
    pub(super) produced: Option<Rc<Produced<'source>>>,
    pub(super) native: Option<CachedAggregate>,
}

/// One completed carrier. The Binding retains its canonical vocabulary and its
/// existing lease; the separate lease counts only this shared owner's remainder.
pub(crate) struct Produced<'source> {
    values: Binding<'static>,
    pub(super) replay: Option<tuples::Replay<'source>>,
    lease: StorageLease,
}

impl<'source> Produced<'source> {
    /// Retain completed values in one accounted shared owner. Refusal drops the
    /// unpublished values; the registry receives no partially built carrier.
    fn new(
        values: Binding<'static>,
        replay: Option<tuples::Replay<'source>>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Rc<Self>, FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let lease = computation.lease();
        counters.work(limits, location)?;
        let replay_header = replay
            .as_ref()
            .map_or(0, |_| size_of::<tuples::Replay<'source>>());
        let bytes =
            size_of::<Self>() - size_of::<Binding>() - replay_header + 2 * size_of::<usize>();
        admit_shared_owner(&lease, bytes, computation, limits, location)?;
        counters.work(limits, location)?;
        let mut produced = Rc::new(Produced {
            values,
            replay,
            lease,
        });
        let allocated = Rc::get_mut(&mut produced).expect("new unpublished carrier owner");
        allocated.lease.observe(bytes, location)?;
        computation.storage_observed(&allocated.lease, 0, 0, limits, counters, location)?;
        Ok(produced)
    }
}

/// Growing queries retain the established local carrier; one final Builder can
/// instead share its already completed stage. Both expose the same value frame.
pub(crate) enum Carrier<'source> {
    Owned(Binding<'static>),
    Shared(Rc<Produced<'source>>),
}
impl Deref for Carrier<'_> {
    type Target = Binding<'static>;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(values) => values,
            Self::Shared(produced) => &produced.values,
        }
    }
}
impl Carrier<'_> {
    pub(crate) fn owns_binding_header(&self) -> bool {
        matches!(self, Self::Owned(_))
    }
}

impl<'query, 'source> Workspace<'query, 'source> {
    pub(super) fn new(
        context: Context<'_, &Computation<'query, 'source>>,
    ) -> Result<Rc<Self>, FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let terms = TermTable::new(computation, limits, counters, location)?;
        let rows = Contexts::new(computation, limits, counters, location)?;
        // TermTable and Contexts already account their own complete headers.
        // Rc's two control words and all remaining inline fields are new owners.
        let bytes =
            size_of::<Self>() - size_of::<TermTable>() - size_of::<Contexts<Family<'source>>>()
                + 2 * size_of::<usize>();
        let lease = computation.lease();
        admit_shared_owner(&lease, bytes, computation, limits, location)?;
        counters.work(limits, location)?;
        let mut owner = Rc::new(Self {
            support: computation.query_support(),
            enabled: Cell::new(false),
            terms: RefCell::new(terms),
            rows: RefCell::new(rows),
            cached_elements: Cell::new(0),
            cached_roots: Cell::new(0),
            lease,
        });
        let allocated = Rc::get_mut(&mut owner).expect("new unpublished shared owner");
        allocated.lease.observe(bytes, location)?;
        computation.storage_observed(&allocated.lease, 0, 0, limits, counters, location)?;
        Ok(owner)
    }

    /// One charged dynamic borrow of the existing coordinate owner. The
    /// returned guard cannot overlap a mutable borrow or recursive source call.
    pub(super) fn terms(
        &self,
        work: GroundingWork<'_>,
    ) -> Result<Ref<'_, TermTable>, FormulaFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        counters.work(limits, location)?;
        Ok(self.terms.borrow())
    }
    pub(super) fn terms_mut(
        &self,
        work: GroundingWork<'_>,
    ) -> Result<RefMut<'_, TermTable>, FormulaFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        counters.work(limits, location)?;
        Ok(self.terms.borrow_mut())
    }

    /// Only the completed-support capability can enable producer retention.
    /// Other builders still use this same owner's ordinary native cache.
    pub(super) fn enable(
        &self,
        queries: &CompletedQueries<'source>,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        self.authenticate(queries.support(), location)?;
        self.enabled.set(true);
        Ok(())
    }

    pub(crate) fn authenticate(
        &self,
        support: &Support<'_>,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        if std::ptr::eq(
            std::ptr::from_ref(self.support).cast::<()>(),
            std::ptr::from_ref(support).cast::<()>(),
        ) {
            Ok(())
        } else {
            Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location,
            })
        }
    }

    pub(super) fn key(
        &self,
        inputs: &[usize],
        assignment: &Binding,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Buffer<Option<usize>>, FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        self.authenticate(computation.query_support(), location)?;
        self.terms(GroundingWork::new(limits, counters, location))?
            .authenticate_owner(computation, limits, counters, location)?;
        // Empty projections still authenticate the live vocabulary witness.
        assignment
            .slots()
            .prefix(0)
            .expect("the empty prefix exists")
            .bind_with(computation.read(), || counters.work(limits, location))
            .map_err(|error| crate::formula_binding::failure(error, location))?;
        let mut key = Buffer::new(computation, limits, counters, location)?;
        key.resize(inputs.len(), None, computation, limits, counters, location)?;
        for (at, &slot) in inputs.iter().enumerate() {
            counters.work(limits, location)?;
            let value = assignment
                .slots()
                .key(slot)
                .map_err(|error| crate::formula_binding::assignment(error, location))?;
            key.slice_mut()[at] = value
                .as_ref()
                .map(|value| {
                    self.terms_mut(GroundingWork::new(limits, counters, location))?
                        .insert(value, None, computation, limits, counters, location)
                })
                .transpose()?;
        }
        Ok(key)
    }

    pub(crate) fn values(
        self: &Rc<Self>,
        aggregate: Ir<'_, 'source, AggregateIr>,
        assignment: &Binding,
        support: &Support<'source>,
        budget: &mut Budget,
        context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<Carrier<'source>, FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        self.authenticate(support, location)?;
        self.authenticate(computation.query_support(), location)?;
        counters.work(limits, location)?;
        let Ir::Source(source) = aggregate else {
            return crate::formula_assignment::values(
                aggregate,
                assignment,
                support,
                budget,
                Context::new(computation, limits, counters, location),
            )
            .map(Carrier::Owned);
        };
        let enabled = self.enabled.get();
        #[cfg(test)]
        let enabled = enabled && testing::enabled();
        if !enabled {
            return crate::formula_assignment::values(
                aggregate,
                assignment,
                support,
                budget,
                Context::new(computation, limits, counters, location),
            )
            .map(Carrier::Owned);
        }
        let key = self.key(
            &source.family_inputs,
            assignment,
            Context::new(&*computation, limits, counters, location),
        )?;
        {
            counters.work(limits, location)?;
            let rows = self.rows.borrow();
            let slot = rows.find(source.id, &key, computation, limits, counters, location)?;
            if let Some(slot) = slot {
                let family = rows.get(slot).expect("published family coordinate");
                if let Some(original) = family.source
                    && !std::ptr::eq(original, source)
                {
                    return Err(FormulaFailure::SupportRelation {
                        error: zetesis_core::relation::Failure::Owner,
                        location,
                    });
                }
                if let Some(produced) = &family.produced {
                    counters.work(limits, location)?;
                    #[cfg(test)]
                    testing::record(testing::Event::Reused);
                    return Ok(Carrier::Shared(Rc::clone(produced)));
                }
            }
        }
        // Registry and term borrows ended before complete source construction.
        let produced = self.produce(
            aggregate,
            assignment,
            support,
            budget,
            Context::new(&mut *computation, limits, counters, location),
        )?;
        counters.work(limits, location)?;
        let mut rows = self.rows.borrow_mut();
        // Nested construction may append rows; integer coordinates remain stable.
        // A completed nested stage is never overwritten by this draft.
        let current = rows.find(source.id, &key, computation, limits, counters, location)?;
        if let Some(slot) = current {
            let family = rows.get_mut(slot).expect("existing native family");
            if let Some(existing) = &family.produced {
                counters.work(limits, location)?;
                #[cfg(test)]
                testing::record(testing::Event::Reused);
                return Ok(Carrier::Shared(Rc::clone(existing)));
            }
            counters.work(limits, location)?;
            family.source = Some(source);
            family.produced = Some(Rc::clone(&produced));
        } else {
            rows.insert(
                source.id,
                &key,
                Family {
                    source: Some(source),
                    produced: Some(Rc::clone(&produced)),
                    native: None,
                },
                Context::new(&*computation, limits, counters, location),
            )?;
        }
        #[cfg(test)]
        testing::record(testing::Event::Produced);
        Ok(Carrier::Shared(produced))
    }

    /// Construct every producer-owned component before exposing a registry row.
    /// Refusal may retain interned vocabulary coordinates, never a partial stage.
    fn produce(
        &self,
        aggregate: Ir<'_, 'source, AggregateIr>,
        assignment: &Binding,
        support: &Support<'source>,
        budget: &mut Budget,
        context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<Rc<Produced<'source>>, FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let mut trace = tuples::Trace::new(
            self,
            Context::new(&*computation, limits, counters, location),
        )?;
        let collected = crate::formula_assignment::collect(
            aggregate,
            assignment,
            support,
            budget,
            &mut trace,
            Context::new(&mut *computation, limits, counters, location),
        )?;
        let values = collected.carrier(
            crate::formula_assignment::Kind::of(aggregate.function),
            Context::new(&mut *computation, limits, counters, location),
        )?;
        let replay = trace.finish(
            &collected,
            Context::new(&*computation, limits, counters, location),
        )?;
        drop(collected);
        Produced::new(
            values,
            replay,
            Context::new(&*computation, limits, counters, location),
        )
    }
}

/// Admit a future owner without reporting its unallocated capacity as observed.
/// The caller publishes the actual lease immediately after Rc allocation.
fn admit_shared_owner(
    lease: &StorageLease,
    bytes: usize,
    computation: &Computation<'_, '_>,
    limits: &crate::FormulaLimits,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    let available = computation.allowance(lease, limits, location)?;
    ceiling(
        FormulaResource::SupportBytes,
        (limits.max_support_bytes - available) as u128 + bytes as u128,
        limits.max_support_bytes as u128,
        location,
    )
}

#[cfg(test)]
pub(crate) mod testing {
    use std::cell::Cell;

    #[derive(Clone, Copy, Debug, Default)]
    pub(crate) struct Counts {
        pub(crate) produced: usize,
        pub(crate) reused: usize,
        pub(crate) carrier_borrowed: usize,
        pub(crate) consumed: usize,
    }
    #[derive(Clone, Copy)]
    pub(crate) enum Event {
        Produced,
        Reused,
        CarrierBorrowed,
        Consumed,
    }
    thread_local! {
        static ENABLED: Cell<bool> = const { Cell::new(true) };
        static COUNTS: Cell<Counts> = const { Cell::new(Counts {
            produced: 0, reused: 0, carrier_borrowed: 0, consumed: 0,
        }) };
    }
    pub(super) fn enabled() -> bool {
        ENABLED.get()
    }
    pub(crate) fn counts() -> Counts {
        COUNTS.get()
    }
    pub(crate) fn record(event: Event) {
        COUNTS.with(|cell| {
            let mut counts = cell.get();
            match event {
                Event::Produced => counts.produced += 1,
                Event::Reused => counts.reused += 1,
                Event::CarrierBorrowed => counts.carrier_borrowed += 1,
                Event::Consumed => counts.consumed += 1,
            }
            cell.set(counts);
        });
    }
    pub(crate) fn scoped<T>(enabled: bool, action: impl FnOnce() -> T) -> T {
        struct Restore(bool, Counts);
        impl Drop for Restore {
            fn drop(&mut self) {
                ENABLED.set(self.0);
                COUNTS.set(self.1);
            }
        }
        let _restore = Restore(ENABLED.replace(enabled), COUNTS.replace(Counts::default()));
        action()
    }
}

#[cfg(test)]
mod tests;
