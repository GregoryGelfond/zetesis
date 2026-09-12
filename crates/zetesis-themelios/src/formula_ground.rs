//! Finite substitutions and support-preserving conditional-choice formulas.

mod objectives;
mod scoped_body;
mod atoms;
mod nodes;
mod metadata;
#[cfg(test)]
mod constants;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use themelios_base::span::Location;
use themelios_program::program::{AggregateFunction, DefaultNegation};
use zetesis_core::{Atom, AtomPattern, Value};
use zetesis_ferraris::{
    AggregateComparison, AggregateElement, AggregateExtremum, AggregateFamilyLimits,
    AggregateGuard as NumericGuard, Node, Theory, ValueExtremumElement, append_aggregate,
    append_aggregate_family, append_value_extremum,
};

use crate::expansion::Budget;
use crate::formula::{Compiled, ceiling};
use crate::formula_binding::Binding;
use crate::formula_ir::{
    AggregateGuard, AggregateIr, AggregateKey, ChoiceIr, HeadElementKey, HeadIr, HeadLiteral,
    HeadMeasure, HeadOperand, LiteralIr, Prepared, Projection, RuleIr, value_bytes,
};
use crate::formula_support::{self, Counters, Join, Support};
use crate::grounding_observer::{Event, Profile};
use crate::{ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource};

/// Canonical Boolean nodes established by `Builder::initialize`.
/// Falsum is bottom; verum is the implication from bottom to itself.
pub(super) const FALSUM: usize = 0;
pub(super) const VERUM: usize = 1;

pub(super) const fn boolean(truth: bool) -> usize {
    if truth { VERUM } else { FALSUM }
}

pub(crate) fn ground(
    prepared: Prepared,
    limits: &FormulaLimits,
    budget: &mut Budget,
    location: Location,
    observer: Option<&dyn crate::GroundingObserver>,
    count_plan: Option<crate::formula_count_plan::Request<'_>>,
) -> Result<Compiled, FormulaFailure> {
    use crate::GroundingPhase;

    let profile = Profile::new(observer);
    let Instantiation {
        builder,
        objectives,
        objective_origins,
        analysis_basis,
        analysis,
        analyzed,
        objective_declarations,
    } = instantiate(prepared, limits, budget, location, &profile, count_plan)?;
    let Emission {
        atoms,
        nodes,
        roots,
        origins,
        count_plan,
    } = builder.finish(&profile)?;
    let theory = profile.phase(GroundingPhase::TheoryValidation, None, || {
        Theory::new(atoms.len(), nodes, roots, limits.theory)
            .map_err(|error| FormulaFailure::Theory { error, location })
    })?;
    let count_plan = count_plan.map_or(
        crate::formula_count_plan::Outcome::NotRequested,
        |collector| collector.finish(&theory),
    );
    Ok(Compiled {
        analysis_basis,
        analysis,
        analyzed,
        theory,
        count_plan,
        atoms,
        origins,
        objectives,
        objective_origins,
        objective_declarations,
    })
}

/// Source-dependent construction owns possible support only while joins use it.
/// The returned builder owns emitted atoms, not a borrowed support catalog.
struct Instantiation<'a> {
    builder: Builder<'a>,
    objectives: zetesis_objective::ObjectiveProgram,
    objective_origins: Vec<Vec<Location>>,
    analysis_basis: crate::AnalysisBasis,
    analysis: themelios_analysis::Analysis,
    analyzed: themelios_program::program::Program,
    objective_declarations: Vec<Location>,
}

fn instantiate<'a>(
    prepared: Prepared,
    limits: &'a FormulaLimits,
    budget: &'a mut Budget,
    location: Location,
    profile: &Profile<'_>,
    count_plan: Option<crate::formula_count_plan::Request<'_>>,
) -> Result<Instantiation<'a>, FormulaFailure> {
    use crate::GroundingPhase;

    let mut counters = Counters::observed(profile.work());
    let catalog = profile.phase(GroundingPhase::SupportCompletion, None, || {
        formula_support::build(&prepared, limits, budget, &mut counters, location)
    })?;
    let completed = profile.phase(GroundingPhase::SupportCompletion, None, || {
        catalog.snapshot(limits, &mut counters, location)
    })?;
    let support = completed.relations();
    let (objectives, objective_origins) =
        profile.phase(GroundingPhase::ObjectiveActivation, None, || {
            objectives::prepare(
                &prepared,
                &completed,
                limits,
                budget,
                &mut counters,
                location,
            )
        })?;
    let mut builder = profile.phase(GroundingPhase::FormulaInitialization, None, || {
        let mut builder = Builder::empty(
            limits,
            budget,
            counters,
            Purpose::Theory,
            count_plan.map(|request| crate::formula_count_plan::Collector::new(request, location)),
        );
        builder.initialize(location)?;
        Ok::<_, FormulaFailure>(builder)
    })?;
    for rule in &prepared.rules {
        profile.phase(
            GroundingPhase::RuleInstantiation,
            Some(rule.location),
            || {
                if crate::formula_factor::rule(&mut builder, rule, support)? {
                    return Ok(());
                }
                let mut outer = Join::rule(rule, support, builder.budget)?;
                while let Some(row) =
                    outer.next_row(limits, builder.budget, &mut builder.counters, rule.location)?
                {
                    if row.passes {
                        builder.rule(rule, &row.values, support)?;
                    } else {
                        builder.validate_body(&rule.body, &row.values, support, rule.location)?;
                    }
                }
                Ok::<_, FormulaFailure>(())
            },
        )?;
    }
    Ok(Instantiation {
        builder,
        objectives,
        objective_origins,
        analysis_basis: prepared.analysis_basis,
        analysis: prepared.analysis,
        analyzed: prepared.analyzed,
        objective_declarations: prepared.objective_declarations,
    })
}

/// Only these owned vectors and optional premises survive formula construction.
/// Validation borrows no interning index, producer table or aggregate cache.
struct Emission {
    atoms: Vec<Atom>,
    nodes: Vec<Node>,
    roots: Vec<usize>,
    origins: Vec<Vec<Location>>,
    count_plan: Option<crate::formula_count_plan::Collector>,
}

