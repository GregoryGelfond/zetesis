//! Finite substitutions and support-preserving conditional-choice formulas.

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
use crate::formula_ir::{
    AggregateGuard, AggregateIr, AggregateKey, Element, HeadIr, LiteralIr, ObjectiveIr, Prepared,
    Projection, RuleIr, value_bytes,
};
use crate::formula_support::{self, Counters, Join, Support};
use crate::grounding_observer::Event;
use crate::{ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource};

pub(crate) fn ground(
    prepared: Prepared,
    limits: FormulaLimits,
    budget: &mut Budget,
    location: Location,
    observer: Option<&dyn crate::GroundingObserver>,
) -> Result<Compiled, FormulaFailure> {
    use crate::GroundingPhase;

    let profile = crate::grounding_observer::Profile::new(observer);
    let mut counters = Counters::observed(profile.work());
    let support = profile.phase(GroundingPhase::SupportCompletion, None, || {
        formula_support::build(&prepared, limits, budget, &mut counters, location)
    })?;
    let (objectives, objective_origins) =
        profile.phase(GroundingPhase::ObjectiveActivation, None, || {
            activate_objectives(&prepared, &support, limits, budget, &mut counters, location)
        })?;
    let mut builder = profile.phase(GroundingPhase::FormulaInitialization, None, || {
        let mut builder = Builder {
            limits,
            budget,
            atoms: Vec::new(),
            atom_indices: BTreeMap::new(),
            producers: Vec::new(),
            producer_origins: Vec::new(),
            atom_locations: Vec::new(),
            nodes: Vec::new(),
            node_indices: BTreeMap::new(),
            roots: Vec::new(),
            origins: Vec::new(),
            counters,
            origin_count: 0,
            aggregate_cache: BTreeMap::new(),
            cached_elements: 0,
            cached_key_bytes: 0,
            cached_roots: 0,
        };
        builder.node(Node::False, location)?;
        builder.node(Node::Implies(0, 0), location)?;
        Ok::<_, FormulaFailure>(builder)
    })?;
    for rule in &prepared.rules {
        profile.phase(
            GroundingPhase::RuleInstantiation,
            Some(rule.location),
            || {
                if crate::formula_factor::rule(&mut builder, rule, &support)? {
                    return Ok(());
                }
                let mut outer = Join::new(
                    &rule.body,
                    &[],
                    rule.variables,
                    &support,
                    builder.budget,
                    rule.location,
                )?;
                while let Some(binding) =
                    outer.next(limits, builder.budget, &mut builder.counters, rule.location)?
                {
                    builder.rule(rule, &binding, &support)?;
                }
                Ok::<_, FormulaFailure>(())
            },
        )?;
    }
    profile.phase(GroundingPhase::Coherence, None, || builder.coherence())?;
    profile.phase(GroundingPhase::SupportGuards, None, || {
        builder.support_guards()
    })?;
    let theory = profile.phase(GroundingPhase::TheoryValidation, None, || {
        Theory::new(
            builder.atoms.len(),
            builder.nodes,
            builder.roots,
            limits.theory,
        )
        .map_err(|error| FormulaFailure::Theory { error, location })
    })?;
    Ok(Compiled {
        analysis: prepared.analysis,
        analyzed: prepared.analyzed,
        theory,
        atoms: builder.atoms,
        origins: builder.origins,
        objectives,
        objective_origins,
        objective_declarations: prepared.objective_declarations.clone(),
    })
}

