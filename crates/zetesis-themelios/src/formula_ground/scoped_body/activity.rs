//! Source coverage over one append-only formula owner and immutable atom view.
//!
//! A completed prefix retains the same three-valued fold used by fresh scoped
//! bodies. Native threshold guards share the lower/upper reduction of their
//! exact numeric row; repeated conditions remain independent contributions.
//! Neither cache compares source syntax or infers correlated satisfiability.
//! The caller drops this owner before changing any atom's source activity or
//! replacing a formula/catalog prefix. Appending preserves all existing IDs.

use zetesis_ferraris::{FormulaParts, NodeView, Threshold};

use crate::FormulaFailure;
use crate::formula_ground::atoms::Catalog;
use crate::formula_source_activity::{Activity, Context, SourceEligibility};
use crate::formula_support::{Buffer, StorageLease};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
struct Bounds {
    lower: i128,
    upper: i128,
}

/// The two buffer leases own their headers and capacities. This owner's lease
/// accounts for the remaining header, including the completed-prefix cursor.
pub(super) struct Evaluation {
    values: Buffer<Activity>,
    rows: Buffer<Option<Bounds>>,
    completed: usize,
    _lease: StorageLease,
}

impl Evaluation {
    pub(super) fn new(context: &mut Context<'_, '_, '_>) -> Result<Self, FormulaFailure> {
        let values = Buffer::new(
            context.computation,
            context.limits,
            context.counters,
            context.location,
        )?;
        let rows = Buffer::new(
            context.computation,
            context.limits,
            context.counters,
            context.location,
        )?;
        let mut lease = context.computation.lease();
        let header =
            size_of::<Self>() - size_of::<Buffer<Activity>>() - size_of::<Buffer<Option<Bounds>>>();
        lease.observe(header, context.location)?;
        context.computation.storage_observed(
            &lease,
            0,
            header,
            context.limits,
            context.counters,
            context.location,
        )?;
        Ok(Self {
            values,
            rows,
            completed: 0,
            _lease: lease,
        })
    }

    /// Extend the cached fold through `root`. Formula children precede parents,
    /// and the owning builder has already validated every native numeric row.
    /// A refused node leaves `completed` at the preceding successful node; its
    /// reserved storage and performed work remain accounted for.
    pub(super) fn evaluate(
        &mut self,
        parts: &FormulaParts,
        atoms: &Catalog,
        root: usize,
        eligibility: &SourceEligibility,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<Activity, FormulaFailure> {
        self.evaluate_with(parts, root, context, |atom, context| {
            let source = atoms.source(atom, context.counters, context.limits, context.location)?;
            eligibility.source_activity(&source, context)
        })
    }

    fn evaluate_with(
        &mut self,
        parts: &FormulaParts,
        root: usize,
        context: &mut Context<'_, '_, '_>,
        mut atom: impl FnMut(usize, &mut Context<'_, '_, '_>) -> Result<Activity, FormulaFailure>,
    ) -> Result<Activity, FormulaFailure> {
        // Cached roots must still observe cancellation and admit their lookup.
        context.counters.poll(context.location)?;
        context.counters.work(context.limits, context.location)?;
        let count = root.checked_add(1).expect("owned formula index fits usize");
        if count > self.values.len() {
            self.values.resize(
                count,
                Activity::Absent,
                context.computation,
                context.limits,
                context.counters,
                context.location,
            )?;
        }
        if parts.rows().len() > self.rows.len() {
            self.rows.resize(
                parts.rows().len(),
                None,
                context.computation,
                context.limits,
                context.counters,
                context.location,
            )?;
        }
        let view = parts.view();
        for index in self.completed..count {
            context.counters.work(context.limits, context.location)?;
            let node = view.node(index).expect("owned formula node");
            let activity = match node {
                NodeView::False => Activity::Absent,
                NodeView::Atom(id) => atom(id, context)?,
                NodeView::And(row) | NodeView::Or(row) => {
                    let conjunction = matches!(node, NodeView::And(_));
                    let mut activity = if conjunction {
                        Activity::Required
                    } else {
                        Activity::Absent
                    };
                    for &child in row {
                        context.counters.work(context.limits, context.location)?;
                        activity = if conjunction {
                            activity.min(self.values.slice()[child])
                        } else {
                            activity.max(self.values.slice()[child])
                        };
                    }
                    activity
                }
                NodeView::Threshold(threshold) => {
                    self.threshold(parts, index, threshold, context)?
                }
                NodeView::ExtremumCut(cut) => {
                    let mut activity = match cut.endpoint_truth() {
                        Some(true) => Activity::Required,
                        Some(false) | None => Activity::Absent,
                    };
                    if cut.endpoint_truth().is_none() {
                        for (&child, &rank) in cut.row.conditions.iter().zip(cut.row.ranks) {
                            context.counters.work(context.limits, context.location)?;
                            if cut.qualifies(rank) {
                                activity = activity.max(self.values.slice()[child]);
                            }
                        }
                    }
                    activity
                }
                NodeView::Implies(left, right) => {
                    context
                        .counters
                        .charge_work(2, context.limits, context.location)?;
                    self.values.slice()[left]
                        .negate()
                        .max(self.values.slice()[right])
                }
            };
            self.values.slice_mut()[index] = activity;
            self.completed = index + 1;
        }
        Ok(self.values.slice()[root])
    }

    fn threshold(
        &mut self,
        parts: &FormulaParts,
        index: usize,
        threshold: Threshold<'_>,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<Activity, FormulaFailure> {
        // Descriptor, source row and memo reads preserve exact physical row
        // identity even when appending moves the underlying allocations.
        context
            .counters
            .charge_work(3, context.limits, context.location)?;
        let descriptor = parts.nodes()[index]
            .threshold_descriptor()
            .expect("threshold node has a descriptor");
        let row = parts.thresholds()[descriptor].row;
        let bounds = if let Some(bounds) = self.rows.slice()[row] {
            bounds
        } else {
            let mut bounds = Bounds { lower: 0, upper: 0 };
            for (&condition, &weight) in threshold.conditions.iter().zip(threshold.weights) {
                context.counters.work(context.limits, context.location)?;
                let activity = self.values.slice()[condition];
                // Admission proves the whole nonnegative row fits i128, so
                // either selected subtotal does too. Zeroes and repetitions
                // are visited with the same meaning as the uncached fold.
                if activity == Activity::Required {
                    bounds.lower += i128::from(weight);
                }
                if activity != Activity::Absent {
                    bounds.upper += i128::from(weight);
                }
            }
            context.counters.work(context.limits, context.location)?;
            self.rows.slice_mut()[row] = Some(bounds);
            bounds
        };
        if bounds.lower >= threshold.bound {
            Ok(Activity::Required)
        } else if bounds.upper < threshold.bound {
            Ok(Activity::Absent)
        } else {
            Ok(Activity::Optional)
        }
    }
}