pub(super) struct Builder<'a> {
    purpose: Purpose,
    pub(super) limits: &'a FormulaLimits,
    pub(super) budget: &'a mut Budget,
    catalog: atoms::Catalog,
    metadata: metadata::Metadata,
    nodes: Vec<Node>,
    node_indices: nodes::Index,
    roots: Vec<usize>,
    origins: Vec<Vec<Location>>,
    pub(super) counters: Counters,
    origin_count: usize,
    aggregate_cache: BTreeMap<AggregateContext, CachedAggregate>,
    cached_elements: usize,
    cached_key_bytes: u128,
    cached_roots: usize,
    count_plan: Option<crate::formula_count_plan::Collector>,
}

#[derive(Clone, Copy)]
enum Purpose {
    Theory,
    Objective,
    /// Discarded source-body validation, bounded by the existing theory ceilings.
    Validation,
}

/// Original aggregate identity and its scoped outer assignment. Absence is
/// retained in the key; it cannot alias an ordinary numeric zero.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct AggregateContext {
    aggregate: usize,
    outer: Vec<Option<Value>>,
}

struct CachedAggregate {
    elements: GroundAggregate,
    roots: Option<Arc<[(Value, usize)]>>,
}

#[derive(Clone)]
enum GroundAggregate {
    Numeric(Arc<[AggregateElement]>),
    Extrema(Arc<[ValueExtremumElement]>),
}
impl GroundAggregate {
    fn len(&self) -> usize {
        match self {
            Self::Numeric(elements) => elements.len(),
            Self::Extrema(elements) => elements.len(),
        }
    }
}
impl Builder<'_> {
    fn empty<'a>(
        limits: &'a FormulaLimits,
        budget: &'a mut Budget,
        counters: Counters,
        purpose: Purpose,
        count_plan: Option<crate::formula_count_plan::Collector>,
    ) -> Builder<'a> {
        Builder {
            limits,
            budget,
            catalog: atoms::Catalog::default(),
            metadata: metadata::Metadata::default(),
            nodes: Vec::new(),
            node_indices: nodes::Index::new(),
            roots: Vec::new(),
            origins: Vec::new(),
            counters,
            origin_count: 0,
            aggregate_cache: BTreeMap::new(),
            cached_elements: 0,
            cached_key_bytes: 0,
            cached_roots: 0,
            count_plan,
            purpose,
        }
    }
    fn node_bound(&self) -> (FormulaResource, usize) {
        match self.purpose {
            Purpose::Theory | Purpose::Validation => {
                (FormulaResource::Nodes, self.limits.theory.max_nodes)
            }
            Purpose::Objective => (
                FormulaResource::ObjectiveFormulaNodes,
                self.limits.max_objective_formula_nodes,
            ),
        }
    }
    /// Admit the canonical constants before any other node in an empty builder.
    /// Each admission retains its ordinary work and purpose-specific ceiling.
    /// On failure the caller still owns the builder and its spent counters.
    fn initialize(&mut self, location: Location) -> Result<(), FormulaFailure> {
        self.node(Node::False, location)?;
        self.node(Node::Implies(FALSUM, FALSUM), location)?;
        Ok(())
    }
    fn atom_bound(&self) -> (FormulaResource, usize) {
        match self.purpose {
            Purpose::Theory | Purpose::Validation => {
                (FormulaResource::Atoms, self.limits.theory.max_atoms)
            }
            Purpose::Objective => (
                FormulaResource::ObjectiveFormulaAtoms,
                self.limits.max_objective_formula_atoms,
            ),
        }
    }
    /// Complete the semantic additions before discarding construction indexes.
    /// Moving the emitted vectors preserves IDs, root order and source origins;
    /// cumulative budgets are never released with the discarded scratch.
    fn finish(mut self, profile: &Profile<'_>) -> Result<Emission, FormulaFailure> {
        use crate::GroundingPhase;

        profile.phase(GroundingPhase::Coherence, None, || self.coherence())?;
        profile.phase(GroundingPhase::SupportGuards, None, || {
            self.support_guards()
        })?;
        Ok(Emission {
            atoms: self.catalog.into_atoms(),
            nodes: self.nodes,
            roots: self.roots,
            origins: self.origins,
            count_plan: self.count_plan,
        })
    }

    // Only the completed atom catalog establishes which opposite tuples can
    // coexist. Reuse its IDs; coherence must create neither atoms nor support.
    fn coherence(&mut self) -> Result<(), FormulaFailure> {
        for index in 0..self.catalog.len() {
            let atom = &self.catalog.atoms()[index];
            if atom.predicate().sign() != zetesis_core::Sign::Negative {
                continue;
            }
            let location = self.metadata.location(index);
            let bytes = atom.predicate().name().len() as u128
                + atom.values().iter().map(value_bytes).sum::<u128>();
            self.budget
                .charge(ExpansionResource::ScalarBytes, bytes, location)?;
            let opposite = Atom::new(
                zetesis_core::Predicate::new(atom.predicate().name(), atom.predicate().arity())
                    .expect("validated nonempty predicate"),
                atom.values().to_vec(),
            )
            .expect("opposite atom keeps the same arity");
            self.work(location)?;
            if let Some(other) = self.catalog.find(&opposite) {
                let positive = self.node(Node::Atom(other), location)?;
                let negative = self.node(Node::Atom(index), location)?;
                let both = self.and(positive, negative, location)?;
                let constraint = self.neg(both, location)?;
                let origins = [location, self.metadata.location(other)];
                let evidence = if origins[0] == origins[1] {
                    &origins[..1]
                } else {
                    &origins[..]
                };
                self.root_at(constraint, Cow::Borrowed(evidence), location)?;
            }
        }
        Ok(())
    }

    pub(super) fn work(&mut self, location: Location) -> Result<(), FormulaFailure> {
        self.counters.work(self.limits, location)
    }
    pub(super) fn node(&mut self, node: Node, location: Location) -> Result<usize, FormulaFailure> {
        self.work(location)?;
        self.counters.record(Event::NodeLookup);
        let bound = self.node_bound();
        let (index, inserted) = nodes::intern(
            &mut self.node_indices,
            &mut self.nodes,
            node,
            bound,
            location,
        )?;
        if inserted {
            self.counters.record(Event::NodeInserted);
        }
        Ok(index)
    }
    pub(super) fn and(
        &mut self,
        left: usize,
        right: usize,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        if left == FALSUM || right == FALSUM {
            Ok(FALSUM)
        } else if left == VERUM {
            Ok(right)
        } else if right == VERUM || left == right {
            Ok(left)
        } else {
            self.node(Node::And(left, right), location)
        }
    }
    pub(super) fn or(
        &mut self,
        left: usize,
        right: usize,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        if left == VERUM || right == VERUM {
            Ok(VERUM)
        } else if left == FALSUM {
            Ok(right)
        } else if right == FALSUM || left == right {
            Ok(left)
        } else {
            self.node(Node::Or(left, right), location)
        }
    }
    pub(super) fn neg(
        &mut self,
        formula: usize,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        self.node(Node::Implies(formula, FALSUM), location)
    }
    pub(super) fn root(&mut self, formula: usize, rule: &RuleIr) -> Result<(), FormulaFailure> {
        self.root_at(formula, Cow::Borrowed(&rule.origins), rule.location)
    }
    fn root_at(
        &mut self,
        formula: usize,
        evidence: Cow<'_, [Location]>,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        self.admit_root(evidence.len(), location)?;
        self.publish_root(formula, evidence.into_owned());
        Ok(())
    }
    fn admit_root(&self, origin_count: usize, location: Location) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::Roots,
            self.roots.len() as u128 + 1,
            self.limits.theory.max_roots as u128,
            location,
        )?;
        let origins = self.origin_count as u128 + origin_count as u128;
        ceiling(
            FormulaResource::Origins,
            origins,
            self.limits.max_origin_locations as u128,
            location,
        )?;
        Ok(())
    }
    /// Publish only after root/evidence admission and successful evidence preparation.
    fn publish_root(&mut self, formula: usize, evidence: Vec<Location>) {
        self.origin_count += evidence.len();
        self.roots.push(formula);
        self.origins.push(evidence);
        self.counters.record(Event::Root);
    }
    pub(super) fn atom(
        &mut self,
        pattern: &AtomPattern,
        assignment: &Binding,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        self.work(location)?;
        let mut bytes = pattern.predicate().name().len() as u128;
        for term in pattern.terms() {
            bytes += value_bytes(assignment.resolve(term, location)?);
        }
        // Conservative symbolic allowance for lookup and retained atom/index
        // storage. This cumulative admission charge is not live heap occupancy.
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            bytes.saturating_mul(3),
            location,
        )?;
        let key =
            pattern
                .key(assignment.slots())
                .map_err(|error| FormulaFailure::UnsafeVariable {
                    variable: error.variable,
                    location,
                })?;
        self.counters.record(Event::AtomLookup);
        let required = self.catalog.len() as u128 + 1;
        let (atom_resource, atom_limit) = self.atom_bound();
        let index = match self.catalog.entry(key) {
            atoms::Entry::Occupied(index) => index,
            atoms::Entry::Vacant(entry) => {
                ceiling(atom_resource, required, atom_limit as u128, location)?;
                if matches!(self.purpose, Purpose::Theory) {
                    self.budget
                        .charge(ExpansionResource::Origins, 1, location)?;
                }
                let index = entry
                    .insert()
                    .map_err(|error| FormulaFailure::AtomAllocation { error, location })?;
                self.counters.record(Event::AtomInserted);
                if matches!(self.purpose, Purpose::Theory) {
                    self.metadata
                        .atom(location, &mut self.counters, self.limits)?;
                }
                index
            }
        };
        self.node(Node::Atom(index), location)
    }
    /// Validate a rejected complete row without changing the original atom/node
    /// catalog, caches, roots or producer tables. Scratch uses the existing
    /// theory ceilings; cumulative source work and scalar copying remain shared.
    fn validate_body(
        &mut self,
        literals: &[LiteralIr],
        binding: &Binding,
        support: &Support<'_>,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let mut context = crate::formula_objective_dependencies::eligibility::Context {
            limits: self.limits,
            budget: self.budget,
            counters: &mut self.counters,
            location,
        };
        scoped_body::validate_with_purpose(
            literals,
            binding,
            support,
            &mut context,
            Purpose::Validation,
        )?;
        Ok(())
    }
    pub(super) fn body(
        &mut self,
        literals: &[LiteralIr],
        assignment: &Binding,
        location: Location,
        support: &Support,
    ) -> Result<usize, FormulaFailure> {
        let mut result = VERUM;
        for literal in literals {
            if let LiteralIr::Atom(negation, pattern) = literal {
                let mut atom = self.atom(pattern, assignment, location)?;
                if *negation != DefaultNegation::None {
                    atom = self.neg(atom, location)?;
                }
                if *negation == DefaultNegation::NotNot {
                    atom = self.neg(atom, location)?;
                }
                result = self.and(result, atom, location)?;
            } else if let LiteralIr::PatternAtom(pattern) = literal {
                let atom = self.atom(&pattern.atom, assignment, location)?;
                result = self.and(result, atom, location)?;
            } else if let LiteralIr::Aggregate(aggregate) = literal {
                let aggregate = self.aggregate(aggregate, assignment, support, location)?;
                result = self.and(result, aggregate, location)?;
            } else if let LiteralIr::Conditional(conditional) = literal {
                let conditional = self.conditional(conditional, assignment, support, location)?;
                result = self.and(result, conditional, location)?;
            } else if let LiteralIr::ProjectedAtom(negation, projection) = literal {
                let mut projected = self.project(projection, assignment, support, location)?;
                projected = self.neg(projected, location)?;
                if *negation == DefaultNegation::NotNot {
                    projected = self.neg(projected, location)?;
                }
                result = self.and(result, projected, location)?;
            }
        }
        Ok(result)
    }
    pub(super) fn project(
        &mut self,
        projection: &Projection,
        assignment: &Binding,
        support: &Support,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        match projection {
            Projection::Arguments { predicate, terms } => {
                self.project_arguments(predicate, terms, assignment, support, location)
            }
            Projection::Witnesses {
                atom,
                bindings,
                variables,
                inputs,
            } => {
                let mut rows = Join::new(
                    bindings,
                    &assignment.prefix(*inputs),
                    *variables,
                    support,
                    self.budget,
                    location,
                )?;
                let mut result = FALSUM;
                while let Some(row) =
                    rows.next(self.limits, self.budget, &mut self.counters, location)?
                {
                    self.work(location)?;
                    let atom = self.atom(atom, &row, location)?;
                    result = self.or(result, atom, location)?;
                }
                Ok(result)
            }
        }
    }

    fn project_arguments(
        &mut self,
        predicate: &zetesis_core::Predicate,
        terms: &[Option<zetesis_core::Term>],
        assignment: &Binding,
        support: &Support,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        let mut result = FALSUM;
        for atom in support.rows(predicate) {
            self.work(location)?;
            let mut matches = true;
            for (column, term) in terms.iter().enumerate() {
                let value = atom.value(column).expect("checked projection arity");
                self.work(location)?;
                if let Some(term) = term {
                    matches &= assignment.resolve(term, location)? == value;
                }
            }
            if matches {
                self.budget.charge(
                    ExpansionResource::ScalarBytes,
                    atom.predicate().name().len() as u128
                        + formula_support::row_values(atom)
                            .map(value_bytes)
                            .sum::<u128>(),
                    location,
                )?;
                let pattern = AtomPattern::new(
                    atom.predicate().clone(),
                    formula_support::row_values(atom)
                        .cloned()
                        .map(zetesis_core::Term::Constant)
                        .collect(),
                )
                .expect("projection retains the source atom arity");
                let atom = self.atom(&pattern, &Binding::default(), location)?;
                result = self.or(result, atom, location)?;
            }
        }
        Ok(result)
    }
    fn rule(
        &mut self,
        rule: &RuleIr,
        assignment: &Binding,
        support: &Support,
    ) -> Result<(), FormulaFailure> {
        self.work(rule.location)?;
        let body = self.body(
            &rule.body,
            &rule.body_binding(assignment),
            rule.location,
            support,
        )?;
        if body == FALSUM {
            return Ok(());
        }
        match &rule.head {
            HeadIr::Normal(head) => {
                let head = match head {
                    Some(head) => self.atom(head, assignment, rule.location)?,
                    None => FALSUM,
                };
                let formula = self.node(Node::Implies(body, head), rule.location)?;
                self.root(formula, rule)?;
                if head != FALSUM {
                    self.producer(head, body, rule)?;
                }
                Ok(())
            }
            HeadIr::Disjunction(heads) => {
                let mut disjunction = FALSUM;
                let mut distinct = BTreeSet::new();
                for head in heads {
                    let (literal, atom) = self.head_literal(head, assignment, rule.location)?;
                    if distinct.insert(literal) {
                        disjunction = self.or(disjunction, literal, rule.location)?;
                        // Necessary support is the original body for each head,
                        // not a shifted rule excluding the other disjuncts.
                        if let Some(atom) = atom {
                            if head.positive_atom().is_some() {
                                self.producer(atom, body, rule)?;
                            } else {
                                // A negative occurrence contributes provenance for
                                // its atom's guard, without supplying any support.
                                self.head_origins(atom, rule)?;
                            }
                        }
                    }
                }
                let formula = self.node(Node::Implies(body, disjunction), rule.location)?;
                self.root(formula, rule)
            }
            HeadIr::Choice(group) => self.choice(rule, group, body, assignment, support),
        }
    }
    pub(super) fn producer(
        &mut self,
        head: usize,
        antecedent: usize,
        rule: &RuleIr,
    ) -> Result<(), FormulaFailure> {
        self.work(rule.location)?;
        let Node::Atom(atom) = self.nodes[head] else {
            unreachable!("head is an atom");
        };
        self.metadata.producer(
            atom,
            antecedent,
            &mut self.counters,
            self.limits,
            rule.location,
        )?;
        self.record_head_origins(atom, rule)
    }
    fn head_origins(&mut self, head: usize, rule: &RuleIr) -> Result<(), FormulaFailure> {
        let Node::Atom(atom) = self.nodes[head] else {
            unreachable!("head occurrence is an atom");
        };
        self.record_head_origins(atom, rule)
    }
    fn record_head_origins(&mut self, atom: usize, rule: &RuleIr) -> Result<(), FormulaFailure> {
        for &location in &rule.origins {
            self.metadata
                .origin(atom, location, self.budget, &mut self.counters, self.limits)?;
        }
        Ok(())
    }
    fn support_guards(&mut self) -> Result<(), FormulaFailure> {
        // The completed owner can be borrowed independently while formulas grow.
        // No producer sequence is copied or folded before this semantic phase.
        let metadata = std::mem::take(&mut self.metadata);
        for atom in 0..self.catalog.len() {
            let location = metadata.location(atom);
            let mut supported = FALSUM;
            for antecedent in metadata.producers(atom) {
                self.work(location)?;
                supported = self.or(supported, antecedent, location)?;
            }
            let head = self.node(Node::Atom(atom), location)?;
            let necessary = self.node(Node::Implies(head, supported), location)?;
            let negative = self.neg(necessary, location)?;
            let guard = self.neg(negative, location)?;
            self.admit_root(metadata.origins(atom).len(), location)?;
            let origins = metadata.copy_origins(atom, &mut self.counters, self.limits)?;
            self.publish_root(guard, origins);
        }
        Ok(())
    }
    fn choice(
        &mut self,
        rule: &RuleIr,
        group: &ChoiceIr,
        body: usize,
        assignment: &Binding,
        support: &Support,
    ) -> Result<(), FormulaFailure> {
        let ChoiceIr {
            measure, guards, ..
        } = group;
        let keys = crate::formula_head_aggregate::validate_group(
            group,
            assignment,
            support,
            self.limits,
            self.budget,
            &mut self.counters,
            rule.location,
        )?;
        let keys = if self.count_plan.is_some() && *measure == HeadMeasure::Count {
            keys
        } else {
            drop(keys);
            None
        };
        let HeadGroup { eligible, activity } = self.head_group(group, assignment, support, rule)?;
        let retaining = keys.is_some() && !guards.is_empty();
        let mut count_bounds =
            retaining.then(|| crate::formula_count_plan::Bounds::new(eligible.len()));
        let kind = match measure {
            HeadMeasure::Min => Some(AggregateExtremum::Min),
            HeadMeasure::Max => Some(AggregateExtremum::Max),
            _ => None,
        };
        let (ordinary, retained) = if retaining {
            (BTreeMap::new(), Some((eligible, keys)))
        } else {
            drop(keys);
            (eligible, None)
        };
        if let Some((eligible, _)) = &retained {
            self.choice_permissions(
                eligible.iter().map(|(&head, &entry)| (head, entry)),
                body,
                rule,
            )?;
        } else {
            self.choice_permissions(ordinary.into_iter(), body, rule)?;
        }
        if !guards.is_empty() {
            let mut selected = HeadContributions::new(kind);
            for (key, condition) in activity {
                if let Some(contribution) = crate::formula_head_aggregate::contribution(
                    *measure,
                    key.first(),
                    rule.location,
                )? {
                    selected.push(contribution, condition, self.budget, rule.location)?;
                }
            }
            let within = self.aggregate_guards_with_capture(
                &selected.finish(),
                guards,
                assignment,
                kind,
                rule.location,
                count_bounds.as_mut(),
            )?;
            let outside = self.neg(within, rule.location)?;
            let violated = self.and(body, outside, rule.location)?;
            let constraint = self.node(Node::Implies(violated, FALSUM), rule.location)?;
            self.root(constraint, rule)?;
            if let (Some(collector), Some((eligible, Some(keys))), Some(bounds)) =
                (&mut self.count_plan, retained, count_bounds)
            {
                collector.capture_group(
                    crate::formula_count_plan::Input {
                        body,
                        eligible: &eligible,
                        tuple_keys: keys,
                        nodes: &self.nodes,
                        atoms: self.catalog.atoms(),
                        bounds,
                        origins: &rule.origins,
                        location: rule.location,
                    },
                    within,
                    constraint,
                );
            }
        }
        Ok(())
    }
    /// Lower signed truth independently of producer eligibility. A negated
    /// operand remains an implication to falsum, so its reduct is frozen in M.
    fn head_literal(
        &mut self,
        head: &HeadLiteral,
        assignment: &Binding,
        location: Location,
    ) -> Result<(usize, Option<usize>), FormulaFailure> {
        let (mut literal, atom) = match &head.operand {
            HeadOperand::Atom(pattern) => {
                let atom = self.atom(pattern, assignment, location)?;
                (atom, Some(atom))
            }
            HeadOperand::Boolean(value) => (boolean(*value), None),
        };
        if head.negation != DefaultNegation::None {
            literal = self.neg(literal, location)?;
        }
        if head.negation == DefaultNegation::NotNot {
            literal = self.neg(literal, location)?;
        }
        Ok((literal, atom))
    }

    fn head_group(
        &mut self,
        group: &ChoiceIr,
        assignment: &Binding,
        support: &Support,
        rule: &RuleIr,
    ) -> Result<HeadGroup, FormulaFailure> {
        let mut result = HeadGroup::default();
        let elements = &group.elements;
        for element in elements {
            let mut local =
                Join::element(element, assignment, support, self.budget, rule.location)?;
            while let Some(binding) =
                local.next(self.limits, self.budget, &mut self.counters, rule.location)?
            {
                let condition = self.body(
                    &element.condition,
                    &element.body_binding(&binding),
                    rule.location,
                    support,
                )?;
                let (head, atom) = self.head_literal(&element.head, &binding, rule.location)?;
                if let Some(atom) = atom {
                    if element.head.positive_atom().is_some() {
                        let previous = result.eligible.get(&atom).copied().unwrap_or(FALSUM);
                        result
                            .eligible
                            .insert(atom, self.or(previous, condition, rule.location)?);
                    } else {
                        self.head_origins(atom, rule)?;
                    }
                }
                if !group.guards.is_empty() {
                    let selected = self.and(condition, head, rule.location)?;
                    match &element.key {
                        HeadElementKey::BooleanOccurrences(occurrences) => {
                            for occurrence in occurrences {
                                self.work(rule.location)?;
                                self.budget.charge(
                                    ExpansionResource::ScalarBytes,
                                    size_of::<Location>() as u128,
                                    rule.location,
                                )?;
                                self.head_activity(
                                    &mut result,
                                    HeadKey::BooleanOccurrence(*occurrence),
                                    selected,
                                    rule.location,
                                )?;
                            }
                        }
                        HeadElementKey::Atom => {
                            self.head_activity(
                                &mut result,
                                HeadKey::Atom(
                                    element.head.negation,
                                    atom.expect("atomic element key"),
                                ),
                                selected,
                                rule.location,
                            )?;
                        }
                        HeadElementKey::Tuple(terms) => {
                            let key =
                                HeadKey::Tuple(self.head_tuple(terms, &binding, rule.location)?);
                            self.head_activity(&mut result, key, selected, rule.location)?;
                        }
                    }
                }
            }
        }
        Ok(result)
    }

    fn head_activity(
        &mut self,
        group: &mut HeadGroup,
        key: HeadKey,
        selected: usize,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let previous = group.activity.get(&key).copied().unwrap_or(FALSUM);
        if !group.activity.contains_key(&key) {
            ceiling(
                FormulaResource::AggregateElements,
                group.activity.len() as u128 + 1,
                self.limits.aggregate.max_elements as u128,
                location,
            )?;
        }
        let activity = self.or(previous, selected, location)?;
        group.activity.insert(key, activity);
        Ok(())
    }

    fn head_tuple(
        &mut self,
        terms: &[zetesis_core::Term],
        assignment: &Binding,
        location: Location,
    ) -> Result<Vec<Value>, FormulaFailure> {
        let mut tuple = Vec::new();
        for term in terms {
            self.work(location)?;
            let value = assignment.resolve(term, location)?;
            self.budget.charge(
                ExpansionResource::ScalarBytes,
                size_of::<Value>() as u128 + value_bytes(value),
                location,
            )?;
            tuple.push(value.clone());
        }
        Ok(tuple)
    }

    fn choice_permissions(
        &mut self,
        eligible: impl Iterator<Item = (usize, usize)>,
        body: usize,
        rule: &RuleIr,
    ) -> Result<(), FormulaFailure> {
        for (head, condition) in eligible {
            let antecedent = self.and(body, condition, rule.location)?;
            let negative = self.neg(head, rule.location)?;
            let choice = self.or(head, negative, rule.location)?;
            let support = self.node(Node::Implies(antecedent, choice), rule.location)?;
            self.root(support, rule)?;
            self.producer(head, antecedent, rule)?;
        }
        Ok(())
    }
}