fn activate_objectives(
    prepared: &Prepared,
    support: &Support,
    limits: FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<(zetesis_objective::ObjectiveProgram, Vec<Vec<Location>>), FormulaFailure> {
    let mut active = Vec::new();
    let mut objective_origins = Vec::new();
    for objective in &prepared.objectives {
        if objective_active(objective, support, limits, budget, counters)? {
            budget.charge(
                ExpansionResource::Origins,
                objective.origins.len() as u128,
                objective.location,
            )?;
            active.push(objective.template.clone());
            objective_origins.push(objective.origins.clone());
        }
    }
    let objectives = if active.is_empty() {
        zetesis_objective::ObjectiveProgram::none()
    } else {
        zetesis_objective::ObjectiveProgram::new(active, limits.objective)
            .map_err(|error| FormulaFailure::Objective { error, location })?
    };
    Ok::<_, FormulaFailure>((objectives, objective_origins))
}

fn objective_active(
    objective: &ObjectiveIr,
    support: &Support,
    limits: FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    let mut bindings = Join::new(
        &objective.condition,
        &[],
        objective.variables,
        support,
        budget,
        objective.location,
    )?;
    let mut numeric = false;
    while let Some(binding) = bindings.next(limits, budget, counters, objective.location)? {
        counters.work(limits, objective.location)?;
        if let Ok(Value::Number(weight)) = objective.template.weight().resolve(&binding) {
            objective
                .template
                .weight_polarity()
                .normalize(*weight)
                .ok_or_else(|| {
                    crate::diagnostic::unsupported(
                        crate::ProfileFeature::NumericOverflow,
                        objective.location,
                    )
                })?;
            numeric = true;
            // Maximizing templates must check every eligible numeric row:
            // a later MIN value cannot be hidden by an earlier valid weight.
            if objective.template.weight_polarity() == zetesis_objective::WeightPolarity::AsWritten
            {
                break;
            }
        }
    }
    Ok(numeric)
}

pub(super) struct Builder<'a> {
    pub(super) limits: FormulaLimits,
    pub(super) budget: &'a mut Budget,
    atoms: Vec<Atom>,
    atom_indices: BTreeMap<Atom, usize>,
    producers: Vec<Vec<usize>>,
    producer_origins: Vec<BTreeSet<Location>>,
    atom_locations: Vec<Location>,
    nodes: Vec<Node>,
    node_indices: BTreeMap<(u8, usize, usize), usize>,
    roots: Vec<usize>,
    origins: Vec<Vec<Location>>,
    pub(super) counters: Counters,
    origin_count: usize,
    aggregate_cache: BTreeMap<(usize, Vec<Value>), CachedAggregate>,
    cached_elements: usize,
    cached_key_bytes: u128,
    cached_roots: usize,
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
    // Only the completed atom catalog establishes which opposite tuples can
    // coexist. Reuse its IDs; coherence must create neither atoms nor support.
    fn coherence(&mut self) -> Result<(), FormulaFailure> {
        for index in 0..self.atoms.len() {
            let atom = &self.atoms[index];
            if atom.predicate().sign() != zetesis_core::Sign::Negative {
                continue;
            }
            let location = self.atom_locations[index];
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
            if let Some(&other) = self.atom_indices.get(&opposite) {
                let positive = self.node(Node::Atom(other), location)?;
                let negative = self.node(Node::Atom(index), location)?;
                let both = self.and(positive, negative, location)?;
                let constraint = self.neg(both, location)?;
                let origins = [location, self.atom_locations[other]];
                let evidence = if origins[0] == origins[1] {
                    &origins[..1]
                } else {
                    &origins[..]
                };
                self.root_at(constraint, evidence, location)?;
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
        let key = node_key(node);
        if let Some(index) = self.node_indices.get(&key) {
            return Ok(*index);
        }
        ceiling(
            FormulaResource::Nodes,
            self.nodes.len() as u128 + 1,
            self.limits.theory.max_nodes as u128,
            location,
        )?;
        let index = self.nodes.len();
        self.nodes.push(node);
        self.node_indices.insert(key, index);
        self.counters.record(Event::NodeInserted);
        Ok(index)
    }
    pub(super) fn and(
        &mut self,
        left: usize,
        right: usize,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        if left == 0 || right == 0 {
            Ok(0)
        } else if left == 1 {
            Ok(right)
        } else if right == 1 || left == right {
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
        if left == 1 || right == 1 {
            Ok(1)
        } else if left == 0 {
            Ok(right)
        } else if right == 0 || left == right {
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
        self.node(Node::Implies(formula, 0), location)
    }
    pub(super) fn root(&mut self, formula: usize, rule: &RuleIr) -> Result<(), FormulaFailure> {
        self.root_at(formula, &rule.origins, rule.location)
    }
    fn root_at(
        &mut self,
        formula: usize,
        evidence: &[Location],
        location: Location,
    ) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::Roots,
            self.roots.len() as u128 + 1,
            self.limits.theory.max_roots as u128,
            location,
        )?;
        let origins = self.origin_count as u128 + evidence.len() as u128;
        ceiling(
            FormulaResource::Origins,
            origins,
            self.limits.max_origin_locations as u128,
            location,
        )?;
        self.origin_count += evidence.len();
        self.roots.push(formula);
        self.origins.push(evidence.to_vec());
        self.counters.record(Event::Root);
        Ok(())
    }
    pub(super) fn atom(
        &mut self,
        pattern: &AtomPattern,
        assignment: &[Value],
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        self.work(location)?;
        let mut bytes = pattern.predicate().name().len() as u128;
        for term in pattern.terms() {
            bytes += value_bytes(
                term.resolve(assignment)
                    .expect("all scoped variables assigned"),
            );
        }
        // One temporary identity plus at most two retained copies; charge before cloning.
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            bytes.saturating_mul(3),
            location,
        )?;
        let atom = pattern
            .instantiate(assignment)
            .expect("all scoped variables assigned");
        self.counters.record(Event::AtomLookup);
        let index = if let Some(index) = self.atom_indices.get(&atom) {
            *index
        } else {
            ceiling(
                FormulaResource::Atoms,
                self.atoms.len() as u128 + 1,
                self.limits.theory.max_atoms as u128,
                location,
            )?;
            let index = self.atoms.len();
            self.budget
                .charge(ExpansionResource::Origins, 1, location)?;
            self.atoms.push(atom.clone());
            self.producers.push(Vec::new());
            self.producer_origins.push(BTreeSet::from([location]));
            self.atom_locations.push(location);
            self.atom_indices.insert(atom, index);
            self.counters.record(Event::AtomInserted);
            index
        };
        self.node(Node::Atom(index), location)
    }
    pub(super) fn body(
        &mut self,
        literals: &[LiteralIr],
        assignment: &[Value],
        location: Location,
        support: &Support,
    ) -> Result<usize, FormulaFailure> {
        let mut result = 1;
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
    fn project(
        &mut self,
        projection: &Projection,
        assignment: &[Value],
        support: &Support,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        let mut result = 0;
        for atom in support.rows(&projection.predicate) {
            self.work(location)?;
            let mut matches = true;
            for (term, value) in projection.terms.iter().zip(atom.values()) {
                self.work(location)?;
                if let Some(term) = term {
                    matches &= term
                        .resolve(assignment)
                        .expect("named projection arguments are safe")
                        == value;
                }
            }
            if matches {
                self.budget.charge(
                    ExpansionResource::ScalarBytes,
                    atom.predicate().name().len() as u128
                        + atom.values().iter().map(value_bytes).sum::<u128>(),
                    location,
                )?;
                let pattern = AtomPattern::new(
                    atom.predicate().clone(),
                    atom.values()
                        .iter()
                        .cloned()
                        .map(zetesis_core::Term::Constant)
                        .collect(),
                )
                .expect("projection retains the source atom arity");
                let atom = self.atom(&pattern, &[], location)?;
                result = self.or(result, atom, location)?;
            }
        }
        Ok(result)
    }
    fn rule(
        &mut self,
        rule: &RuleIr,
        assignment: &[Value],
        support: &Support,
    ) -> Result<(), FormulaFailure> {
        self.work(rule.location)?;
        let body = self.body(&rule.body, assignment, rule.location, support)?;
        if body == 0 {
            return Ok(());
        }
        match &rule.head {
            HeadIr::Normal(head) => {
                let head = match head {
                    Some(head) => self.atom(head, assignment, rule.location)?,
                    None => 0,
                };
                let formula = self.node(Node::Implies(body, head), rule.location)?;
                self.root(formula, rule)?;
                if head != 0 {
                    self.producer(head, body, rule)?;
                }
                Ok(())
            }
            HeadIr::Disjunction(heads) => {
                let mut disjunction = 0;
                let mut distinct = BTreeSet::new();
                for head in heads {
                    let atom = self.atom(&head.atom, assignment, rule.location)?;
                    let mut literal = atom;
                    if head.negation != DefaultNegation::None {
                        literal = self.neg(literal, rule.location)?;
                    }
                    if head.negation == DefaultNegation::NotNot {
                        literal = self.neg(literal, rule.location)?;
                    }
                    if distinct.insert(literal) {
                        disjunction = self.or(disjunction, literal, rule.location)?;
                        // Necessary support is the original body for each head,
                        // not a shifted rule excluding the other disjuncts.
                        if head.positive_atom().is_some() {
                            self.producer(atom, body, rule)?;
                        } else {
                            // A negative occurrence contributes provenance for
                            // its atom's guard, without supplying any support.
                            self.head_origins(atom, rule)?;
                        }
                    }
                }
                let formula = self.node(Node::Implies(body, disjunction), rule.location)?;
                self.root(formula, rule)
            }
            HeadIr::Choice { guards, elements } => {
                self.choice(rule, body, guards, elements, assignment, support)
            }
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
        self.producers[atom].push(antecedent);
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
            if !self.producer_origins[atom].contains(&location) {
                self.budget
                    .charge(ExpansionResource::Origins, 1, location)?;
                self.producer_origins[atom].insert(location);
            }
        }
        Ok(())
    }
    fn support_guards(&mut self) -> Result<(), FormulaFailure> {
        for atom in 0..self.atoms.len() {
            let location = self.atom_locations[atom];
            let mut supported = 0;
            for antecedent in std::mem::take(&mut self.producers[atom]) {
                supported = self.or(supported, antecedent, location)?;
            }
            let head = self.node(Node::Atom(atom), location)?;
            let necessary = self.node(Node::Implies(head, supported), location)?;
            let negative = self.neg(necessary, location)?;
            let guard = self.neg(negative, location)?;
            let origins: Vec<_> = std::mem::take(&mut self.producer_origins[atom])
                .into_iter()
                .collect();
            self.root_at(guard, &origins, location)?;
        }
        Ok(())
    }
    fn choice(
        &mut self,
        rule: &RuleIr,
        body: usize,
        guards: &[AggregateGuard],
        elements: &[Element],
        assignment: &[Value],
        support: &Support,
    ) -> Result<(), FormulaFailure> {
        crate::formula_count_head::validate_group(
            elements,
            assignment,
            support,
            self.limits,
            self.budget,
            &mut self.counters,
            rule.location,
        )?;
        let mut eligible = BTreeMap::new();
        for element in elements {
            let mut local = Join::new(
                &element.condition,
                assignment,
                element.variables,
                support,
                self.budget,
                rule.location,
            )?;
            while let Some(binding) =
                local.next(self.limits, self.budget, &mut self.counters, rule.location)?
            {
                let condition = self.body(&element.condition, &binding, rule.location, support)?;
                let head = self.atom(&element.head, &binding, rule.location)?;
                let previous = eligible.get(&head).copied().unwrap_or(0);
                eligible.insert(head, self.or(previous, condition, rule.location)?);
            }
        }
        let mut selected = Vec::new();
        for (head, condition) in eligible {
            let antecedent = self.and(body, condition, rule.location)?;
            let negative = self.neg(head, rule.location)?;
            let choice = self.or(head, negative, rule.location)?;
            let support = self.node(Node::Implies(antecedent, choice), rule.location)?;
            self.root(support, rule)?;
            self.producer(head, antecedent, rule)?;
            if !guards.is_empty() {
                ceiling(
                    FormulaResource::AggregateElements,
                    selected.len() as u128 + 1,
                    self.limits.aggregate.max_elements as u128,
                    rule.location,
                )?;
                selected.push(AggregateElement {
                    weight: 1,
                    condition: self.and(condition, head, rule.location)?,
                });
            }
        }
        if !guards.is_empty() {
            let within = self.aggregate_guards(
                &GroundAggregate::Numeric(selected.into()),
                guards,
                assignment,
                None,
                rule.location,
            )?;
            let outside = self.neg(within, rule.location)?;
            let violated = self.and(body, outside, rule.location)?;
            let constraint = self.node(Node::Implies(violated, 0), rule.location)?;
            self.root(constraint, rule)?;
        }
        Ok(())
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
        assignment: &[Value],
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
        assignment: &[Value],
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
        assignment: &[Value],
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
        let index = roots
            .binary_search_by(|(value, _)| value.cmp(&assignment[target]))
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
        assignment: &[Value],
        location: Location,
    ) -> Result<((usize, Vec<Value>), u128), FormulaFailure> {
        let bytes = std::mem::size_of::<usize>() as u128
            + assignment
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != target)
                .map(|(_, value)| std::mem::size_of::<Value>() as u128 + value_bytes(value))
                .sum::<u128>();
        ceiling(
            FormulaResource::AggregateCacheKeys,
            bytes,
            self.limits.max_aggregate_cache_key_bytes as u128,
            location,
        )?;
        let mut outer = Vec::new();
        for (index, value) in assignment.iter().enumerate() {
            self.work(location)?;
            if index != target {
                self.budget
                    .charge(ExpansionResource::ScalarBytes, value_bytes(value), location)?;
                outer.push(value.clone());
            }
        }
        Ok(((id, outer), bytes))
    }
    fn aggregate_elements(
        &mut self,
        aggregate: &AggregateIr,
        assignment: &[Value],
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
                        let Some(weight) = crate::formula_assignment::tuple_weight(
                            aggregate.function,
                            tuple,
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
                let previous = grouped.get(&key).map_or(0, |(_, condition)| *condition);
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
        assignment: &[Value],
        location: Location,
    ) -> Result<GroundKey, FormulaFailure> {
        match key {
            AggregateKey::Tuple(terms) => {
                let mut values = Vec::new();
                for term in terms {
                    self.work(location)?;
                    let value = term
                        .resolve(assignment)
                        .expect("safe aggregate variable assigned");
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
                    .map(|term| {
                        value_bytes(
                            term.resolve(assignment)
                                .expect("safe aggregate variable assigned"),
                        )
                    })
                    .sum();
                self.budget.charge(
                    ExpansionResource::ScalarBytes,
                    bytes + pattern.predicate().name().len() as u128,
                    location,
                )?;
                Ok(GroundKey::Atom(
                    pattern
                        .instantiate(assignment)
                        .expect("safe aggregate variable assigned"),
                ))
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
                Node::False => 0,
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
        limits.max_nodes = limits.max_nodes.min(self.limits.theory.max_nodes);
        limits.max_work = limits
            .max_work
            .min(self.limits.max_work - self.counters.work);
        limits
    }
    fn aggregate_guards(
        &mut self,
        elements: &GroundAggregate,
        guards: &[AggregateGuard],
        assignment: &[Value],
        kind: Option<AggregateExtremum>,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        let mut result = 1;
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
            let Value::Number(bound) = bound else {
                return Err(crate::diagnostic::unsupported(
                    crate::ProfileFeature::Aggregate,
                    location,
                )
                .into());
            };
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
fn remap(index: usize, first: usize, canonical: &[usize]) -> usize {
    if index < first {
        index
    } else {
        canonical[index - first]
    }
}
fn node_key(node: Node) -> (u8, usize, usize) {
    match node {
        Node::False => (0, 0, 0),
        Node::Atom(a) => (1, a, 0),
        Node::And(a, b) => (2, a, b),
        Node::Or(a, b) => (3, a, b),
        Node::Implies(a, b) => (4, a, b),
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
