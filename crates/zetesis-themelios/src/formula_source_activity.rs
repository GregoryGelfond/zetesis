//! Shared finite source activity for objectives and projection declarations.
//!
//! Required/possible activity is a source grounding abstraction, not answer-set
//! realization. Optional means retained as a grounding possibility; it does not
//! assert that several optional literals can hold together. For example,
//! `a,not a` can retain a priority while its original-model query is always false.
//! Choices stay optional even when constraints force an answer.
//! The original theory is never read or changed by this certificate.
//! Completed support and whole-producer refinement establish source coverage;
//! the separate `model_query` module constructs original-model objective queries
//! only after row eligibility. Its retained-node limit does not bound activity.

use std::collections::BTreeSet;

use crate::formula_support::components::Pattern as AtomPattern;
use themelios_base::span::Location;
use themelios_program::program::DefaultNegation;
use themelios_program::symbol::{Name, Signature};
use zetesis_core::catalog::PredicateRef;
pub(crate) mod model_query;
mod cyclic;
mod possible;
mod roots;
mod authority;

use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_binding::Binding;
use crate::formula_ir::{HeadIr, LiteralIr, Prepared, RuleIr};
use crate::formula_support::{
    self, CompletedQueries, Computation, Counters, Join, SourceAtom, SourceSelection, StorageLease,
    Support,
};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

fn ordinary(literals: &[LiteralIr]) -> bool {
    literals.iter().all(|literal| {
        matches!(
            literal,
            LiteralIr::Atom(..)
                | LiteralIr::Compare(..)
                | LiteralIr::Guard(_)
                | LiteralIr::Bind { .. }
                | LiteralIr::Range { .. }
        )
    })
}

fn refusal(location: Location) -> FormulaFailure {
    crate::diagnostic::unsupported(crate::ProfileFeature::ObjectiveSourceEligibility, location)
        .into()
}

/// A completed source atom can be absent, possible or required. This order
/// makes finite conjunction the minimum and alternative producers the maximum.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Activity {
    Absent,
    Optional,
    Required,
}

impl Activity {
    pub(crate) fn negate(self) -> Self {
        match self {
            Self::Absent => Self::Required,
            Self::Optional => Self::Optional,
            Self::Required => Self::Absent,
        }
    }
}

/// Finite source-atom truth coverage built from completed possible Support.
/// Optional rows need not be simultaneously realizable. This certificate never
/// determines answer-set membership or replaces original-model query truth.
pub(crate) struct SourceEligibility {
    atoms: SourceSelection,
    activity: Vec<Activity>,
    round: Option<authority::Round>,
    lease: StorageLease,
}

pub(crate) struct Context<'a, 'terms, 'source> {
    pub computation: &'a mut Computation<'terms, 'source>,
    pub limits: &'a FormulaLimits,
    pub budget: &'a mut Budget,
    pub counters: &'a mut Counters,
    pub location: Location,
}

