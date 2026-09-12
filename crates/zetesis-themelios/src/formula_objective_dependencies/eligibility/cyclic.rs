//! Finite cyclic cones use complete possible support as a certificate.
//! No model search or least-required solver runs here. Optional covers either
//! truth value; only atoms outside completed support are classified absent.
//! Unresolved dependants use the same conservative carrier. The enclosing
//! constructor requires `CompletedQueries`, so aggregate proposals and scoped
//! producers share the rows used by final grounding only after a full fixed
//! point. Aggregate equality and conditional truth remain in the original
//! theory, never in this source-activity lookup.

use std::collections::BTreeSet;
use themelios_program::symbol::Signature;

use super::{Context, SourceEligibility, signature};
use crate::FormulaFailure;
use crate::formula_support::Support;

impl SourceEligibility {
    pub(super) fn cyclic(
        &mut self,
        support: &Support<'_>,
        unresolved: &BTreeSet<Signature>,
        temporary: usize,
        context: &mut Context<'_>,
    ) -> Result<(), FormulaFailure> {
        for predicate in support.predicates() {
            context.work()?;
            if unresolved.contains(&signature(predicate)) {
                self.possible(predicate, support, temporary, context)?;
            }
        }
        Ok(())
    }
}
