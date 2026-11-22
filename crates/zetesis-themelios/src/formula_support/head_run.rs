//! Runs of ground fact rules of one predicate, admitted as one head block.
//!
//! A program without fact blocks states its facts as body-less rules. Where
//! such rules follow one another with one head predicate, the first round
//! prepares their heads at the owner once, as a fact block does, instead of
//! preparing each rule's head pattern on its own.

use crate::formula_ir::{HeadIr, RuleIr};
use crate::{FormulaFailure, FormulaLimits};

use super::components::Pattern;
use super::{Computation, Counters, Support, SupportAppend};

/// The open run, if any: the rule visited last and the run's end.
#[derive(Default)]
pub(super) struct HeadRun {
    open: Option<(usize, usize)>,
}

/// A ground, body-less rule's single head.
pub(super) fn fact_head(rule: &RuleIr) -> Option<Pattern> {
    match rule.head {
        HeadIr::Normal(Some(head)) if rule.body.is_empty() && rule.variables == 0 => Some(head),
        _ => None,
    }
}

impl HeadRun {
    /// Continue the open run at rule `index`, or end it and open the run that
    /// starts there, when two or more fact rules of one predicate do.
    pub(super) fn before_rule<'source>(
        &mut self,
        index: usize,
        rules: &'source [RuleIr],
        delta: &mut SupportAppend<'source>,
        support: &Support<'source>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        if let Some((last, end)) = self.open {
            if index == last + 1 && index < end {
                self.open = Some((index, end));
                delta.expect_head(index);
                return Ok(());
            }
            self.close(delta, support, limits, counters, rules[index].location)?;
        }
        let rule = &rules[index];
        let Some(head) = fact_head(rule) else {
            return Ok(());
        };
        let computation = Computation::new(delta, support);
        let view = computation.static_components(rule.location)?;
        let predicate = head.get(view, limits, counters, rule.location)?.predicate();
        let mut end = index + 1;
        while let Some(next) = rules.get(end).and_then(fact_head) {
            counters.work(limits, rule.location)?;
            let other = next.get(view, limits, counters, rule.location)?.predicate();
            if !other.equals_ref_with(predicate, || counters.work(limits, rule.location))? {
                break;
            }
            end += 1;
        }
        if end - index < 2 {
            return Ok(());
        }
        delta.open_head_block(
            predicate,
            end - index,
            support.workspace_bytes(),
            crate::formula_support::GroundingWork::new(limits, counters, rule.location),
        )?;
        self.open = Some((index, end));
        delta.expect_head(index);
        Ok(())
    }

    /// End the open run, if any.
    pub(super) fn close<'source>(
        &mut self,
        delta: &mut SupportAppend<'source>,
        support: &Support<'source>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: crate::ProgramSite,
    ) -> Result<(), FormulaFailure> {
        if self.open.take().is_some() {
            delta.close_head_block(
                support.workspace_bytes(),
                crate::formula_support::GroundingWork::new(limits, counters, location),
            )?;
        }
        Ok(())
    }
}
