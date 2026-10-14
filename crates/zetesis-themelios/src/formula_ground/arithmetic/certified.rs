//! A whole flat original family can prove that its evidence scan has no error
//! or warning to discover. Total expressions do not prove a binding exists:
//! no defined witness is invented, and evidence cursors remain unselected.
//!
//! Each fragment borrows the completed support and owns only its ordinary leased
//! join/projection scratch, released before the next sibling. One decline keeps
//! the original full-family traversal; work, storage and cancellation refusals
//! propagate. No proof or lookup survives this operation.

#[cfg(test)]
mod tests;

use super::{
    Budget, Computation, Counters, FormulaFailure, FormulaLimits, Join, Prepared, ProgramSite,
    Support,
};

pub(super) fn family(
    prepared: &Prepared,
    location: ProgramSite,
    support: &Support<'_>,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    #[cfg(test)]
    if tests::disabled() {
        return Ok(false);
    }
    // A fact holds no arithmetic: facts with evaluated arguments stay rules.
    for rule in prepared
        .rules_without_facts()
        .iter()
        .chain(&prepared.projection)
    {
        counters.work(limits, location)?;
        if rule.location != location {
            continue;
        }
        let mut join = Join::rule(rule, support, computation, limits, budget, counters)?;
        if !join.total_family_fragment(rule, computation, limits, counters)? {
            return Ok(false);
        }
        #[cfg(test)]
        tests::proved();
    }
    counters.work(limits, location)?;
    Ok(true)
}