/// Permission coalesces by head atom; measure activity coalesces independently
/// by the complete tuple. An ordinary atom choice uses its atom as the implicit
/// key and default-negation sign; a Boolean choice uses its source occurrence
/// within this outer group.
#[derive(Default)]
struct HeadGroup {
    eligible: BTreeMap<usize, usize>,
    activity: BTreeMap<HeadKey, usize>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum HeadKey {
    Tuple(Vec<Value>),
    Atom(DefaultNegation, usize),
    BooleanOccurrence(Location),
}

impl HeadKey {
    fn first(&self) -> Option<&Value> {
        match self {
            Self::Tuple(tuple) => tuple.first(),
            Self::Atom(..) | Self::BooleanOccurrence(_) => None,
        }
    }
}

/// One storage family for coalesced tuple contributions. Each entry retains
/// every selected eligible witness; no candidate truth is assumed here.
enum HeadContributions {
    Numeric(Vec<AggregateElement>),
    Extrema(Vec<ValueExtremumElement>),
}

impl HeadContributions {
    fn new(kind: Option<AggregateExtremum>) -> Self {
        if kind.is_some() {
            Self::Extrema(Vec::new())
        } else {
            Self::Numeric(Vec::new())
        }
    }

    fn push(
        &mut self,
        contribution: crate::formula_head_aggregate::Contribution<'_>,
        condition: usize,
        budget: &mut Budget,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        use crate::formula_head_aggregate::Contribution;

        match (self, contribution) {
            (Self::Numeric(elements), Contribution::Numeric(weight)) => {
                elements.push(AggregateElement { weight, condition });
            }
            (Self::Extrema(elements), Contribution::Extremum(value)) => {
                elements.push(ValueExtremumElement {
                    value: formula_support::copy(value, budget, location)?,
                    condition,
                });
            }
            _ => unreachable!("group measure determines its contribution family"),
        }
        Ok(())
    }