impl Context<'_, '_, '_> {
    fn work(&mut self) -> Result<(), FormulaFailure> {
        self.counters.work(self.limits, self.location)
    }

    fn atom(
        &mut self,
        pattern: AtomPattern,
        binding: &Binding<'_>,
    ) -> Result<SourceAtom, FormulaFailure> {
        let pattern =
            self.computation
                .static_pattern(pattern, self.limits, self.counters, self.location)?;
        self.computation
            .atom(pattern, binding, self.limits, self.counters, self.location)
    }

    fn pattern_matches(
        &mut self,
        pattern: AtomPattern,
        expected: &Signature,
    ) -> Result<bool, FormulaFailure> {
        let pattern =
            self.computation
                .static_pattern(pattern, self.limits, self.counters, self.location)?;
        self.matches_signature(pattern.predicate(), expected)
    }

    /// Compare the canonical predicate to an upstream analysis key without
    /// copying its spelling into a second execution predicate.
    fn matches_signature(
        &mut self,
        predicate: PredicateRef<'_>,
        expected: &Signature,
    ) -> Result<bool, FormulaFailure> {
        self.work()?;
        if crate::coherence::source_sign(predicate.sign()) != expected.sign
            || predicate.arity() != expected.arity as usize
        {
            return Ok(false);
        }
        let actual = predicate.name();
        self.counters
            .charge_work(actual.len() as u128, self.limits, self.location)?;
        Ok(actual == expected.name.as_str())
    }

    fn produces(&mut self, rule: &RuleIr, expected: &Signature) -> Result<bool, FormulaFailure> {
        match &rule.head {
            HeadIr::Normal(Some(head)) => self.pattern_matches(*head, expected),
            HeadIr::Choice(group) => {
                for element in &group.elements {
                    self.work()?;
                    if let Some(head) = element.head.positive_atom()
                        && self.pattern_matches(*head, expected)?
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            HeadIr::Disjunction(_) | HeadIr::ConditionalDisjunction { .. } => {
                for head in rule
                    .head
                    .disjuncts()
                    .filter_map(|head| head.positive_atom())
                {
                    self.work()?;
                    if self.pattern_matches(*head, expected)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            HeadIr::Normal(None) => Ok(false),
        }
    }

    fn allocation(&self) -> FormulaFailure {
        FormulaFailure::Objective {
            error: zetesis_objective::AdmissionError::Allocation,
            location: self.location,
        }
    }

    fn scalar(&mut self, literal: &LiteralIr, binding: &Binding) -> Result<bool, FormulaFailure> {
        match literal {
            LiteralIr::Compare(left, relation, right) => {
                let left = formula_support::expression(
                    left,
                    binding,
                    self.computation,
                    self.limits,
                    self.counters,
                    self.location,
                )?;
                let right = formula_support::expression(
                    right,
                    binding,
                    self.computation,
                    self.limits,
                    self.counters,
                    self.location,
                )?;
                let read = self.computation.read();
                self.counters.work(self.limits, self.location)?;
                let left = read.term(&left).map_err(|error| {
                    crate::formula_binding::assignment(
                        zetesis_core::catalog::AssignmentError::Read(error),
                        self.location,
                    )
                })?;
                self.counters.work(self.limits, self.location)?;
                let right = read.term(&right).map_err(|error| {
                    crate::formula_binding::assignment(
                        zetesis_core::catalog::AssignmentError::Read(error),
                        self.location,
                    )
                })?;
                formula_support::compare(
                    left,
                    *relation,
                    right,
                    self.limits,
                    self.counters,
                    self.location,
                )
            }
            LiteralIr::Guard(guard) => guard.evaluate(
                binding,
                self.computation,
                self.limits,
                self.counters,
                self.location,
            ),
            LiteralIr::Bind { .. } | LiteralIr::Range { .. } => Ok(true),
            _ => Err(refusal(self.location)),
        }
    }

    /// Reserve the coexisting closure, pending traversal and remaining set
    /// before inserting any new predicate. Legacy certificate storage remains
    /// included throughout this independent certificate's construction.
    fn closure<'a>(
        &mut self,
        graph: &themelios_analysis::depend::DependencyGraph,
        retained: usize,
        conditions: impl IntoIterator<Item = &'a [LiteralIr]>,
    ) -> Result<BTreeSet<Signature>, FormulaFailure> {
        let mut relevant = BTreeSet::new();
        let mut pending = Vec::new();
        for condition in conditions {
            self.source_roots(condition, retained, &mut relevant, &mut pending)?;
        }
        while let Some(predicate) = pending.pop() {
            self.work()?;
            for (_, dependency) in graph.edges_from(&predicate) {
                self.work()?;
                if !relevant.contains(dependency) {
                    self.discover(dependency.clone(), retained, &mut relevant, &mut pending)?;
                }
            }
        }
        Ok(relevant)
    }

    fn discover(
        &mut self,
        predicate: Signature,
        retained: usize,
        relevant: &mut BTreeSet<Signature>,
        pending: &mut Vec<Signature>,
    ) -> Result<(), FormulaFailure> {
        if relevant.contains(&predicate) {
            return Ok(());
        }
        self.entries(retained.saturating_add(relevant.len().saturating_add(1).saturating_mul(3)))?;
        pending.try_reserve(1).map_err(|_| self.allocation())?;
        relevant.insert(predicate.clone());
        pending.push(predicate);
        Ok(())
    }

    fn entries(&self, count: usize) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::ObjectivePresenceEntries,
            count as u128,
            self.limits.max_objective_presence_entries as u128,
            self.location,
        )
    }
}

