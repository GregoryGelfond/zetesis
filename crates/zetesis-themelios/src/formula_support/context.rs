//! An operation's authority and cumulative grounding resource account.

use super::Counters;
use crate::FormulaLimits;
use crate::ProgramSite;

/// Borrow one grounding operation's limits, accounting and diagnostic location.
/// Cancellation, shared allowance and observation remain in the same `Counters`;
/// this view neither resets them nor owns retained workspace. Expansion `Budget`
/// is independent and is passed only to operations which consume it.
pub(crate) struct GroundingWork<'a> {
    pub(crate) limits: &'a FormulaLimits,
    pub(crate) counters: &'a mut Counters,
    pub(crate) location: ProgramSite,
}
impl<'a> GroundingWork<'a> {
    pub(crate) fn new(
        limits: &'a FormulaLimits,
        counters: &'a mut Counters,
        location: ProgramSite,
    ) -> Self {
        Self {
            limits,
            counters,
            location,
        }
    }
}

/// Pair a concrete read or append capability with its operation's resource
/// account. `C` is a borrowed `Computation`, shared for read-only preparation and
/// exclusive for generated terms. No join or binding retains this context.
pub(crate) struct Context<'a, C> {
    pub(crate) computation: C,
    pub(crate) work: GroundingWork<'a>,
}
impl<'a, C> Context<'a, C> {
    pub(crate) fn new(
        computation: C,
        limits: &'a FormulaLimits,
        counters: &'a mut Counters,
        location: ProgramSite,
    ) -> Self {
        Self {
            computation,
            work: GroundingWork::new(limits, counters, location),
        }
    }
}
