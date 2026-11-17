//! Exact consequence scan under a complete signed stratified source
//! certificate: recursive only through certain components.
//! The activity cells and canonical source identities are the same ones used
//! by source eligibility; only this stronger certificate permits exact output.

use super::{Activity, Context, SourceEligibility};
use crate::formula_facts::{FactBlock, Item};
use crate::formula_ir::{HeadIr, LiteralIr};
use crate::formula_support::{
    AcyclicPlan, Join, SourceAtom, SourceBatch, SourceSelection, StorageLease,
};
use crate::grounding_observer::Profile;
use crate::{FormulaFailure, FormulaResource, GroundingPhase};
use themelios_program::program::DefaultNegation;

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
    // Heads of the current saturated rank, published together when the
    // rank ends: no rule of a closed rank reads activity, and every later
    // rank starts after the publication.
    let mut closed: Option<(usize, SourceBatch)> = None;
    for &(rank, position) in plan.items() {
        if let Some((pending, _)) = &closed
            && *pending != rank
        {
            let (_, batch) = closed.take().expect("checked pending rank");
            result.publish_required(batch, context)?;
        }
        let item = source.items().get(position);
        context.location = match item {
            Item::Rule(_, rule) => rule.location,
            Item::Facts(block) => block.location(),
        };
        // A saturated rank's heads, fact rows included, are published
        // together when the rank ends, exactly as its fact rules' heads were.
        let batch = if plan.closed(rank) {
            if closed.is_none() {
                closed = Some((
                    rank,
                    SourceSelection::batch(
                        context.computation,
                        context.limits,
                        context.counters,
                        context.location,
                    )?,
                ));
            }
            closed.as_mut().map(|(_, batch)| batch)
        } else {
            None
        };
        match item {
            Item::Rule(index, rule) => {
                consistent &= profile.phase(
                    GroundingPhase::RuleInstantiation,
                    Some(rule.location),
                    || result.rule_exact((index, rule), batch, support, context),
                )?;
            }
            Item::Facts(block) => {
                profile.phase(
                    GroundingPhase::RuleInstantiation,
                    Some(block.location()),
                    || result.facts_exact(block, batch, context),
                )?;
            }
        }
    }
    if let Some((_, batch)) = closed {
        result.publish_required(batch, context)?;
    }
    consistent &= profile.phase(GroundingPhase::Coherence, None, || result.coherent(context))?;
    result.into_exact(consistent, context)
}

impl SourceEligibility {
    /// Publish the heads of one completed saturated rank `Required`.
    /// Already selected atoms are raised in place; new atoms take the next
    /// local coordinates, so the activity table grows by exactly that range.
    /// A refusal aborts the whole exact evaluation.
    fn publish_required(
        &mut self,
        batch: SourceBatch,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<(), FormulaFailure> {
        let Self {
            atoms,
            activity,
            lease,
            ..
        } = self;
        debug_assert_eq!(activity.len(), atoms.len());
        let before = activity.len();
        batch.merge_into(
            atoms,
            (FormulaResource::Atoms, context.limits.theory.max_atoms),
            crate::formula_support::Context::new(
                &*context.computation,
                context.limits,
                context.counters,
                context.location,
            ),
            |local, counters| {
                counters.work(context.limits, context.location)?;
                if let Some(cell) = activity.get_mut(local) {
                    *cell = Activity::Required;
                }
                Ok(())
            },
        )?;
        let additional = atoms.len() - before;
        crate::formula_support::reserve(
            activity,
            additional,
            lease,
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
        activity.resize(atoms.len(), Activity::Required);
        Ok(())
    }

    /// Stage each fact row `Required`: a body-free rule's one passing binding
    /// has that activity, since the fold over no literal starts from it. In a
    /// saturated rank (`closed` holds its pending heads), each row joins the
    /// rank's batch instead, as a fact rule's head does, and is published
    /// `Required` with the rank: one sorted merge rather than one ordered
    /// insertion per row.
    fn facts_exact(
        &mut self,
        block: FactBlock<'_>,
        mut closed: Option<&mut SourceBatch>,
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
            match closed.as_deref_mut() {
                Some(batch) => batch.push(
                    &atom,
                    context.computation,
                    context.limits,
                    context.counters,
                    context.location,
                )?,
                None => self.stage_exact(&atom, Activity::Required, context)?,
            }
        }
        Ok(())
    }

    /// Stage the heads of the rule at position `index` among the rules, and
    /// report whether no instance of it is a violated constraint. A row
    /// under which a decided default-negated atom fails is passed over as
    /// support completion passed over it
    /// (`formula_support::decided_negations_hold`): its body is absent, and
    /// its head, which no other row derives, lies outside the completed
    /// support and is not named here. The join has read the row's head
    /// arguments by then, so a fatal failure among them still refuses.
    ///
    /// In a saturated component (`closed` holds its pending heads), every
    /// passing row whose decided negations hold has a `Required` body without
    /// a fold: the join offers only completed-support rows, and completed
    /// support restricted to certain predicates is their least model. This is
    /// what lets such a component be recursive: a fold would read
    /// same-component atoms before their producers are staged. Its heads are
    /// published with the rank. Support completion must have decided every
    /// default negation of such a rule; otherwise the plan and the support
    /// disagree and the evaluation refuses rather than publish.
    fn rule_exact(
        &mut self,
        (index, rule): (usize, &crate::formula_ir::RuleIr),
        mut closed: Option<&mut SourceBatch>,
        support: &crate::formula_support::Support<'_>,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<bool, FormulaFailure> {
        let mut consistent = true;
        if closed.is_some() {
            saturation_agrees(index, rule, support, context)?;
        }
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
            if let Some(certainty) = support.certainty()
                && !crate::formula_support::decided_negations_hold(
                    rule,
                    certainty.decided(index),
                    &row.values,
                    context.computation,
                    context.limits,
                    context.counters,
                )?
            {
                continue;
            }
            if let Some(batch) = closed.as_deref_mut() {
                let HeadIr::Normal(Some(head)) = &rule.head else {
                    unreachable!("a closed component has a head")
                };
                let atom = context.atom(*head, &row.values)?;
                batch.push(
                    &atom,
                    context.computation,
                    context.limits,
                    context.counters,
                    context.location,
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

/// Refuse a saturated rule whose default negations support completion did not
/// all decide, or whose assignment elements it did not all require: the plan
/// and the support would then disagree about the rule's certainty.
fn saturation_agrees(
    index: usize,
    rule: &crate::formula_ir::RuleIr,
    support: &crate::formula_support::Support<'_>,
    context: &mut Context<'_, '_, '_>,
) -> Result<(), FormulaFailure> {
    let mut negated = 0_usize;
    let mut required = true;
    for literal in &rule.body {
        match literal {
            LiteralIr::Atom(negation, _) if *negation != DefaultNegation::None => negated += 1,
            LiteralIr::Aggregate(aggregate) => {
                for (position, element) in aggregate.elements.iter().enumerate() {
                    context.work()?;
                    required &= crate::formula_support::certain::Certainty::requires(
                        support.certainty(),
                        aggregate.id,
                        position,
                        element,
                    );
                }
            }
            _ => {}
        }
        context.work()?;
    }
    let decided = negated == 0
        || support
            .certainty()
            .is_some_and(|certainty| certainty.decided(index).count() == negated);
    if decided && required {
        Ok(())
    } else {
        Err(FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Owner,
            location: rule.location,
        })
    }
}
