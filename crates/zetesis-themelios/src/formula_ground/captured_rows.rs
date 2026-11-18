//! Passing rows of a captured constraint template, kept so that retention can
//! emit the template's instances without joining its body a second time.
//!
//! Capture joins every streamable template once to count its instances and to
//! retain the atoms they read. Retention, after completion, grounds in the core
//! the templates whose instances fit. Those instances are exactly the passing
//! rows capture visited, in the same order: both traversals build the same
//! join over the same immutable support, and retention skips failing rows that
//! capture has already validated. Replaying the captured bindings therefore
//! emits the same instances in the same order, through the same `Builder::rule`.
//!
//! Rows are kept only while retention could still take the template: while
//! the instance bound stays within the retention share not already promised to
//! an earlier recorded template, while the theory's ceilings (which only fill
//! as grounding proceeds) still hold it, and while the frame's growth stays
//! within its storage allowance. Otherwise the rows are dropped and retention
//! joins the template as before, so recording never causes a refusal. Copied
//! slots and frame growth are charged as ordinary work and storage.

#[cfg(test)]
pub(super) mod testing;
#[cfg(test)]
mod tests;

use super::{Builder, InstanceBound};
use crate::formula_binding::Binding;
use crate::formula_ir::RuleIr;
use crate::formula_support::{GroundingWork, Support};
use crate::{FormulaFailure, ProgramSite};

/// The passing rows of one template, `width` slots per row, in visit order.
pub(super) struct CapturedRows {
    width: usize,
    rows: u64,
    /// Created at the first passing row; a template without instances keeps none.
    frame: Option<Binding<'static>>,
}

/// Recording state for one template during capture.
pub(super) struct Recording {
    unit: InstanceBound,
    rows: Option<CapturedRows>,
}

impl Recording {
    /// Start recording unless retention grants no share at all.
    pub(super) fn start(rule: &RuleIr, share: u128) -> Self {
        #[cfg(test)]
        if !testing::enabled() {
            return Self::stopped(rule);
        }
        if share == 0 {
            return Self::stopped(rule);
        }
        Self {
            unit: InstanceBound::of(rule),
            rows: Some(CapturedRows {
                width: 0,
                rows: 0,
                frame: None,
            }),
        }
    }

    fn stopped(rule: &RuleIr) -> Self {
        Self {
            unit: InstanceBound::of(rule),
            rows: None,
        }
    }

    /// The recorded rows when capture recorded every instance, promising
    /// their bound out of `share`.
    pub(super) fn finish(self, instances: u64, share: &mut u128) -> Option<CapturedRows> {
        let rows = self.rows?;
        debug_assert_eq!(rows.rows, instances, "every passing row is recorded");
        *share -= self.unit.times(instances).nodes;
        Some(rows)
    }
}

impl<'source> Builder<'_, '_, 'source> {
    /// Record one passing row, the `instances`-th, or stop recording when
    /// retention could no longer take this template within `share`.
    pub(super) fn record_row(
        &mut self,
        recording: &mut Recording,
        values: &Binding<'_>,
        instances: u64,
        share: u128,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let Some(rows) = &mut recording.rows else {
            return Ok(());
        };
        let bound = recording.unit.times(instances);
        let width = values.len();
        if rows.frame.is_none() {
            rows.width = width;
        }
        let end = usize::try_from(rows.rows)
            .ok()
            .and_then(|rows| rows.checked_add(1))
            .and_then(|rows| rows.checked_mul(width))
            .filter(|_| bound.nodes <= share && self.holds(bound) && width == rows.width);
        let Some(end) = end else {
            // Dropping the frame releases its storage.
            recording.rows = None;
            return Ok(());
        };
        let frame = match &mut rows.frame {
            Some(frame) => frame,
            None => rows.frame.insert(Binding::new(
                self.computation,
                self.limits,
                self.counters,
                location,
            )?),
        };
        if !frame.admits_scope(end, self.computation, self.limits, location) {
            recording.rows = None;
            return Ok(());
        }
        frame.extend_scope(end, self.computation, self.limits, self.counters, location)?;
        let base = end - width;
        for slot in 0..width {
            if values.is_bound(slot, location)? {
                frame.copy_slot(
                    base + slot,
                    values,
                    slot,
                    &mut GroundingWork::new(self.limits, self.counters, location),
                )?;
            }
        }
        rows.rows += 1;
        Ok(())
    }

    /// Emit `rows`' instances of `rule` as retention's join would: the same
    /// bindings, in the same order, through `Builder::rule`.
    pub(super) fn replay_rows(
        &mut self,
        rule: &'source RuleIr,
        support: &Support<'source>,
        rows: CapturedRows,
    ) -> Result<(), FormulaFailure> {
        let location = rule.location;
        #[cfg(test)]
        testing::replayed();
        let Some(frame) = rows.frame else {
            return Ok(());
        };
        let atoms = self.catalog.len();
        let mut binding = Binding::new(self.computation, self.limits, self.counters, location)?;
        binding.extend_scope(
            rows.width,
            self.computation,
            self.limits,
            self.counters,
            location,
        )?;
        let mut base = 0;
        for _ in 0..rows.rows {
            self.budget.poll(location)?;
            for slot in 0..rows.width {
                if frame.is_bound(base + slot, location)? {
                    binding.copy_slot(
                        slot,
                        &frame,
                        base + slot,
                        &mut GroundingWork::new(self.limits, self.counters, location),
                    )?;
                } else {
                    binding.clear(slot, self.limits, self.counters, location)?;
                }
            }
            self.rule(rule, &binding, support, None, None)?;
            base += rows.width;
        }
        debug_assert_eq!(self.catalog.len(), atoms, "capture retained every atom");
        Ok(())
    }
}
