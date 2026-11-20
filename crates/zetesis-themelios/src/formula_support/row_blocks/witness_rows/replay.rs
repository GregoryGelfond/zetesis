//! A lending reader restores exact source rows; concrete consumers own meaning.

use super::{
    Batch, Buffer, Completion, Computation, Context, Counters, FormulaFailure, FormulaLimits,
    GroundingWork, PackedCoords, Purpose, Row, StorageLease, Support, WitnessPlan, WitnessRow,
    owner, restore,
};
use crate::grounding_observer::Event;

pub(crate) struct Replay<'plan, 'source> {
    prepared: &'plan WitnessPlan<'plan, 'source>,
    support: &'plan Support<'source>,
    rows: Buffer<Row<'source, 'source>>,
    _lease: StorageLease,
}

impl<'plan, 'source: 'plan> Replay<'plan, 'source> {
    pub(crate) fn new(
        prepared: &'plan WitnessPlan<'plan, 'source>,
        support: &'plan Support<'source>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        counters.work(limits, location)?;
        if !std::ptr::eq(prepared.relations, support.relations()) {
            return Err(owner(location));
        }
        let mut lease = computation.lease();
        let header = size_of::<Self>() - size_of::<Buffer<Row<'source, 'source>>>();
        lease.observe(header, location)?;
        computation.storage_observed(&lease, 0, header, limits, counters, location)?;
        Ok(Self {
            prepared,
            support,
            rows: Buffer::new(computation, limits, counters, location)?,
            _lease: lease,
        })
    }

    pub(crate) fn batch<'batch>(
        &'batch mut self,
        batch: &'batch Batch<'plan, 'source>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Rows<'batch, 'plan, 'source>, FormulaFailure> {
        let location = self.prepared.location();
        counters.work(limits, location)?;
        if !std::ptr::eq(self.prepared, batch.plan) {
            return Err(owner(location));
        }
        Ok(Rows {
            replay: self,
            suffixes: &batch.suffixes,
            completions: batch.completions.slice().iter(),
            start: 0,
        })
    }

    pub(crate) fn retained<'batch>(
        &'batch mut self,
        family: &'batch super::retained::Family<'plan, 'source>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Rows<'batch, 'plan, 'source>, FormulaFailure> {
        let location = self.prepared.location();
        counters.charge_work(3, limits, location)?;
        if !std::ptr::eq(self.prepared.base.plan, &raw const family.plan)
            || !std::ptr::eq(self.prepared.relations, family.relations)
            || !std::ptr::eq(self.prepared.base.rule, family.rule)
        {
            return Err(owner(location));
        }
        Ok(Rows {
            replay: self,
            suffixes: &family.suffixes,
            completions: family.completions.slice().iter(),
            start: 0,
        })
    }
}

pub(crate) struct Rows<'batch, 'plan, 'source> {
    replay: &'batch mut Replay<'plan, 'source>,
    suffixes: &'batch PackedCoords,
    completions: std::slice::Iter<'batch, Completion>,
    start: usize,
}

impl<'plan, 'source: 'plan> Rows<'_, 'plan, 'source> {
    pub(crate) fn next(
        &mut self,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<WitnessRow<'_, 'source>>, FormulaFailure> {
        let Some(&completion) = self.completions.next() else {
            return Ok(None);
        };
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let mut operation = |counters: &mut Counters| {
            restore(
                &mut self.replay.rows,
                self.replay.prepared,
                self.suffixes,
                self.start,
                completion,
                Context::new(computation, limits, counters, location),
            )
        };
        if self.replay.prepared.purpose == Purpose::Support {
            counters.observe_work(Event::SupportJoinWork, operation)?;
        } else {
            operation(counters)?;
        }
        self.start = completion.suffix_end;
        Ok(Some(WitnessRow {
            rows: self.replay.rows.slice(),
            unchanged: completion.unchanged,
            patterns: &self.replay.prepared.base.plan.patterns,
            support: self.replay.support,
            resolutions: self.replay.prepared.sources.slice(),
        }))
    }
}
