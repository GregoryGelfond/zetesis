//! Completed possible relation rows supply identities, without copying payload.
//! Membership covers possible truth and never establishes realizability.

use super::{Activity, Context, SourceEligibility, authority::Round};
use crate::FormulaFailure;
use crate::formula_support::Support;

impl SourceEligibility {
    pub(super) fn possible(
        &mut self,
        predicate: zetesis_core::catalog::PredicateRef<'_>,
        support: &Support<'_>,
        temporary: usize,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<(), FormulaFailure> {
        self.round = Some(Round::new(context));
        let result = self
            .possible_into_round(
                predicate,
                support,
                temporary.saturating_add(self.activity.len()),
                context,
            )
            .and_then(|()| self.publish_predicate(context));
        self.round = None;
        result
    }

    pub(super) fn possible_into_round(
        &mut self,
        predicate: zetesis_core::catalog::PredicateRef<'_>,
        support: &Support<'_>,
        temporary: usize,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<(), FormulaFailure> {
        context.work()?;
        for row in support.rows(predicate) {
            context.work()?;
            self.retain_atom(row.atom(), Activity::Optional, temporary, context)?;
        }
        Ok(())
    }
}
