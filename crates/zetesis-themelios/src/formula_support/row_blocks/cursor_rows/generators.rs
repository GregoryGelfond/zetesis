//! A rule scan needs a second binding generator before this private staging.
//!
//! This is the same initial-prefix distinction used by `order::is_test`: a
//! positive occurrence whose variables are already bound is a test, not a
//! generator. The admitted descriptor has no initial bound slots. We need only
//! the first generator's variables: finding a later new slot proves a second
//! generator and ends the check. No binding, row or original plan is changed.

#[cfg(test)]
mod tests;

use super::{Counters, FormulaFailure, FormulaLimits, ProgramSite, order};
use crate::formula_support::PositivePattern;
use zetesis_core::{PatternRef, TemplateTerm};

pub(super) fn expands(
    plan: &order::Plan<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    let mut first = None;
    for occurrence in &plan.patterns {
        counters.work(limits, location)?;
        let PositivePattern::Flat(pattern) = occurrence.pattern else {
            // The preceding source certificate normally declines this shape.
            // It cannot establish this narrower flat-prefix classification.
            return Ok(false);
        };
        let terms = pattern.terms();
        for column in 0..terms.len() {
            counters.work(limits, location)?;
            let TemplateTerm::Variable(variable) = terms.at(column).expect("checked arity") else {
                continue;
            };
            if let Some(generator) = first {
                if !contains(generator, variable, limits, counters, location)? {
                    return Ok(true);
                }
            } else {
                first = Some(pattern);
                break;
            }
        }
    }
    Ok(false)
}

fn contains(
    pattern: PatternRef<'_>,
    variable: usize,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    let terms = pattern.terms();
    for column in 0..terms.len() {
        counters.work(limits, location)?;
        if matches!(terms.at(column), Some(TemplateTerm::Variable(found)) if found == variable) {
            return Ok(true);
        }
    }
    Ok(false)
}
