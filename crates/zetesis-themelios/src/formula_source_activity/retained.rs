//! Private cycle-local retention of complete immutable producer witnesses.
//! Activity and every source consumer are evaluated anew in each round.

#[cfg(test)]
mod tests;

use super::{Context, SourceEligibility, parallel::Producer};
use crate::formula_support::row_blocks::witness_rows::retained::{Family, Recording};
use crate::formula_support::row_blocks::witness_rows::{Replay, RowBinding};
use crate::formula_support::{Buffer, Context as WorkContext, Join, StorageLease, Support};
use crate::{FormulaFailure, formula_ground::SourceBodies, formula_ir::Prepared};

/// A borrowed cycle-local dispatch view, not a second owner or allocation.
/// Its absent state has no retained family. The Some owner accounts its complete
/// inline Option layout and every actual payload when first constructed.
pub(super) struct Retention<'state, 'query, 'source> {
    current: &'state mut Option<Rows<'query, 'source>>,
    rules: usize,
}

impl<'state, 'query, 'source: 'query> Retention<'state, 'query, 'source> {
    pub(super) fn new(
        current: &'state mut Option<Rows<'query, 'source>>,
        prepared: &Prepared,
    ) -> Self {
        Self {
            current,
            rules: prepared.rules_without_facts().len(),
        }
    }

    pub(super) fn replay(
        &self,
        owner: &mut SourceEligibility,
        producer: Producer<'source, '_>,
        support: &'query Support<'source>,
        bodies: &mut SourceBodies<'_, '_, 'source, '_>,
    ) -> Result<bool, FormulaFailure> {
        let context = bodies.context(producer.position.1.location);
        context.counters.work(context.limits, context.location)?;
        match self.current.as_ref() {
            Some(rows) => rows.replay(owner, producer, support, bodies),
            None => Ok(false),
        }
    }

    pub(super) fn capture(
        &mut self,
        owner: &mut SourceEligibility,
        producer: Producer<'source, '_>,
        cursor: Join<'query, 'source>,
        recording: Recording<'source>,
        bodies: &mut SourceBodies<'_, '_, 'source, '_>,
    ) -> Result<(), FormulaFailure> {
        let context = bodies.context(producer.position.1.location);
        self.admit(producer.position.0, context)?;
        self.current
            .as_mut()
            .expect("first eligible capture admitted cells")
            .capture(owner, producer, cursor, recording, bodies)
    }

    fn admit(
        &mut self,
        rule: usize,
        context: Context<'_, '_, 'source>,
    ) -> Result<(), FormulaFailure> {
        context.counters.work(context.limits, context.location)?;
        if self.current.is_none() {
            #[cfg(test)]
            FIRST_ALLOCATION.with(|first| first.set(Some((rule, self.rules))));
            let _ = rule;
            *self.current = Some(Rows::new(self.rules, context)?);
        }
        Ok(())
    }
}

pub(super) struct Rows<'query, 'source> {
    entries: Buffer<Option<Family<'query, 'source>>>,
    _lease: StorageLease,
}

