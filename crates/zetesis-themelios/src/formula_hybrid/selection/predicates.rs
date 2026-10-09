//! Predicate windows lend the existing catalog orders to one source body.

use super::{CompletedSupport, Counters, FormulaFailure, FormulaLimits, reserve};
use crate::formula_hybrid::literal_atom;
use crate::formula_ir::{LiteralIr, RuleIr};
use zetesis_core::{AtomLookup, PredicateLookup};

/// Source occurrences and catalog views share their external immutable owners.
/// This contains no region truth, binding, row cursor or copied atom index.
pub(in crate::formula_hybrid) struct RulePredicates<'source> {
    body: &'source [LiteralIr],
    predicates: Vec<Option<PredicateLookup<'source, 'source, 'source>>>,
}

impl<'source> RulePredicates<'source> {
    pub(in crate::formula_hybrid) fn slots(
        rules: &[RuleIr],
        support: &mut CompletedSupport<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Vec<Option<Self>>, FormulaFailure> {
        let location = rules.first().expect("nonempty streamed source").location;
        let mut slots = Vec::new();
        let mut bytes = 0;
        reserve(
            &mut slots,
            rules.len(),
            &mut bytes,
            support,
            limits,
            counters,
            location,
        )?;
        for rule in rules {
            counters.work(limits, rule.location)?;
            slots.push(None);
        }
        support.retain_workspace(usize::try_from(bytes).expect("admitted support bytes fit usize"));
        Ok(slots)
    }

    pub(in crate::formula_hybrid) fn new(
        rule: &'source RuleIr,
        index: AtomLookup<'source, 'source>,
        support: &mut CompletedSupport<'source>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Self, FormulaFailure> {
        let mut predicates = Vec::new();
        // The outer slot already accounts for this header. Admit only the
        // occurrence buffer here; on refusal no buffer enters the ledger.
        let mut bytes = 0;
        reserve(
            &mut predicates,
            rule.body.len(),
            &mut bytes,
            support,
            limits,
            counters,
            rule.location,
        )?;
        for literal in &rule.body {
            counters.work(limits, rule.location)?;
            let selected = if let Some((_, pattern)) = literal_atom(literal) {
                let components = support
                    .components()
                    .ok_or_else(|| crate::formula_support::components::missing(rule.location))?;
                let pattern = pattern.get(components, limits, counters, rule.location)?;
                Some(index.prepare_predicate_with(pattern.predicate(), || {
                    counters.work(limits, rule.location)
                })?)
            } else {
                None
            };
            predicates.push(selected);
        }
        support.retain_workspace(usize::try_from(bytes).expect("admitted support bytes fit usize"));
        Ok(Self {
            body: &rule.body,
            predicates,
        })
    }

    /// A foreign/reordered body cannot reuse occurrence authority. The caller
    /// keeps the ordinary full lookup for a body not paired with this plan.
    pub(in crate::formula_hybrid) fn at(
        &self,
        body: &[LiteralIr],
        occurrence: usize,
    ) -> Option<PredicateLookup<'source, 'source, 'source>> {
        std::ptr::eq(self.body, body)
            .then(|| self.predicates.get(occurrence).copied().flatten())
            .flatten()
    }
}
