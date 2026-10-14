//! Exact consequence scan under a complete signed acyclic source certificate.
//! The activity cells and canonical source identities are the same ones used
//! by source eligibility; only this stronger certificate permits exact output.

use super::{Activity, Context, SourceEligibility};
use crate::formula_facts::{FactBlock, Item};
use crate::formula_ir::HeadIr;
use crate::formula_support::{AcyclicPlan, Join, SourceAtom, SourceSelection, StorageLease};
use crate::grounding_observer::Profile;
use crate::{FormulaFailure, FormulaResource, GroundingPhase};

pub(crate) struct Exact {
    pub(crate) atoms: SourceSelection,
    pub(crate) selected: Vec<usize>,
    pub(crate) consistent: bool,
    pub(crate) lease: StorageLease,
}

impl SourceEligibility {
    // The private AcyclicPlan is the sole caller which enables this mode.
    // Direct publication is safe because a producer reads only strictly earlier
    // predicates. Alternative producers still combine through the same max.
    pub(super) fn stage_exact(
        &mut self,
        atom: &SourceAtom,
        activity: Activity,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<(), FormulaFailure> {
        let (id, _) = self.atoms.insert(
            atom,
            (FormulaResource::Atoms, context.limits.theory.max_atoms),
            context.computation,
            context.limits,
            context.counters,
            context.location,
        )?;
        let additional = self.atoms.len().saturating_sub(self.activity.len());
        crate::formula_support::reserve(
            &mut self.activity,
            additional,
            &mut self.lease,
            Self::header_bytes(),
            crate::formula_support::Context::new(
                &*context.computation,
                context.limits,
                context.counters,
                context.location,
            ),
        )?;
        context
            .counters
            .charge_work(additional as u128 + 1, context.limits, context.location)?;
        self.activity.resize(self.atoms.len(), Activity::Absent);
        self.activity[id] = self.activity[id].max(activity);
        Ok(())
    }
}

pub(crate) fn evaluate_acyclic(
    plan: &AcyclicPlan,
    source: &crate::formula_ir::Prepared,
    support: &crate::formula_support::Support<'_>,
    profile: &Profile<'_>,
    context: &mut Context<'_, '_, '_>,
) -> Result<Exact, FormulaFailure> {
    let mut result = SourceEligibility::new(context)?;
    result.exact = true;
    let mut consistent = true;
    for &(_, position) in plan.items() {
        match source.items().get(position) {
            Item::Rule(_, rule) => {
                context.location = rule.location;
                consistent &= profile.phase(
                    GroundingPhase::RuleInstantiation,
                    Some(rule.location),
                    || result.rule_exact(rule, support, context),
                )?;
            }
            Item::Facts(block) => {
                context.location = block.location();
                profile.phase(
                    GroundingPhase::RuleInstantiation,
                    Some(block.location()),
                    || result.facts_exact(block, context),
                )?;
            }
        }
    }
    consistent &= profile.phase(GroundingPhase::Coherence, None, || result.coherent(context))?;
    result.into_exact(consistent, context)
}

impl SourceEligibility {
    /// Stage each fact row `Required`: a body-free rule's one passing binding
    /// has that activity, since the fold over no literal starts from it.
    fn facts_exact(
        &mut self,
        block: FactBlock<'_>,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<(), FormulaFailure> {
        let mut head = crate::formula_support::facts::FactHead::new(
            block,
            context.computation,
            context.limits,
            context.counters,
            context.location,
        )?;
        for row in block.rows() {
            context.location = row.location();
            let atom = head.atom(row, context.computation, context.limits, context.counters)?;
            self.stage_exact(&atom, Activity::Required, context)?;
        }
        Ok(())
    }

    fn rule_exact(
        &mut self,
        rule: &crate::formula_ir::RuleIr,
        support: &crate::formula_support::Support<'_>,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<bool, FormulaFailure> {
        let mut consistent = true;
        let mut join = Join::rule(
            rule,
            support,
            context.computation,
            context.limits,
            context.budget,
            context.counters,
        )?;
        // This is the maintained complete-column selection, after independent
        // original-family admission. Rejected rows still validate their keys.
        join.select_total_body(rule, context.computation, context.limits, context.counters)?;
        while let Some(row) = join.next_row(
            context.computation,
            context.limits,
            context.budget,
            context.counters,
            rule.location,
        )? {
            if !row.passes {
                crate::formula_ground::validate_atom_keys(
                    &rule.body,
                    &row.values,
                    context.computation,
                    context.limits,
                    context.counters,
                    rule.location,
                    |variable, location| FormulaFailure::UnsafeVariable { variable, location },
                )?;
                continue;
            }
            let body = self.fold_activity(&rule.body, &row.values, true, context)?;
            match &rule.head {
                HeadIr::Normal(Some(head)) => {
                    self.retain(*head, &row.values, body, 0, context)?;
                }
                HeadIr::Normal(None) => consistent &= body != Activity::Required,
                _ => unreachable!("complete normal source certificate"),
            }
        }
        Ok(consistent)
    }

    fn coherent(&self, context: &mut Context<'_, '_, '_>) -> Result<bool, FormulaFailure> {
        let mut consistent = true;
        for (id, activity) in self.activity.iter().enumerate() {
            context.work()?;
            if *activity != Activity::Required {
                continue;
            }
            let atom = self
                .atoms
                .source(id, context.limits, context.counters, context.location)?;
            let value = context.computation.source_atom(
                &atom,
                context.limits,
                context.counters,
                context.location,
            )?;
            let opposite = match value.predicate().sign() {
                zetesis_core::Sign::Positive => zetesis_core::Sign::Negative,
                zetesis_core::Sign::Negative => zetesis_core::Sign::Positive,
            };
            if let Some(other) = context.computation.signed(
                &atom,
                opposite,
                context.limits,
                context.counters,
                context.location,
            )? {
                consistent &= self.lookup(&other, context)? != Activity::Required;
            }
        }
        Ok(consistent)
    }

    fn into_exact(
        self,
        consistent: bool,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<Exact, FormulaFailure> {
        let mut selected = Vec::new();
        let mut lease = context.computation.lease();
        let header = size_of::<Exact>() - size_of::<SourceSelection>();
        lease.observe(header, context.location)?;
        for (id, activity) in self.activity.iter().enumerate() {
            context.work()?;
            if *activity != Activity::Required {
                continue;
            }
            crate::formula_support::reserve(
                &mut selected,
                1,
                &mut lease,
                header,
                crate::formula_support::Context::new(
                    &*context.computation,
                    context.limits,
                    context.counters,
                    context.location,
                ),
            )?;
            selected.push(id);
        }
        Ok(Exact {
            atoms: self.atoms,
            selected,
            consistent,
            lease,
        })
    }
}