impl<'query, 'source: 'query> Rows<'query, 'source> {
    fn new(rules: usize, context: Context<'_, '_, 'source>) -> Result<Self, FormulaFailure> {
        let Context {
            computation,
            limits,
            counters,
            location,
            ..
        } = context;
        let mut lease = computation.lease();
        let header =
            size_of::<Option<Self>>() - size_of::<Buffer<Option<Family<'query, 'source>>>>();
        lease.observe(header, location)?;
        computation.storage_observed(&lease, 0, header, limits, counters, location)?;
        let mut entries = Buffer::new(computation, limits, counters, location)?;
        entries.reserve(rules, computation, limits, counters, location)?;
        for _ in 0..rules {
            entries.push(None, computation, limits, counters, location)?;
        }
        Ok(Self {
            entries,
            _lease: lease,
        })
    }

    pub(super) fn replay(
        &self,
        owner: &mut SourceEligibility,
        producer: Producer<'source, '_>,
        support: &'query Support<'source>,
        bodies: &mut SourceBodies<'_, '_, 'source, '_>,
    ) -> Result<bool, FormulaFailure> {
        let location = producer.position.1.location;
        let context = bodies.context(location);
        context.counters.work(context.limits, location)?;
        let Some(family) = self
            .entries
            .slice()
            .get(producer.position.0)
            .ok_or_else(|| invalid(location))?
            .as_ref()
        else {
            return Ok(false);
        };
        let plan = family.view(
            producer.position.1,
            support,
            WorkContext::new(
                &*context.computation,
                context.limits,
                context.counters,
                location,
            ),
        )?;
        let mut replay = Replay::new(
            &plan,
            support,
            WorkContext::new(
                &*context.computation,
                context.limits,
                context.counters,
                location,
            ),
        )?;
        let mut rows = replay.retained(family, context.limits, context.counters)?;
        let mut binding = None;
        #[cfg(test)]
        HITS.with(|hits| hits.set(hits.get() + 1));
        while let Some(row) = {
            let context = bodies.context(location);
            rows.next(WorkContext::new(
                &*context.computation,
                context.limits,
                context.counters,
                location,
            ))?
        } {
            let context = bodies.context(location);
            if binding.is_none() {
                binding = Some(RowBinding::new(
                    &plan,
                    &row,
                    WorkContext::new(
                        &*context.computation,
                        context.limits,
                        context.counters,
                        location,
                    ),
                )?);
            }
            let value = binding
                .as_mut()
                .expect("first row initialized assignment")
                .read(
                    &row,
                    WorkContext::new(
                        &*context.computation,
                        context.limits,
                        context.counters,
                        location,
                    ),
                )?;
            // A new logical consumer visit keeps the ordinary complete-binding
            // admission, while no fictional Join probes are charged.
            context.counters.substitution(context.limits, location)?;
            owner.rule_binding(producer, value, bodies)?;
        }
        Ok(true)
    }

    pub(super) fn capture(
        &mut self,
        owner: &mut SourceEligibility,
        producer: Producer<'source, '_>,
        mut cursor: Join<'query, 'source>,
        mut recording: Recording<'source>,
        bodies: &mut SourceBodies<'_, '_, 'source, '_>,
    ) -> Result<(), FormulaFailure> {
        let location = producer.position.1.location;
        {
            let context = bodies.context(location);
            cursor.retain_witnesses(WorkContext::new(
                &*context.computation,
                context.limits,
                context.counters,
                location,
            ))?;
        }
        while let Some(row) = {
            let context = bodies.context(location);
            cursor.next_matched_row(
                context.budget,
                WorkContext::new(
                    context.computation,
                    context.limits,
                    context.counters,
                    location,
                ),
            )?
        } {
            if !row.passes {
                continue;
            }
            let witness = row.witnesses.as_ref().ok_or_else(|| invalid(location))?;
            {
                let context = bodies.context(location);
                recording.stage(
                    witness,
                    WorkContext::new(
                        &*context.computation,
                        context.limits,
                        context.counters,
                        location,
                    ),
                )?;
            }
            // Preserve first-pass join/consumer interleaving and source errors.
            owner.rule_binding(producer, &row.values, bodies)?;
        }
        let context = bodies.context(location);
        let family = recording.finish(
            cursor,
            WorkContext::new(
                &*context.computation,
                context.limits,
                context.counters,
                location,
            ),
        )?;
        context.counters.work(context.limits, location)?;
        let entry = self
            .entries
            .slice_mut()
            .get_mut(producer.position.0)
            .ok_or_else(|| invalid(location))?;
        if entry.is_some() {
            return Err(invalid(location));
        }
        // The cell's whole inline family header was reserved before capture.
        *entry = Some(family.embedded(location)?);
        #[cfg(test)]
        CAPTURES.with(|captures| captures.set(captures.get() + 1));
        Ok(())
    }
}

fn invalid(location: crate::ProgramSite) -> FormulaFailure {
    FormulaFailure::SupportRelation {
        error: zetesis_core::relation::Failure::Owner,
        location,
    }
}

#[cfg(test)]
thread_local! {
    static ENABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    static HITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static CAPTURES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static FIRST_ALLOCATION: std::cell::Cell<Option<(usize, usize)>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(super) fn enabled() -> bool {
    ENABLED.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(super) fn controlled<T>(enabled: bool, operation: impl FnOnce() -> T) -> (T, usize, usize) {
    struct Reset(bool, usize, usize);
    impl Drop for Reset {
        fn drop(&mut self) {
            ENABLED.with(|flag| flag.set(self.0));
            HITS.with(|count| count.set(self.1));
            CAPTURES.with(|count| count.set(self.2));
        }
    }
    let _reset = Reset(
        ENABLED.with(|flag| flag.replace(enabled)),
        HITS.with(|count| count.replace(0)),
        CAPTURES.with(|count| count.replace(0)),
    );
    let result = operation();
    (
        result,
        HITS.with(std::cell::Cell::get),
        CAPTURES.with(std::cell::Cell::get),
    )
}

#[cfg(test)]
fn allocation<T>(operation: impl FnOnce() -> T) -> (T, Option<(usize, usize)>) {
    struct Reset(Option<(usize, usize)>);
    impl Drop for Reset {
        fn drop(&mut self) {
            FIRST_ALLOCATION.with(|first| first.set(self.0));
        }
    }
    let _reset = Reset(FIRST_ALLOCATION.with(|first| first.replace(None)));
    let value = operation();
    (value, FIRST_ALLOCATION.with(std::cell::Cell::get))
}
