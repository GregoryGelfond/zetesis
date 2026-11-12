//! Final eligibility over a producer's certified ordered tuple occurrences.
//! Fresh joins preserve the current outer and local frame. Only tuple identity,
//! ASP order and requiredness move from the one existing family owner.

use std::rc::Rc;
use std::sync::Arc;

use super::families::tuples::Replay;
use super::{
    AggregateElement, Builder, Coalesced, ExtremumElement, FALSUM, GroundAggregate, GroundKey,
    HeadContributions, Kind, Measure,
};
use crate::formula_binding::Binding;
use crate::formula_ir::AggregateIr;
use crate::formula_support::{Buffer, Context, Ir, Join, Support};
use crate::{FormulaFailure, ProgramSite};

impl<'source> Builder<'_, '_, 'source> {
    pub(super) fn replay_aggregate(
        &mut self,
        aggregate: Ir<'_, 'source, AggregateIr>,
        assignment: &Binding,
        support: &Support<'source>,
        replay: &Replay<'source>,
        location: ProgramSite,
    ) -> Result<Coalesced, FormulaFailure> {
        let kind = Kind::of(aggregate.function);
        // Entries are in the producer's canonical full-tuple order. None means
        // no contributing occurrence has been visited, not a false condition.
        let mut states: Buffer<Option<(Measure, usize)>> =
            Buffer::new(self.computation, self.limits, self.counters, location)?;
        states.resize(
            replay.tuples.len(),
            None,
            self.computation,
            self.limits,
            self.counters,
            location,
        )?;
        let mut at = 0;
        for (element, segment) in aggregate
            .map(|source| source.elements.as_slice())
            .iter()
            .zip(replay.segments.iter())
        {
            let mut local = Join::local(
                element.map(|element| element.condition.as_slice()),
                assignment,
                element.variables,
                support,
                self.budget,
                Context::new(&*self.computation, self.limits, self.counters, location),
            )?;
            local.share_families(
                Rc::clone(&self.families),
                self.limits,
                self.counters,
                location,
            )?;
            local.select_numeric_local(Context::new(
                &*self.computation,
                self.limits,
                self.counters,
                location,
            ))?;
            while let Some(binding) = local.next(
                self.computation,
                self.limits,
                self.budget,
                self.counters,
                location,
            )? {
                self.work(location)?;
                assert!(
                    at < segment.end,
                    "certified producer and fresh join have identical occurrences"
                );
                let tuple = replay.occurrences.slice()[at];
                at += 1;
                let key = GroundKey::Tuple(replay.tuples.slice()[tuple].coordinate);
                // Preserve ignored-weight and endpoint-error order: measure is
                // evaluated for each occurrence before its fresh condition.
                let Some(weight) = self.measure(kind, key, location)? else {
                    continue;
                };
                let condition = self.body_ir(
                    element.map(|element| element.condition.as_slice()).iter(),
                    &binding,
                    location,
                    support,
                )?;
                self.work(location)?;
                let previous = states.slice()[tuple].map_or(FALSUM, |(_, condition)| condition);
                let condition = self.or(previous, condition, location)?;
                self.work(location)?;
                states.slice_mut()[tuple] = Some((weight, condition));
            }
            self.work(location)?;
            assert_eq!(
                at, segment.end,
                "certified producer and fresh join exhaust together"
            );
        }
        self.work(location)?;
        assert_eq!(
            at,
            replay.occurrences.len(),
            "all producer segments consumed"
        );
        let result = self.finish_replayed_aggregate(kind, replay, &states, location)?;
        #[cfg(test)]
        super::families::testing::record(super::families::testing::Event::Consumed);
        Ok(result)
    }

    fn finish_replayed_aggregate(
        &mut self,
        kind: Kind,
        replay: &Replay<'source>,
        states: &Buffer<Option<(Measure, usize)>>,
        location: ProgramSite,
    ) -> Result<Coalesced, FormulaFailure> {
        // Match the old native allocation's contributing population, including
        // literal FALSUM conditions and excluding only ignored numeric tuples.
        let mut count = 0;
        for state in states.iter() {
            self.work(location)?;
            count += usize::from(state.is_some());
        }
        let mut elements = HeadContributions::new(
            kind.extremum(),
            count,
            self.computation,
            self.limits,
            self.counters,
            location,
        )?;
        let mut required = Buffer::new(self.computation, self.limits, self.counters, location)?;
        required.reserve(
            count,
            self.computation,
            self.limits,
            self.counters,
            location,
        )?;
        for (tuple, state) in replay.tuples.iter().zip(states.iter()) {
            self.work(location)?;
            let Some((measure, condition)) = *state else {
                continue;
            };
            required.push(
                tuple.required,
                self.computation,
                self.limits,
                self.counters,
                location,
            )?;
            match (&mut elements, measure) {
                (HeadContributions::Numeric(elements), Measure::Numeric(weight)) => {
                    elements.push(
                        AggregateElement { weight, condition },
                        self.computation,
                        self.limits,
                        self.counters,
                        location,
                    )?;
                }
                (HeadContributions::Extrema(elements), Measure::Extremum(value)) => {
                    elements.push(
                        ExtremumElement { value, condition },
                        self.computation,
                        self.limits,
                        self.counters,
                        location,
                    )?;
                }
                _ => unreachable!("the aggregate has one fixed contribution kind"),
            }
        }
        self.work(location)?;
        let elements: GroundAggregate = elements.finish();
        Ok((elements, Some(Arc::new(required))))
    }
}
