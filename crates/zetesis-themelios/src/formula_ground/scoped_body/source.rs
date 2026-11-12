//! Scoped lowering shared by one producer pass over completed support.
//!
//! A producer retains immutable formula and aggregate owners over completed
//! support. When activity updates are staged, completed readings also remain
//! reusable until the producer returns, before publication or refinement. Direct
//! exact publication instead discards the readings before each evaluation while
//! retaining the immutable lowering. Neither roots nor local atom IDs escape into
//! another builder. Ordinary bodies never initialize these scratch owners.

use super::activity::Evaluation;
use super::{Builder, Purpose};
use crate::FormulaFailure;
use crate::ProgramSite;
use crate::formula_binding::Binding;
use crate::formula_ir::LiteralIr;
use crate::formula_source_activity::{Activity, Context, SourceEligibility};
use crate::formula_support::{StorageLease, Support};

/// One producer's cumulative account and lazily constructed scoped formulas.
///
/// Exactly one of `unprepared` and `builder` owns the mutable account, except
/// during the fallible one-way preparation transition. A failed transition ends
/// the producer; completed work remains in the caller's borrowed counters.
/// The builder's existing aggregate identity and complete `family_inputs` key
/// distinguish its families. Every body still lowers its binding-dependent
/// residual literals; a shared aggregate never licenses whole-body reuse.
pub(crate) struct SourceBodies<'a, 'terms, 'source, 'support> {
    unprepared: Option<Context<'a, 'terms, 'source>>,
    builder: Option<Builder<'a, 'terms, 'source>>,
    evaluation: Option<Evaluation>,
    support: &'support Support<'source>,
    lease: StorageLease,
}

impl<'a, 'terms, 'source, 'support> SourceBodies<'a, 'terms, 'source, 'support> {
    pub(crate) fn new(
        context: Context<'a, 'terms, 'source>,
        support: &'support Support<'source>,
    ) -> Self {
        Self {
            lease: context.computation.lease(),
            unprepared: Some(context),
            builder: None,
            evaluation: None,
            support,
        }
    }

    /// The immutable support which owns every family in this producer pass.
    pub(crate) const fn support(&self) -> &'support Support<'source> {
        self.support
    }

    /// Reborrow the same cumulative account for joins and staged head updates.
    pub(crate) fn context(&mut self, location: ProgramSite) -> Context<'_, 'terms, 'source> {
        if let Some(builder) = &mut self.builder {
            Context {
                computation: builder.computation,
                limits: builder.limits,
                budget: builder.budget,
                counters: builder.counters,
                location,
            }
        } else {
            let context = self
                .unprepared
                .as_mut()
                .expect("the producer retains its unprepared account");
            Context {
                computation: context.computation,
                limits: context.limits,
                budget: context.budget,
                counters: context.counters,
                location,
            }
        }
    }

    fn prepare(&mut self, location: ProgramSite) -> Result<(), FormulaFailure> {
        if self.builder.is_some() {
            return Ok(());
        }
        let context = self
            .unprepared
            .take()
            .expect("one transition into the producer's scoped builder");
        // The builder's existing owners account themselves. Evaluation admits
        // its own header when initialized below; its dormant slot is not a
        // second live owner. This lease covers the wrapper's remaining state.
        let header = size_of::<Self>() - size_of::<Builder<'_, '_, '_>>() - size_of::<Evaluation>();
        self.lease.observe(header, location)?;
        context.computation.storage_observed(
            &self.lease,
            0,
            header,
            context.limits,
            context.counters,
            location,
        )?;
        let mut builder = Builder::empty(
            context.computation,
            context.limits,
            context.budget,
            context.counters,
            Purpose::Validation,
            None,
            location,
        )?;
        builder.initialize(location)?;
        self.builder = Some(builder);
        Ok(())
    }

    /// Lower this exact body, then read its root under the producer's snapshot.
    ///
    /// `stable_snapshot` is false for the exact evaluator, whose staging updates
    /// source activity immediately. It may reuse immutable lowering, but must
    /// begin a fresh reduction after every such update. The ordinary staged
    /// producer retains its reduction only until this owner is dropped.
    pub(crate) fn activity(
        &mut self,
        literals: &[LiteralIr],
        binding: &Binding,
        eligibility: &SourceEligibility,
        stable_snapshot: bool,
        location: ProgramSite,
    ) -> Result<Activity, FormulaFailure> {
        self.prepare(location)?;
        let builder = self.builder.as_mut().expect("scoped builder prepared");
        let root = builder.body(literals, binding, location, self.support)?;
        let mut context = Context {
            computation: builder.computation,
            limits: builder.limits,
            budget: builder.budget,
            counters: builder.counters,
            location,
        };
        if !stable_snapshot {
            self.evaluation = None;
        }
        if self.evaluation.is_none() {
            self.evaluation = Some(Evaluation::new(&mut context)?);
        }
        self.evaluation
            .as_mut()
            .expect("activity reduction prepared")
            .evaluate(
                builder.nodes.parts(),
                &builder.catalog,
                root,
                eligibility,
                &mut context,
            )
    }
}