impl SourceEligibility {
    /// Discover all scoped atom dependencies and complete their shared activity.
    /// This never adds the observing conditions to logical producers.
    pub(crate) fn build_from_conditions<'a>(
        prepared: &Prepared,
        completed_support: &CompletedQueries<'_>,
        retained: usize,
        conditions: impl IntoIterator<Item = &'a [LiteralIr]>,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<Self, FormulaFailure> {
        let support = completed_support.support();
        let graph = prepared.analysis.dependencies();
        let relevant = context.closure(graph, retained, conditions)?;
        // The dependency sets coexist with the completed atom table. These
        // logical planning slots are independent of allocator representation.
        let temporary = retained.saturating_add(relevant.len().saturating_mul(3));
        context.entries(temporary)?;
        let mut remaining = relevant.clone();
        let mut completed = BTreeSet::<Signature>::new();
        let mut result = Self::new(context)?;
        while !remaining.is_empty() {
            let mut ready = None;
            for predicate in &remaining {
                context.work()?;
                let mut available = true;
                for (_, dependency) in graph.edges_from(predicate) {
                    context.work()?;
                    available &= !relevant.contains(dependency) || completed.contains(dependency);
                }
                if available {
                    ready = Some(predicate.clone());
                    break;
                }
            }
            let Some(predicate) = ready else {
                result.cyclic(prepared, support, &remaining, temporary, context)?;
                break;
            };
            result.predicate(prepared, &predicate, support, temporary, context)?;
            remaining.remove(&predicate);
            completed.insert(predicate);
        }
        Ok(result)
    }

    fn predicate(
        &mut self,
        prepared: &Prepared,
        predicate: &Signature,
        support: &Support<'_>,
        temporary: usize,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<(), FormulaFailure> {
        self.round = Some(authority::Round::new(context));
        let result = self
            .derive_predicate(
                prepared,
                predicate,
                support,
                temporary.saturating_add(self.activity.len()),
                context,
            )
            .and_then(|()| self.publish_predicate(context));
        self.round = None;
        result
    }

    fn derive_predicate(
        &mut self,
        prepared: &Prepared,
        predicate: &Signature,
        support: &Support<'_>,
        temporary: usize,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<(), FormulaFailure> {
        let mut precise = true;
        for rule in &prepared.rules {
            context.work()?;
            if !context.produces(rule, predicate)? {
                continue;
            }
            context.location = rule.location;
            precise &= match &rule.head {
                HeadIr::Choice(_) | HeadIr::Normal(_) => true,
                HeadIr::Disjunction(heads) => {
                    heads.iter().all(|head| head.positive_atom().is_some())
                }
                HeadIr::ConditionalDisjunction { .. } => false,
            };
        }
        if precise {
            for rule in &prepared.rules {
                context.work()?;
                if !context.produces(rule, predicate)? {
                    continue;
                }
                self.rule(rule, predicate, support, temporary, context)?;
            }
        } else {
            // Unsupported head classification retains completed Support as an
            // upper carrier. Rich bodies of classified producers instead share
            // the original scoped lowering and independent activity fold.
            for candidate in support.predicates() {
                context.work()?;
                if context.matches_signature(candidate, predicate)? {
                    self.possible_into_round(candidate, support, temporary, context)?;
                    break;
                }
            }
        }
        Ok(())
    }

    fn rule(
        &mut self,
        rule: &RuleIr,
        predicate: &Signature,
        support: &Support<'_>,
        temporary: usize,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<(), FormulaFailure> {
        context.location = rule.location;
        let mut outer = Join::rule(
            rule,
            support,
            context.computation,
            context.limits,
            context.budget,
            context.counters,
        )?;
        while let Some(binding) = outer.next(
            context.computation,
            context.limits,
            context.budget,
            context.counters,
            rule.location,
        )? {
            let body =
                self.producer_activity(&rule.body, &rule.body_binding(&binding), support, context)?;
            match &rule.head {
                HeadIr::Normal(Some(head)) => {
                    self.retain(*head, &binding, body, temporary, context)?;
                }
                HeadIr::Disjunction(heads) => {
                    if heads.iter().any(|head| head.positive_atom().is_none()) {
                        return Err(refusal(rule.location));
                    }
                    for head in heads.iter().filter_map(|head| head.positive_atom()) {
                        if context.pattern_matches(*head, predicate)? {
                            self.retain(
                                *head,
                                &binding,
                                body.min(Activity::Optional),
                                temporary,
                                context,
                            )?;
                        }
                    }
                }
                HeadIr::ConditionalDisjunction { .. } => {
                    // Rich local eligibility uses completed support above;
                    // model truth remains with the exact original query.
                    return Err(refusal(rule.location));
                }
                HeadIr::Choice(group) => {
                    for element in &group.elements {
                        let Some(head) = element.head.positive_atom() else {
                            continue;
                        };
                        if !context.pattern_matches(*head, predicate)? {
                            continue;
                        }
                        let mut local = Join::element(
                            element,
                            &binding,
                            support,
                            context.budget,
                            crate::formula_support::Context::new(
                                &*context.computation,
                                context.limits,
                                context.counters,
                                rule.location,
                            ),
                        )?;
                        while let Some(row) = local.next(
                            context.computation,
                            context.limits,
                            context.budget,
                            context.counters,
                            rule.location,
                        )? {
                            let eligible = self.producer_activity(
                                &element.condition,
                                &element.body_binding(&row),
                                support,
                                context,
                            )?;
                            self.retain(
                                *head,
                                &row,
                                body.min(eligible).min(Activity::Optional),
                                temporary,
                                context,
                            )?;
                        }
                    }
                }
                HeadIr::Normal(None) => unreachable!("only producer heads selected"),
            }
        }
        Ok(())
    }

    fn retain(
        &mut self,
        pattern: AtomPattern,
        binding: &Binding,
        activity: Activity,
        temporary: usize,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<(), FormulaFailure> {
        // Validate the complete assigned atom even when its producer is absent.
        // Identity admission changes neither activity nor support membership.
        let atom = context.atom(pattern, binding)?;
        if activity == Activity::Absent {
            return Ok(());
        }
        self.stage(&atom, activity, temporary, context)
    }

    fn producer_activity(
        &self,
        literals: &[LiteralIr],
        binding: &Binding,
        support: &Support<'_>,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<Activity, FormulaFailure> {
        if ordinary(literals) {
            self.activity(literals, binding, context)
        } else {
            crate::formula_ground::source_activity(literals, binding, support, self, context)
        }
    }

    /// Source truth coverage is evaluated without constructing or charging a
    /// retained model query. Every literal is visited even after known absence.
    pub(crate) fn activity(
        &self,
        literals: &[LiteralIr],
        binding: &Binding,
        context: &mut Context<'_, '_, '_>,
    ) -> Result<Activity, FormulaFailure> {
        let mut result = Activity::Required;
        for literal in literals {
            context.work()?;
            let activity = match literal {
                LiteralIr::Atom(negation, pattern) => {
                    let activity = self.pattern_activity(*pattern, binding, context)?;
                    if *negation == DefaultNegation::Not {
                        activity.negate()
                    } else {
                        activity
                    }
                }
                LiteralIr::Aggregate(_)
                | LiteralIr::Conditional(_)
                | LiteralIr::ProjectedAtom(..) => Activity::Optional,
                LiteralIr::PatternAtom(pattern) => {
                    self.pattern_activity(pattern.atom, binding, context)?
                }
                // The complete Join already checked these ordinary data filters.
                LiteralIr::ArgumentCheck { .. } | LiteralIr::TupleCompare(..) => Activity::Required,
                _ => {
                    if context.scalar(literal, binding)? {
                        Activity::Required
                    } else {
                        Activity::Absent
                    }
                }
            };
            result = result.min(activity);
        }
        Ok(result)
    }
}

pub(crate) fn signature<'a>(
    predicate: impl Into<zetesis_core::catalog::PredicateRef<'a>>,
) -> Signature {
    let predicate = predicate.into();
    Signature {
        sign: crate::coherence::source_sign(predicate.sign()),
        name: Name::new(predicate.name()).expect("validated source predicate"),
        arity: u32::try_from(predicate.arity()).expect("bounded source arity"),
    }
}