    fn finish(self) -> GroundAggregate {
        match self {
            Self::Numeric(elements) => GroundAggregate::Numeric(elements.into()),
            Self::Extrema(elements) => GroundAggregate::Extrema(elements.into()),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum GroundKey {
    Tuple(Vec<Value>),
    Atom(Atom),
}

impl Builder<'_> {
    fn aggregate(
        &mut self,
        aggregate: &AggregateIr,
        assignment: &Binding,
        support: &Support,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        if let Some(target) = aggregate.binding {
            return self.assignment_aggregate(aggregate, target, assignment, support, location);
        }
        let elements = self.cached_aggregate_elements(aggregate, assignment, support, location)?;
        let mut result = self.aggregate_guards(
            &elements,
            &aggregate.guards,
            assignment,
            extremum(aggregate.function),
            location,
        )?;
        if aggregate.negation != DefaultNegation::None {
            result = self.neg(result, location)?;
        }
        if aggregate.negation == DefaultNegation::NotNot {
            result = self.neg(result, location)?;
        }
        Ok(result)
    }
    fn cached_aggregate_elements(
        &mut self,
        aggregate: &AggregateIr,
        assignment: &Binding,
        support: &Support,
        location: Location,
    ) -> Result<GroundAggregate, FormulaFailure> {
        let Some(target) = aggregate.binding else {
            return self.aggregate_elements(aggregate, assignment, support, location);
        };
        let (key, bytes) = self.cache_key(aggregate.id, target, assignment, location)?;
        self.work(location)?;
        if let Some(cached) = self.aggregate_cache.get(&key) {
            return Ok(cached.elements.clone());
        }
        ceiling(
            FormulaResource::AggregateCacheRows,
            self.aggregate_cache.len() as u128 + 1,
            self.limits.max_aggregate_cache_rows as u128,
            location,
        )?;
        ceiling(
            FormulaResource::AggregateCacheKeys,
            self.cached_key_bytes + bytes,
            self.limits.max_aggregate_cache_key_bytes as u128,
            location,
        )?;
        let elements = self.aggregate_elements(aggregate, assignment, support, location)?;
        ceiling(
            FormulaResource::AggregateCacheElements,
            self.cached_elements as u128 + elements.len() as u128,
            self.limits.max_aggregate_cache_elements as u128,
            location,
        )?;
        self.cached_elements += elements.len();
        self.cached_key_bytes += bytes;
        self.work(location)?;
        self.aggregate_cache.insert(
            key,
            CachedAggregate {
                elements: elements.clone(),
                roots: None,
            },
        );
        Ok(elements)
    }
    fn assignment_aggregate(
        &mut self,
        aggregate: &AggregateIr,
        target: usize,
        assignment: &Binding,
        support: &Support,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        let (key, _) = self.cache_key(aggregate.id, target, assignment, location)?;
        self.work(location)?;
        let roots = self
            .aggregate_cache
            .get(&key)
            .and_then(|cached| cached.roots.as_ref().map(Arc::clone));
        let roots = if let Some(roots) = roots {
            roots
        } else {
            let elements =
                self.cached_aggregate_elements(aggregate, assignment, support, location)?;
            let roots = self.assignment_family(aggregate.function, &elements, location)?;
            self.work(location)?;
            self.aggregate_cache
                .get_mut(&key)
                .expect("eligibility cache was populated before family construction")
                .roots = Some(Arc::clone(&roots));
            roots
        };
        let target_value = assignment.read(target, location)?;
        let index = roots
            .binary_search_by(|(value, _)| value.cmp(target_value))
            .expect("the complete final-U tuple set covers every assignment proposal");
        Ok(roots[index].1)
    }
    fn assignment_family(
        &mut self,
        function: AggregateFunction,
        elements: &GroundAggregate,
        location: Location,
    ) -> Result<Arc<[(Value, usize)]>, FormulaFailure> {
        let values = match elements {
            GroundAggregate::Numeric(elements) => crate::formula_assignment::candidates(
                function,
                elements.iter().map(|element| element.weight),
                self.limits,
                &mut self.counters,
                location,
            )?,
            GroundAggregate::Extrema(elements) => crate::formula_assignment::extrema_candidates(
                function,
                elements.iter().map(|element| &element.value),
                self.limits,
                self.budget,
                &mut self.counters,
                location,
            )?,
        };
        ceiling(
            FormulaResource::AggregateCacheRoots,
            self.cached_roots as u128 + values.len() as u128,
            self.limits.max_aggregate_cache_roots as u128,
            location,
        )?;
        if let GroundAggregate::Extrema(elements) = elements {
            let kind = extremum(function).expect("value aggregate is min/max");
            let mut roots = Vec::new();
            for value in values {
                self.work(location)?;
                let root =
                    self.extremum_root(elements, kind, AggregateComparison::Eq, &value, location)?;
                roots.push((value, root));
            }
            self.cached_roots += roots.len();
            return Ok(Arc::from(roots));
        }
        let GroundAggregate::Numeric(elements) = elements else {
            unreachable!("handled extrema")
        };
        let mut guards = Vec::new();
        for value in &values {
            self.work(location)?;
            let Value::Number(value) = value else {
                unreachable!("count/sum proposals are numeric")
            };
            guards.push(NumericGuard {
                comparison: AggregateComparison::Eq,
                bound: i64::from(*value),
            });
        }
        let limits = AggregateFamilyLimits {
            aggregate: self.aggregate_limits(),
            max_guards: self.limits.max_assignment_values,
        };
        let first = self.nodes.len();
        let build = append_aggregate_family(
            &mut self.nodes,
            elements,
            &guards,
            limits,
            &zetesis_cpu::Control::default(),
        )
        .map_err(|error| FormulaFailure::Aggregate { error, location })?;
        self.counters.work += build.statistics().work;
        let canonical = self.intern_appended(first, location)?;
        let mut roots = Vec::new();
        for (value, root) in values.into_iter().zip(build.roots()) {
            self.work(location)?;
            roots.push((value, remap(*root, first, &canonical)));
        }
        self.cached_roots += roots.len();
        Ok(Arc::from(roots))
    }
    fn cache_key(
        &mut self,
        id: usize,
        target: usize,
        assignment: &Binding,
        location: Location,
    ) -> Result<(AggregateContext, u128), FormulaFailure> {
        let bytes = std::mem::size_of::<usize>() as u128
            + assignment
                .slots()
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != target)
                .map(|(_, value)| {
                    std::mem::size_of::<Option<Value>>() as u128
                        + value.as_ref().map_or(0, value_bytes)
                })
                .sum::<u128>();
        ceiling(
            FormulaResource::AggregateCacheKeys,
            bytes,
            self.limits.max_aggregate_cache_key_bytes as u128,
            location,
        )?;
        let mut outer = Vec::new();
        for (index, value) in assignment.slots().iter().enumerate() {
            self.work(location)?;
            if index != target {
                self.budget.charge(
                    ExpansionResource::ScalarBytes,
                    value.as_ref().map_or(0, value_bytes),
                    location,
                )?;
                outer.push(value.clone());
            }
        }
        Ok((
            AggregateContext {
                aggregate: id,
                outer,
            },
            bytes,
        ))
    }
    fn aggregate_elements(
        &mut self,
        aggregate: &AggregateIr,
        assignment: &Binding,
        support: &Support,
        location: Location,
    ) -> Result<GroundAggregate, FormulaFailure> {
        let is_extremum = extremum(aggregate.function).is_some();
        let mut grouped = BTreeMap::<GroundKey, (Value, usize)>::new();
        for element in &aggregate.elements {
            let mut local = Join::new(
                &element.condition,
                assignment,
                element.variables,
                support,
                self.budget,
                location,
            )?;
            while let Some(binding) =
                local.next(self.limits, self.budget, &mut self.counters, location)?
            {
                let key = self.aggregate_key(&element.key, &binding, location)?;
                let weight = match &key {
                    GroundKey::Tuple(tuple) if is_extremum => {
                        let value = tuple.first().expect("admitted nonempty extremum tuple");
                        crate::formula_assignment::extremum_value(value, location)?;
                        formula_support::copy(value, self.budget, location)?
                    }
                    GroundKey::Tuple(tuple) => {
                        let Some(weight) = crate::formula_assignment::contribution(
                            aggregate.function,
                            tuple.first(),
                            location,
                        )?
                        else {
                            continue;
                        };
                        Value::Number(weight)
                    }
                    GroundKey::Atom(_) => Value::Number(1),
                };
                let condition = self.body(&element.condition, &binding, location, support)?;
                let previous = grouped
                    .get(&key)
                    .map_or(FALSUM, |(_, condition)| *condition);
                if !grouped.contains_key(&key) {
                    ceiling(
                        FormulaResource::AggregateElements,
                        grouped.len() as u128 + 1,
                        self.limits.aggregate.max_elements as u128,
                        location,
                    )?;
                }
                let condition = self.or(previous, condition, location)?;
                grouped.insert(key, (weight, condition));
            }
        }
        if is_extremum {
            return Ok(GroundAggregate::Extrema(
                grouped
                    .into_values()
                    .map(|(value, condition)| ValueExtremumElement { value, condition })
                    .collect::<Vec<_>>()
                    .into(),
            ));
        }
        let elements: Vec<_> = grouped
            .into_values()
            .map(|(value, condition)| {
                let Value::Number(weight) = value else {
                    unreachable!("numeric contribution")
                };
                AggregateElement { weight, condition }
            })
            .collect();
        Ok(GroundAggregate::Numeric(elements.into()))
    }
    fn aggregate_key(
        &mut self,
        key: &AggregateKey,
        assignment: &Binding,
        location: Location,
    ) -> Result<GroundKey, FormulaFailure> {
        match key {
            AggregateKey::Tuple(terms) => {
                let mut values = Vec::new();
                for term in terms {
                    self.work(location)?;
                    let value = assignment.resolve(term, location)?;
                    self.budget.charge(
                        ExpansionResource::ScalarBytes,
                        value_bytes(value),
                        location,
                    )?;
                    values.push(value.clone());
                }
                Ok(GroundKey::Tuple(values))
            }
            AggregateKey::Atom(pattern) => {
                self.work(location)?;
                let bytes: u128 = pattern
                    .terms()
                    .iter()
                    .map(|term| assignment.resolve(term, location).map(value_bytes))
                    .sum::<Result<u128, _>>()?;
                self.budget.charge(
                    ExpansionResource::ScalarBytes,
                    bytes + pattern.predicate().name().len() as u128,
                    location,
                )?;
                Ok(GroundKey::Atom(assignment.instantiate(pattern, location)?))
            }
        }
    }
    fn intern_appended(
        &mut self,
        first: usize,
        location: Location,
    ) -> Result<Vec<usize>, FormulaFailure> {
        let appended = self.nodes.split_off(first);
        let mut canonical = Vec::with_capacity(appended.len());
        for node in appended {
            self.work(location)?;
            let map = |index: usize| remap(index, first, &canonical);
            // Only structural sharing and intuitionistic constant/identity laws
            // are used; no classical eligibility simplification is permitted.
            let index = match node {
                Node::False => FALSUM,
                Node::Atom(atom) => self.node(Node::Atom(atom), location)?,
                Node::And(left, right) => self.and(map(left), map(right), location)?,
                Node::Or(left, right) => self.or(map(left), map(right), location)?,
                Node::Implies(left, right) => {
                    self.node(Node::Implies(map(left), map(right)), location)?
                }
            };
            canonical.push(index);
        }
        Ok(canonical)
    }
    fn aggregate_limits(&self) -> zetesis_ferraris::AggregateLimits {
        let mut limits = self.limits.aggregate;
        limits.max_nodes = limits.max_nodes.min(self.node_bound().1);
        limits.max_work = limits
            .max_work
            .min(self.limits.max_work - self.counters.work);
        limits
    }
    fn aggregate_guards(
        &mut self,
        elements: &GroundAggregate,
        guards: &[AggregateGuard],
        assignment: &Binding,
        kind: Option<AggregateExtremum>,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        self.aggregate_guards_with_capture(elements, guards, assignment, kind, location, None)
    }
    fn aggregate_guards_with_capture(
        &mut self,
        elements: &GroundAggregate,
        guards: &[AggregateGuard],
        assignment: &Binding,
        kind: Option<AggregateExtremum>,
        location: Location,
        mut capture: Option<&mut crate::formula_count_plan::Bounds>,
    ) -> Result<usize, FormulaFailure> {
        let mut result = VERUM;
        for guard in guards {
            let bound = formula_support::expression(
                &guard.bound,
                assignment,
                self.limits,
                self.budget,
                &mut self.counters,
                location,
            )?;
            if let Some(kind) = kind {
                let GroundAggregate::Extrema(elements) = elements else {
                    unreachable!("extrema contribution")
                };
                let root = self.extremum_root(
                    elements,
                    kind,
                    aggregate_comparison(guard.relation),
                    &bound,
                    location,
                )?;
                result = self.and(result, root, location)?;
                continue;
            }
            let GroundAggregate::Numeric(elements) = elements else {
                unreachable!("numeric contribution")
            };
            let bound = match numeric_comparison(guard.relation, &bound) {
                NumericComparison::Threshold(bound) => bound,
                NumericComparison::Constant(truth) => {
                    self.work(location)?;
                    if let Some(capture) = capture.as_deref_mut() {
                        capture.exclude_logical_guard();
                    }
                    result = self.and(result, boolean(truth), location)?;
                    continue;
                }
            };
            if let Some(capture) = capture.as_deref_mut() {
                capture.guard(aggregate_comparison(guard.relation), bound);
            }
            let limits = self.aggregate_limits();
            let first = self.nodes.len();
            let build = append_aggregate(
                &mut self.nodes,
                elements,
                aggregate_comparison(guard.relation),
                i64::from(bound),
                limits,
                &zetesis_cpu::Control::default(),
            )
            .map_err(|error| FormulaFailure::Aggregate { error, location })?;
            self.counters.work += build.statistics().work;
            let canonical = self.intern_appended(first, location)?;
            let root = remap(build.root(), first, &canonical);
            result = self.and(result, root, location)?;
        }
        Ok(result)
    }
}

/// A finite numeric measure either needs a numeric threshold or has the same
/// comparison truth for every selected tuple subset. This is a canonical
/// aggregate equivalence, not merely a classical tautology simplification.
enum NumericComparison {
    Threshold(i32),
    Constant(bool),
}

fn numeric_comparison(
    relation: themelios_program::program::Relation,
    bound: &Value,
) -> NumericComparison {
    if let Value::Number(bound) = bound {
        return NumericComparison::Threshold(*bound);
    }
    // Every integer has the same order against a nonnumeric logical value.
    // Zero represents that term class only; it neither replaces the measure
    // nor encodes an extremal value. This comparison borrows and allocates nothing.
    NumericComparison::Constant(formula_support::compare(&Value::Number(0), relation, bound))
}

fn remap(index: usize, first: usize, canonical: &[usize]) -> usize {
    if index < first {
        index
    } else {
        canonical[index - first]
    }
}
fn aggregate_comparison(relation: themelios_program::program::Relation) -> AggregateComparison {
    use themelios_program::program::Relation;
    match relation {
        Relation::Eq => AggregateComparison::Eq,
        Relation::Neq => AggregateComparison::Ne,
        Relation::Lt => AggregateComparison::Lt,
        Relation::Le => AggregateComparison::Le,
        Relation::Gt => AggregateComparison::Gt,
        Relation::Ge => AggregateComparison::Ge,
    }
}

fn extremum(function: AggregateFunction) -> Option<AggregateExtremum> {
    match function {
        AggregateFunction::Min => Some(AggregateExtremum::Min),
        AggregateFunction::Max => Some(AggregateExtremum::Max),
        _ => None,
    }
}
impl Builder<'_> {
    fn extremum_root(
        &mut self,
        elements: &[ValueExtremumElement],
        kind: AggregateExtremum,
        comparison: AggregateComparison,
        bound: &Value,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        crate::formula_assignment::extremum_value(bound, location)?;
        let first = self.nodes.len();
        let limits = self.aggregate_limits();
        let build = append_value_extremum(
            &mut self.nodes,
            elements,
            kind,
            comparison,
            bound,
            limits,
            &zetesis_cpu::Control::default(),
        )
        .map_err(|error| FormulaFailure::Aggregate { error, location })?;
        self.counters.work += build.statistics().work;
        let canonical = self.intern_appended(first, location)?;
        Ok(remap(build.root(), first, &canonical))
    }
}
