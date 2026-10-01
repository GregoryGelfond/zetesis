//! Finite substitutions and support-preserving conditional-choice formulas.

mod arithmetic;
mod objectives;
mod scoped_body;
pub(crate) use scoped_body::source_activity;
mod aggregate_guards;
mod aggregate_order;
pub(crate) mod atoms;
mod cache;
mod metadata;
mod nodes;
mod projection;
mod retained;
use retained::RetainedState;
pub(crate) use retained::{RetainedGrounding, ground_retained};
#[cfg(test)]
mod constants;

use crate::formula_support::family::Warnings;
use crate::formula_support::{Context, GroundingWork};

use cache::{Contexts, CoordinateMap};
use std::borrow::Cow;
use std::sync::Arc;

use crate::formula_support::components::{self, Pattern as AtomPattern};
use themelios_base::span::Location;
use themelios_program::program::{AggregateFunction, DefaultNegation};
use zetesis_core::catalog::{TermKey, TermRef};
use zetesis_core::{AtomCatalog, ValueNodeRef};
use zetesis_ferraris::{
    AggregateComparison, AggregateElement, AggregateExtremum, AggregateGuard as NumericGuard, Node,
    Theory, ValueExtremumElement, append_aggregate, append_value_extremum_refs,
};

use crate::expansion::Budget;
use crate::formula::{Compiled, ceiling};
use crate::formula_binding::Binding;
use crate::formula_ir::{
    AggregateGuard, AggregateIr, AggregateKey, ChoiceIr, HeadElementKey, HeadIr, HeadLiteral,
    HeadMeasure, HeadOperand, LiteralIr, Projection, RuleIr,
};
use crate::formula_support::{self, Buffer, Computation, Counters, Join, Support, TermTable};
use crate::grounding_observer::{Event, Profile};
use crate::{ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource};

/// Canonical Boolean nodes established by `Builder::initialize`.
/// Falsum is bottom; verum is the implication from bottom to itself.
pub(super) const FALSUM: usize = 0;
pub(super) const VERUM: usize = 1;

pub(super) const fn boolean(truth: bool) -> usize {
    if truth { VERUM } else { FALSUM }
}

#[derive(Clone, Copy)]
enum Schedule<'a> {
    Eager(Option<crate::formula_count_plan::Request<'a>>),
    Retained,
    Hybrid,
}

impl Schedule<'_> {
    fn retain_constraints(
        self,
        mut rules: Vec<RuleIr>,
        catalog: formula_support::CompletedCatalog,
        instances: u64,
        limits: &FormulaLimits,
        location: Location,
    ) -> Option<crate::formula_hybrid::Constraints> {
        if !matches!(self, Self::Hybrid) {
            return None;
        }
        // Compact the already admitted source vector in place. No instance
        // population or duplicate source representation is retained.
        rules.retain(crate::formula_hybrid::eligible);
        Some(crate::formula_hybrid::Constraints {
            catalog,
            rules,
            instances,
            limits: *limits,
            location,
        })
    }
}

pub(crate) fn ground(
    preparation: crate::formula::Preparation,
    observer: Option<&dyn crate::GroundingObserver>,
    count_plan: Option<crate::formula_count_plan::Request<'_>>,
) -> Result<Compiled, FormulaFailure> {
    ground_with_schedule(preparation, observer, Schedule::Eager(count_plan))
        .map(|grounded| grounded.compiled)
}

pub(crate) fn ground_hybrid(
    preparation: crate::formula::Preparation,
    observer: Option<&dyn crate::GroundingObserver>,
) -> Result<(Compiled, crate::formula_hybrid::Constraints), FormulaFailure> {
    if let Some(&location) = preparation.program.objective_declarations.first() {
        return Err(FormulaFailure::HybridUnsupported {
            feature: crate::HybridFeature::Objectives,
            location,
        });
    }
    if preparation.options.joins != crate::JoinStrategy::Indexed {
        return Err(FormulaFailure::HybridUnsupported {
            feature: crate::HybridFeature::TableJoins,
            location: preparation.location,
        });
    }
    ground_with_schedule(preparation, observer, Schedule::Hybrid).map(|grounded| {
        (
            grounded.compiled,
            grounded
                .constraints
                .expect("hybrid schedule retains its constraints"),
        )
    })
}

struct Grounded {
    compiled: Compiled,
    constraints: Option<crate::formula_hybrid::Constraints>,
    retained: Option<RetainedState>,
}

fn ground_with_schedule(
    preparation: crate::formula::Preparation,
    observer: Option<&dyn crate::GroundingObserver>,
    schedule: Schedule<'_>,
) -> Result<Grounded, FormulaFailure> {
    use crate::GroundingPhase;

    let profile = Profile::new(observer);
    let keyed_constraints = preparation.program.keyed_constraints;
    let key_analysis = preparation.program.key_analysis;
    let theory_limits = preparation.limits.theory;
    let location = preparation.location;
    let Instantiation {
        projection,
        emission,
        objectives,
        objective_origins,
        analysis_basis,
        analysis,
        analyzed,
        objective_declarations,
        warnings,
        constraints,
        expansion,
        retained,
    } = instantiate(preparation, &profile, schedule)?;
    let Emission {
        atoms,
        nodes,
        roots,
        origins,
        count_plan,
    } = emission;
    let theory = profile.phase(GroundingPhase::TheoryValidation, None, || {
        Theory::new(atoms.atoms().len(), nodes, roots, theory_limits)
            .map_err(|error| FormulaFailure::Theory { error, location })
    })?;
    let count_plan = count_plan.map_or(
        crate::formula_count_plan::Outcome::NotRequested,
        |collector| collector.finish(&theory),
    );
    Ok(Grounded {
        compiled: Compiled {
            warnings,
            projection,
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
            keyed_constraints,
            key_analysis,
            expansion,
        },
        constraints,
        retained,
    })
}

/// Emitted occurrences and completed support select one canonical source
/// authority independently. Eager construction releases lookup/support indexes;
/// hybrid construction retains them with the unmaterialized constraint templates.
struct Instantiation {
    expansion: crate::ExpansionUsage,
    projection: crate::PreparedProjection,
    emission: Emission,
    objectives: zetesis_objective::ObjectiveProgram,
    objective_origins: Vec<Vec<Location>>,
    analysis_basis: crate::AnalysisBasis,
    analysis: themelios_analysis::Analysis,
    analyzed: themelios_program::program::Program,
    objective_declarations: Vec<Location>,
    warnings: Vec<crate::FormulaWarning>,
    constraints: Option<crate::formula_hybrid::Constraints>,
    retained: Option<RetainedState>,
}

fn instantiate(
    preparation: crate::formula::Preparation,
    profile: &Profile<'_>,
    schedule: Schedule<'_>,
) -> Result<Instantiation, FormulaFailure> {
    use crate::GroundingPhase;

    let crate::formula::Preparation {
        program: prepared,
        catalog: source_catalog,
        accounting,
        budget: mut expansion_budget,
        limits,
        location,
        options,
    } = preparation;
    let limits = &limits;
    let budget = &mut expansion_budget;
    let mut counters = Counters::resume(accounting, profile.work());
    let (mut catalog, domains) = complete_support(
        &prepared,
        source_catalog,
        options.domains,
        budget,
        profile,
        GroundingWork::new(limits, &mut counters, location),
    )?;
    let pending = {
        let (completed, mut append) =
            profile.phase(GroundingPhase::SupportCompletion, None, || {
                catalog.split(limits, &mut counters, location)
            })?;
        let queries = profile.phase(GroundingPhase::SupportCompletion, None, || {
            completed.queries(options.joins, limits, &counters, location)
        })?;
        let mut computation = Computation::new(&mut append, queries.support());
        emit(
            &prepared,
            &queries,
            domains.as_ref(),
            profile,
            schedule,
            budget,
            Context::new(&mut computation, limits, &mut counters, location),
        )?
    };
    drop(domains);
    let PublishedEmission {
        projection,
        emission,
        objectives,
        objective_origins,
        warnings,
        streamed_instances,
        retained_account,
    } = pending.publish(
        &mut catalog,
        limits,
        location,
        matches!(schedule, Schedule::Retained),
    )?;
    let expansion = budget.usage();
    let (constraints, retained) = if let Some((accounting, output_storage)) = retained_account {
        (
            None,
            Some(RetainedState {
                catalog,
                accounting,
                budget: expansion_budget,
                output_storage,
            }),
        )
    } else {
        (
            schedule.retain_constraints(
                prepared.rules,
                catalog,
                streamed_instances,
                limits,
                location,
            ),
            None,
        )
    };
    Ok(Instantiation {
        expansion,
        projection,
        emission,
        objectives,
        objective_origins,
        analysis_basis: prepared.analysis_basis,
        analysis: prepared.analysis,
        analyzed: prepared.analyzed,
        objective_declarations: prepared.objective_declarations,
        warnings: warnings.into_values(),
        constraints,
        retained,
    })
}

/// Prepare optional domains and complete support under the same cumulative
/// account. The domain certificate remains tied to the prepared program.
fn complete_support<'source>(
    prepared: &'source crate::formula_ir::Prepared,
    mut catalog: formula_support::SupportCatalog,
    options: Option<crate::DomainLimits>,
    budget: &mut Budget,
    profile: &Profile<'_>,
    work: GroundingWork<'_>,
) -> Result<
    (
        formula_support::CompletedCatalog,
        Option<crate::formula_domains::Domains<'source>>,
    ),
    FormulaFailure,
> {
    let GroundingWork {
        limits,
        counters,
        location,
    } = work;
    let domains = if options.is_some() {
        profile.phase(crate::GroundingPhase::DomainAnalysis, None, || {
            let (relations, mut append) = catalog.split(limits, counters, location)?;
            let support = Support::indexed(&relations, limits, counters, location)?;
            let mut computation = Computation::new(&mut append, &support);
            crate::formula_domains::analyze(
                prepared,
                options,
                budget,
                profile,
                Context::new(&mut computation, limits, counters, location),
            )
        })?
    } else {
        profile.domain_analysis(crate::DomainObservation::Disabled);
        None
    };
    let catalog = profile.phase(crate::GroundingPhase::SupportCompletion, None, || {
        formula_support::build(
            catalog,
            prepared,
            domains.as_ref(),
            limits,
            budget,
            counters,
            location,
        )
    })?;
    Ok((catalog, domains))
}

/// Completed support may grow canonical terms during emission. Only owned
/// selections and the same cumulative account cross into publication.
struct PendingEmission {
    projection: projection::PendingProjection,
    objectives: objectives::PendingProgram,
    emission: Emission<formula_support::SourceSelection>,
    warnings: Warnings,
    streamed_instances: u64,
    counters: Counters,
}

fn emit<'source>(
    prepared: &crate::formula_ir::Prepared,
    queries: &formula_support::CompletedQueries<'source>,
    domains: Option<&crate::formula_domains::Domains<'_>>,
    profile: &Profile<'_>,
    schedule: Schedule<'_>,
    budget: &mut Budget,
    context: Context<'_, &mut Computation<'_, 'source>>,
) -> Result<PendingEmission, FormulaFailure> {
    use crate::GroundingPhase;
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    let support = queries.support();
    let mut warnings = profile.phase(GroundingPhase::SupportCompletion, None, || {
        arithmetic::prepare(prepared, support, computation, limits, budget, counters)
    })?;
    let objectives = profile.phase(GroundingPhase::ObjectiveActivation, None, || {
        objectives::prepare(
            prepared,
            queries,
            budget,
            &mut warnings,
            Context::new(&mut *computation, limits, counters, location),
        )
    })?;
    let projection = projection::prepare(
        prepared,
        queries,
        computation,
        limits,
        budget,
        counters,
        location,
    )?;
    let mut builder = profile.phase(GroundingPhase::FormulaInitialization, None, || {
        let mut builder = Builder::empty(
            computation,
            limits,
            budget,
            counters,
            Purpose::Theory,
            match schedule {
                Schedule::Eager(request) => request
                    .map(|request| crate::formula_count_plan::Collector::new(request, location)),
                Schedule::Hybrid | Schedule::Retained => None,
            },
            location,
        )?;
        builder.initialize(location)?;
        Ok::<_, FormulaFailure>(builder)
    })?;
    let streamed_instances =
        builder.instantiate_rules(&prepared.rules, domains, support, profile, schedule)?;
    let (emission, counters) = builder.finish(profile)?;
    Ok(PendingEmission {
        projection,
        objectives,
        emission,
        warnings,
        streamed_instances,
        counters,
    })
}

struct PublishedEmission {
    retained_account: Option<(formula_support::Accounting, formula_support::StorageLease)>,
    projection: crate::PreparedProjection,
    emission: Emission,
    objectives: zetesis_objective::ObjectiveProgram,
    objective_origins: Vec<Vec<Location>>,
    warnings: Warnings,
    streamed_instances: u64,
}

impl PendingEmission {
    fn publish(
        self,
        catalog: &mut formula_support::CompletedCatalog,
        limits: &FormulaLimits,
        location: Location,
        retain: bool,
    ) -> Result<PublishedEmission, FormulaFailure> {
        let Self {
            projection,
            objectives,
            emission,
            warnings,
            streamed_instances,
            mut counters,
        } = self;
        let mut publication = formula_support::Publication::new(catalog, &counters, location)?;
        let projection = projection.publish(&mut publication, limits, &mut counters, location)?;
        let Emission {
            atoms,
            nodes,
            roots,
            origins,
            count_plan,
        } = emission;
        let atoms = publication.atoms(atoms, limits, &mut counters, location)?;
        let (objectives, objective_origins) =
            objectives.publish(&atoms, &mut publication, limits, &mut counters, location)?;
        let emission = Emission {
            atoms,
            nodes,
            roots,
            origins,
            count_plan,
        };
        let retained_account = if retain {
            let external = publication.source_bytes(location)?;
            let mut output_storage = publication.into_lease(location)?;
            RetainedGrounding::admit_envelope(
                &mut output_storage,
                external,
                &counters,
                limits,
                location,
            )?;
            Some((counters.into_accounting(), output_storage))
        } else {
            // Preserve the ordinary publication boundary: both coordinator and
            // history drop here, before later theory validation.
            drop(publication);
            drop(counters);
            None
        };
        Ok(PublishedEmission {
            retained_account,
            projection,
            emission,
            objectives,
            objective_origins,
            warnings,
            streamed_instances,
        })
    }
}

/// Only these owned vectors and optional premises survive formula construction.
/// Validation borrows no interning index, producer table or aggregate cache.
struct Emission<A = AtomCatalog> {
    atoms: A,
    nodes: Vec<Node>,
    roots: Vec<usize>,
    origins: Vec<Vec<Location>>,
    count_plan: Option<crate::formula_count_plan::Collector>,
}

pub(super) struct Builder<'a, 'terms, 'source> {
    pub(super) computation: &'a mut Computation<'terms, 'source>,
    purpose: Purpose,
    pub(super) limits: &'a FormulaLimits,
    pub(super) budget: &'a mut Budget,
    catalog: atoms::Catalog,
    terms: TermTable,
    aggregate_atoms: formula_support::SourceSelection,
    metadata: metadata::Metadata,
    nodes: Vec<Node>,
    node_indices: nodes::Index,
    roots: Vec<usize>,
    origins: Vec<Vec<Location>>,
    pub(super) counters: Counters,
    origin_count: usize,
    aggregate_cache: Contexts<CachedAggregate>,
    cached_elements: usize,
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

struct CachedAggregate {
    elements: GroundAggregate,
    roots: Option<Arc<CoordinateMap<usize, usize>>>,
}

#[derive(Clone)]
enum GroundAggregate {
    Numeric(Arc<Buffer<AggregateElement>>),
    Extrema(Arc<Buffer<ExtremumElement>>),
}
impl GroundAggregate {
    fn len(&self) -> usize {
        match self {
            Self::Numeric(elements) => elements.len(),
            Self::Extrema(elements) => elements.len(),
        }
    }
}
impl Builder<'_, '_, '_> {
    fn instantiate_rules(
        &mut self,
        rules: &[RuleIr],
        domains: Option<&crate::formula_domains::Domains<'_>>,
        support: &Support<'_>,
        profile: &Profile<'_>,
        schedule: Schedule<'_>,
    ) -> Result<u64, FormulaFailure> {
        let mut streamed_instances = 0;
        for (index, rule) in rules.iter().enumerate() {
            profile.phase(
                crate::GroundingPhase::RuleInstantiation,
                Some(rule.location),
                || {
                    if matches!(schedule, Schedule::Hybrid) && crate::formula_hybrid::eligible(rule)
                    {
                        self.capture_constraint(rule, support, &mut streamed_instances)
                    } else {
                        self.instantiate_rule(rule, index, domains, support)
                    }
                },
            )?;
        }
        Ok(streamed_instances)
    }

    /// Retain every atom identity a streamed instance can read, without its
    /// conjunction/implication DAG. Existing completion then emits coherence
    /// and support guards, including falsity for atoms with no producer.
    fn capture_constraint(
        &mut self,
        rule: &RuleIr,
        support: &Support<'_>,
        instances: &mut u64,
    ) -> Result<(), FormulaFailure> {
        let mut join = Join::rule(
            rule,
            support,
            self.computation,
            self.limits,
            self.budget,
            &mut self.counters,
        )?;
        // The same complete-column attempt precedes frozen checker scans.
        // Admit even speculative successful values here: a covering column can
        // include rows excluded by another argument of this source pattern.
        join.select_total_constraint(rule, self.computation, self.limits, &mut self.counters)?;
        while let Some(row) = join.next_row(
            self.computation,
            self.limits,
            self.budget,
            &mut self.counters,
            rule.location,
        )? {
            if row.passes {
                // Every retained count is bounded by the charged substitution cap.
                *instances += 1;
            }
            for literal in &rule.body {
                let pattern = match literal {
                    LiteralIr::Atom(_, pattern) => pattern,
                    LiteralIr::PatternAtom(pattern) => &pattern.atom,
                    _ => continue,
                };
                if row.passes {
                    self.atom(*pattern, &row.values, rule.location)?;
                } else {
                    // Eager discarded-body validation checks these bindings but
                    // publishes no atoms. All scalar operations were checked by
                    // the shared family admission and join; no local scopes are
                    // eligible here, so atom-key validation completes that duty.
                    self.work(rule.location)?;
                    let view = row.values.view(
                        self.computation.read(),
                        self.limits,
                        &mut self.counters,
                        rule.location,
                    )?;
                    self.computation
                        .static_pattern(*pattern, self.limits, &mut self.counters, rule.location)?
                        .key(view)
                        .map_err(|error| FormulaFailure::UnsafeVariable {
                            variable: error.variable,
                            location: rule.location,
                        })?;
                }
            }
        }
        Ok(())
    }

    fn instantiate_rule(
        &mut self,
        rule: &RuleIr,
        index: usize,
        domains: Option<&crate::formula_domains::Domains<'_>>,
        support: &Support,
    ) -> Result<(), FormulaFailure> {
        if crate::formula_factor::rule(self, rule, support)? {
            return Ok(());
        }
        let guards = if let Some(domains) = domains {
            support.domain_guards(
                rule,
                domains.for_rule(index, rule)?,
                self.computation,
                self.limits,
                &mut self.counters,
            )?
        } else {
            None
        };
        let mut outer = Join::domain_rule(
            rule,
            support,
            guards.as_ref(),
            self.computation,
            self.limits,
            self.budget,
            &mut self.counters,
        )?;
        outer.select_total_constraint(rule, self.computation, self.limits, &mut self.counters)?;
        while let Some(row) = outer.next_row(
            self.computation,
            self.limits,
            self.budget,
            &mut self.counters,
            rule.location,
        )? {
            if row.passes {
                self.rule(rule, &row.values, support)?;
            } else {
                self.validate_body(&rule.body, &row.values, support, rule.location)?;
            }
        }
        Ok(())
    }

    fn empty<'a, 'terms, 'source>(
        computation: &'a mut Computation<'terms, 'source>,
        limits: &'a FormulaLimits,
        budget: &'a mut Budget,
        counters: &mut Counters,
        purpose: Purpose,
        count_plan: Option<crate::formula_count_plan::Collector>,
        location: Location,
    ) -> Result<Builder<'a, 'terms, 'source>, FormulaFailure> {
        let catalog = atoms::Catalog::new(computation, counters, limits, location)?;
        let terms = TermTable::new(computation, limits, counters, location)?;
        let aggregate_atoms =
            formula_support::SourceSelection::new(computation, limits, counters, location)?;
        let aggregate_cache = Contexts::new(computation, limits, counters, location)?;
        Ok(Builder {
            computation,
            limits,
            budget,
            catalog,
            terms,
            aggregate_atoms,
            metadata: metadata::Metadata::default(),
            nodes: Vec::new(),
            node_indices: nodes::Index::default(),
            roots: Vec::new(),
            origins: Vec::new(),
            counters: std::mem::take(counters),
            origin_count: 0,
            aggregate_cache,
            cached_elements: 0,
            cached_roots: 0,
            count_plan,
            purpose,
        })
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
    fn finish(
        mut self,
        profile: &Profile<'_>,
    ) -> Result<(Emission<formula_support::SourceSelection>, Counters), FormulaFailure> {
        use crate::GroundingPhase;

        profile.phase(GroundingPhase::Coherence, None, || self.coherence())?;
        profile.phase(GroundingPhase::SupportGuards, None, || {
            self.support_guards()
        })?;
        Ok((
            Emission {
                atoms: self.catalog.into_selection(),
                nodes: self.nodes,
                roots: self.roots,
                origins: self.origins,
                count_plan: self.count_plan,
            },
            self.counters,
        ))
    }

    // Only the completed atom catalog establishes which opposite tuples can
    // coexist. Reuse its IDs; coherence must create neither atoms nor support.
    fn coherence(&mut self) -> Result<(), FormulaFailure> {
        for index in 0..self.catalog.len() {
            let location = self.metadata.location(index);
            let atom = self.catalog.get(
                index,
                self.computation,
                &mut self.counters,
                self.limits,
                location,
            )?;
            if atom.predicate().sign() != zetesis_core::Sign::Negative {
                continue;
            }
            self.counters.work(self.limits, location)?;
            if let Some(other) = self.catalog.find(
                index,
                zetesis_core::Sign::Positive,
                self.computation,
                &mut self.counters,
                self.limits,
                location,
            )? {
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
        pattern: AtomPattern,
        assignment: &Binding,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        self.work(location)?;
        let pattern =
            self.computation
                .static_pattern(pattern, self.limits, &mut self.counters, location)?;
        let atom = self.computation.atom(
            pattern,
            assignment,
            self.limits,
            &mut self.counters,
            location,
        )?;
        self.atom_identity(&atom, location)
    }

    fn atom_ref(
        &mut self,
        atom: zetesis_core::catalog::AtomRef<'_>,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        self.work(location)?;
        let atom = self
            .computation
            .atom_ref(atom, self.limits, &mut self.counters, location)?;
        self.atom_identity(&atom, location)
    }

    fn atom_identity(
        &mut self,
        atom: &formula_support::SourceAtom,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        self.counters.record(Event::AtomLookup);
        if let Some(index) =
            self.catalog
                .position(atom, &mut self.counters, self.limits, location)?
        {
            return self.node(Node::Atom(index), location);
        }
        let bound = self.atom_bound();
        ceiling(
            bound.0,
            self.catalog.len() as u128 + 1,
            bound.1 as u128,
            location,
        )?;
        if matches!(self.purpose, Purpose::Theory) {
            self.budget
                .charge(ExpansionResource::Origins, 1, location)?;
        }
        let (index, inserted) = self.catalog.insert(
            atom,
            bound,
            self.computation,
            &mut self.counters,
            self.limits,
            location,
        )?;
        debug_assert!(inserted, "exclusive emission selection");
        self.counters.record(Event::AtomInserted);
        if matches!(self.purpose, Purpose::Theory) {
            self.metadata
                .atom(location, &mut self.counters, self.limits)?;
        }
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
        let mut context = crate::formula_source_activity::Context {
            computation: self.computation,
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
                let mut atom = self.atom(*pattern, assignment, location)?;
                if *negation != DefaultNegation::None {
                    atom = self.neg(atom, location)?;
                }
                if *negation == DefaultNegation::NotNot {
                    atom = self.neg(atom, location)?;
                }
                result = self.and(result, atom, location)?;
            } else if let LiteralIr::PatternAtom(pattern) = literal {
                let atom = self.atom(pattern.atom, assignment, location)?;
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
                let predicate = self.computation.static_predicate(
                    *predicate,
                    self.limits,
                    &mut self.counters,
                    location,
                )?;
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
                    Context::new(
                        &*self.computation,
                        self.limits,
                        &mut self.counters,
                        location,
                    ),
                )?;
                let mut result = FALSUM;
                while let Some(row) = rows.next(
                    self.computation,
                    self.limits,
                    self.budget,
                    &mut self.counters,
                    location,
                )? {
                    self.work(location)?;
                    let atom = self.atom(*atom, &row, location)?;
                    result = self.or(result, atom, location)?;
                }
                Ok(result)
            }
        }
    }

    fn project_arguments(
        &mut self,
        predicate: zetesis_core::catalog::PredicateRef<'_>,
        terms: &[Option<components::Term>],
        assignment: &Binding,
        support: &Support,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        let mut result = FALSUM;
        for atom in support.rows(predicate) {
            self.work(location)?;
            let mut matches = true;
            for (column, term) in terms.iter().enumerate() {
                self.work(location)?;
                if let Some(term) = term {
                    let value = atom.value(column).expect("checked projection arity");
                    let term = self.computation.static_term(
                        *term,
                        self.limits,
                        &mut self.counters,
                        location,
                    )?;
                    let expected = assignment.resolve(term, self.computation.read(), location)?;
                    matches &= expected
                        .equals_ref_with(value, || self.counters.work(self.limits, location))?;
                }
            }
            if matches {
                let atom = self.atom_ref(atom.atom(), location)?;
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
                    Some(head) => self.atom(*head, assignment, rule.location)?,
                    None => FALSUM,
                };
                let formula = self.node(Node::Implies(body, head), rule.location)?;
                self.root(formula, rule)?;
                if head != FALSUM {
                    self.producer(head, body, rule)?;
                }
                Ok(())
            }
            HeadIr::Disjunction(heads) => self.disjunction(heads, body, assignment, rule),
            HeadIr::ConditionalDisjunction { ordinary, elements } => {
                let mut disjunction = FALSUM;
                let mut count = 0;
                for head in ordinary {
                    let literal = self.disjunct_instance(head, assignment, VERUM, body, rule)?;
                    disjunction = self.or(disjunction, literal, rule.location)?;
                    count += 1;
                }
                for element in elements {
                    let mut local = Join::local_head(
                        &element.condition,
                        &assignment.prefix(element.outer_variables),
                        element.body_variables..element.variables,
                        support,
                        self.budget,
                        Context::new(
                            &*self.computation,
                            self.limits,
                            &mut self.counters,
                            rule.location,
                        ),
                    )?;
                    while let Some(binding) = local.next(
                        self.computation,
                        self.limits,
                        self.budget,
                        &mut self.counters,
                        rule.location,
                    )? {
                        count += 1;
                        ceiling(
                            FormulaResource::DisjunctionElements,
                            count,
                            self.limits.max_disjunction_elements as u128,
                            rule.location,
                        )?;
                        let condition = self.body(
                            &element.condition,
                            &binding.prefix(element.body_variables),
                            rule.location,
                            support,
                        )?;
                        let literal =
                            self.disjunct_instance(&element.head, &binding, condition, body, rule)?;
                        disjunction = self.or(disjunction, literal, rule.location)?;
                    }
                }
                // Exhaustion, not a stopped prefix, establishes the empty disjunction.
                let formula = self.node(Node::Implies(body, disjunction), rule.location)?;
                self.root(formula, rule)
            }
            HeadIr::Choice(group) => self.choice(rule, group, body, assignment, support),
        }
    }
    fn disjunction(
        &mut self,
        heads: &[HeadLiteral],
        body: usize,
        assignment: &Binding,
        rule: &RuleIr,
    ) -> Result<(), FormulaFailure> {
        let mut disjunction = FALSUM;
        let mut distinct = CoordinateMap::new(
            self.computation,
            self.limits,
            &mut self.counters,
            rule.location,
        )?;
        for head in heads {
            let (literal, atom) = self.head_literal(head, assignment, rule.location)?;
            if distinct
                .insert(
                    literal,
                    (),
                    Some((
                        FormulaResource::DisjunctionElements,
                        self.limits.max_disjunction_elements,
                    )),
                    Context::new(
                        &*self.computation,
                        self.limits,
                        &mut self.counters,
                        rule.location,
                    ),
                )?
                .is_none()
            {
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
    fn disjunct_instance(
        &mut self,
        head: &HeadLiteral,
        binding: &Binding,
        condition: usize,
        body: usize,
        rule: &RuleIr,
    ) -> Result<usize, FormulaFailure> {
        let (literal, atom) = self.head_literal(head, binding, rule.location)?;
        if let Some(atom) = atom {
            if head.positive_atom().is_some() {
                let permission = self.and(body, condition, rule.location)?;
                self.producer(atom, permission, rule)?;
            } else {
                self.head_origins(atom, rule)?;
            }
        }
        let implication = self.node(Node::Implies(condition, literal), rule.location)?;
        let absent = self.neg(condition, rule.location)?;
        let eligible = self.neg(absent, rule.location)?;
        self.and(implication, eligible, rule.location)
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
            self.budget,
            Context::new(
                &mut *self.computation,
                self.limits,
                &mut self.counters,
                rule.location,
            ),
        )?;
        let keys = if self.count_plan.is_some() && *measure == HeadMeasure::Count {
            keys
        } else {
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
        self.choice_permissions(eligible.iter().copied(), body, rule)?;
        let retained = retaining.then_some(eligible);
        if !guards.is_empty() {
            let mut selected = HeadContributions::new(
                kind,
                activity.len(),
                self.computation,
                self.limits,
                &mut self.counters,
                rule.location,
            )?;
            for entry in activity.iter() {
                self.work(rule.location)?;
                let (key, condition) = *entry;
                if let Some(contribution) = crate::formula_head_aggregate::contribution(
                    *measure,
                    key.first(&self.terms, self.computation.read(), rule.location)?,
                    rule.location,
                )? {
                    selected.push(
                        contribution,
                        condition,
                        &mut self.terms,
                        Context::new(
                            &*self.computation,
                            self.limits,
                            &mut self.counters,
                            rule.location,
                        ),
                    )?;
                }
            }
            drop(activity);
            self.work(rule.location)?;
            let selected = selected.finish();
            let within = self.aggregate_guards_with_capture(
                &selected,
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
            if let (Some(collector), Some(eligible), Some(keys), Some(bounds)) =
                (&mut self.count_plan, retained, keys, count_bounds)
            {
                collector.capture_group(
                    &crate::formula_count_plan::Input {
                        body,
                        eligible: eligible.slice(),
                        bijection: keys,
                        nodes: &self.nodes,
                        atom_count: self.catalog.len(),
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
                let atom = self.atom(*pattern, assignment, location)?;
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
        let mut result = HeadGroup {
            eligible: CoordinateMap::new(
                self.computation,
                self.limits,
                &mut self.counters,
                rule.location,
            )?,
            activity: CoordinateMap::new(
                self.computation,
                self.limits,
                &mut self.counters,
                rule.location,
            )?,
        };
        let elements = &group.elements;
        for element in elements {
            let mut local = Join::element(
                element,
                assignment,
                support,
                self.budget,
                Context::new(
                    &*self.computation,
                    self.limits,
                    &mut self.counters,
                    rule.location,
                ),
            )?;
            while let Some(binding) = local.next(
                self.computation,
                self.limits,
                self.budget,
                &mut self.counters,
                rule.location,
            )? {
                let condition = self.body(
                    &element.condition,
                    &element.body_binding(&binding),
                    rule.location,
                    support,
                )?;
                let (head, atom) = self.head_literal(&element.head, &binding, rule.location)?;
                self.head_permission(&mut result, &element.head, atom, condition, rule)?;
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

    /// Coalesce positive permissions; negative occurrences add only provenance.
    fn head_permission(
        &mut self,
        result: &mut HeadGroup,
        head: &HeadLiteral,
        atom: Option<usize>,
        condition: usize,
        rule: &RuleIr,
    ) -> Result<(), FormulaFailure> {
        if let Some(atom) = atom {
            if head.positive_atom().is_some() {
                let previous = result
                    .eligible
                    .find(
                        &atom,
                        self.computation,
                        self.limits,
                        &mut self.counters,
                        rule.location,
                    )?
                    .unwrap_or(FALSUM);
                let condition = self.or(previous, condition, rule.location)?;
                result.eligible.insert(
                    atom,
                    condition,
                    None,
                    Context::new(
                        &*self.computation,
                        self.limits,
                        &mut self.counters,
                        rule.location,
                    ),
                )?;
            } else {
                self.head_origins(atom, rule)?;
            }
        }
        Ok(())
    }

    fn head_activity(
        &mut self,
        group: &mut HeadGroup,
        key: HeadKey,
        selected: usize,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let previous = group
            .activity
            .find(
                &key,
                self.computation,
                self.limits,
                &mut self.counters,
                location,
            )?
            .unwrap_or(FALSUM);
        let activity = self.or(previous, selected, location)?;
        group.activity.insert(
            key,
            activity,
            Some((
                FormulaResource::AggregateElements,
                self.limits.aggregate.max_elements,
            )),
            Context::new(
                &*self.computation,
                self.limits,
                &mut self.counters,
                location,
            ),
        )?;
        Ok(())
    }

    fn head_tuple(
        &mut self,
        terms: &[components::Term],
        assignment: &Binding,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        let tuple = crate::formula_assignment::tuple(
            terms,
            assignment,
            self.computation,
            self.limits,
            &mut self.counters,
            location,
        )?;
        self.term(&tuple, location)
    }

    fn term(&mut self, key: &TermKey, location: Location) -> Result<usize, FormulaFailure> {
        self.terms.insert(
            key,
            None,
            self.computation,
            self.limits,
            &mut self.counters,
            location,
        )
    }

    fn choice_permissions(
        &mut self,
        eligible: impl Iterator<Item = (usize, usize)>,
        body: usize,
        rule: &RuleIr,
    ) -> Result<(), FormulaFailure> {
        for (head, condition) in eligible {
            self.work(rule.location)?;
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
struct HeadGroup {
    eligible: CoordinateMap<usize, usize>,
    activity: CoordinateMap<HeadKey, usize>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum HeadKey {
    Tuple(usize),
    Atom(DefaultNegation, usize),
    BooleanOccurrence(Location),
}

impl HeadKey {
    fn first<'a>(
        &self,
        terms: &TermTable,
        read: zetesis_core::catalog::CatalogRead<'a>,
        location: Location,
    ) -> Result<Option<TermRef<'a>>, FormulaFailure> {
        match self {
            Self::Tuple(tuple) => Ok(terms.value(*tuple, read, location)?.child(0)),
            Self::Atom(..) | Self::BooleanOccurrence(_) => Ok(None),
        }
    }
}

#[derive(Clone, Copy)]
struct ExtremumElement {
    value: usize,
    condition: usize,
}

/// One storage family for coalesced tuple contributions. Each entry retains
/// every selected eligible witness; no candidate truth is assumed here.
enum HeadContributions {
    Numeric(Buffer<AggregateElement>),
    Extrema(Buffer<ExtremumElement>),
}

impl HeadContributions {
    fn new(
        kind: Option<AggregateExtremum>,
        capacity: usize,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        if kind.is_some() {
            let mut elements = Buffer::new(computation, limits, counters, location)?;
            elements.reserve(capacity, computation, limits, counters, location)?;
            Ok(Self::Extrema(elements))
        } else {
            let mut elements = Buffer::new(computation, limits, counters, location)?;
            elements.reserve(capacity, computation, limits, counters, location)?;
            Ok(Self::Numeric(elements))
        }
    }

    fn push(
        &mut self,
        contribution: crate::formula_head_aggregate::Contribution<'_>,
        condition: usize,
        terms: &mut TermTable,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        use crate::formula_head_aggregate::Contribution;

        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        match (self, contribution) {
            (Self::Numeric(elements), Contribution::Numeric(weight)) => {
                elements.push(
                    AggregateElement { weight, condition },
                    computation,
                    limits,
                    counters,
                    location,
                )?;
            }
            (Self::Extrema(elements), Contribution::Extremum(value)) => {
                let key = computation.read().term_key(value).map_err(|error| {
                    crate::formula_binding::assignment(
                        zetesis_core::catalog::AssignmentError::Read(error),
                        location,
                    )
                })?;
                let value = terms.insert(&key, None, computation, limits, counters, location)?;
                elements.push(
                    ExtremumElement { value, condition },
                    computation,
                    limits,
                    counters,
                    location,
                )?;
            }
            _ => unreachable!("group measure determines its contribution family"),
        }
        Ok(())
    }

    fn finish(self) -> GroundAggregate {
        match self {
            Self::Numeric(elements) => GroundAggregate::Numeric(Arc::new(elements)),
            Self::Extrema(elements) => GroundAggregate::Extrema(Arc::new(elements)),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum GroundKey {
    Tuple(usize),
    Atom(usize),
}

#[derive(Clone, Copy)]
enum Measure {
    Numeric(i32),
    Extremum(usize),
}

impl Builder<'_, '_, '_> {
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
        let elements = self.aggregate_elements(aggregate, assignment, support, location)?;
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
    fn cached_aggregate(
        &mut self,
        aggregate: &AggregateIr,
        key: &Buffer<Option<usize>>,
        assignment: &Binding,
        support: &Support,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        if let Some(slot) = self.aggregate_cache.find(
            aggregate.id,
            key,
            self.computation,
            self.limits,
            &mut self.counters,
            location,
        )? {
            return Ok(slot);
        }
        ceiling(
            FormulaResource::AggregateCacheRows,
            self.aggregate_cache.len() as u128 + 1,
            self.limits.max_aggregate_cache_rows as u128,
            location,
        )?;
        let elements = self.aggregate_elements(aggregate, assignment, support, location)?;
        ceiling(
            FormulaResource::AggregateCacheElements,
            self.cached_elements as u128 + elements.len() as u128,
            self.limits.max_aggregate_cache_elements as u128,
            location,
        )?;
        self.work(location)?;
        let count = elements.len();
        let next = self.aggregate_cache.len();
        let slot = self.aggregate_cache.insert(
            aggregate.id,
            key,
            CachedAggregate {
                elements,
                roots: None,
            },
            Context::new(
                &*self.computation,
                self.limits,
                &mut self.counters,
                location,
            ),
        )?;
        if slot == next {
            self.cached_elements += count;
        }
        Ok(slot)
    }
    fn assignment_aggregate(
        &mut self,
        aggregate: &AggregateIr,
        target: usize,
        assignment: &Binding,
        support: &Support,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        let key = self.cache_key(target, assignment, location)?;
        let slot = self.cached_aggregate(aggregate, &key, assignment, support, location)?;
        self.work(location)?;
        let roots = self
            .aggregate_cache
            .get(slot)
            .and_then(|cached| cached.roots.as_ref().map(Arc::clone));
        let roots = if let Some(roots) = roots {
            roots
        } else {
            self.work(location)?;
            let elements = self
                .aggregate_cache
                .get(slot)
                .expect("cache lookup returned a published row")
                .elements
                .clone();
            let roots = self.assignment_family(aggregate.function, &elements, location)?;
            self.work(location)?;
            self.aggregate_cache
                .get_mut(slot)
                .expect("eligibility cache was populated before family construction")
                .roots = Some(Arc::clone(&roots));
            self.cached_roots += roots.len();
            roots
        };
        let target_value = assignment.key(target, location)?;
        let key = self
            .terms
            .find(
                &target_value,
                self.computation,
                self.limits,
                &mut self.counters,
                location,
            )?
            .expect("the complete final-U tuple set covers every assignment proposal");
        Ok(roots
            .find(
                &key,
                self.computation,
                self.limits,
                &mut self.counters,
                location,
            )?
            .expect("the complete family has a root for every assignment proposal"))
    }
    fn assignment_family(
        &mut self,
        function: AggregateFunction,
        elements: &GroundAggregate,
        location: Location,
    ) -> Result<Arc<CoordinateMap<usize, usize>>, FormulaFailure> {
        let values = self.aggregate_candidates(function, elements, location)?;
        ceiling(
            FormulaResource::AggregateCacheRoots,
            self.cached_roots as u128 + values.len() as u128,
            self.limits.max_aggregate_cache_roots as u128,
            location,
        )?;
        let mut roots =
            CoordinateMap::new(self.computation, self.limits, &mut self.counters, location)?;
        roots.reserve(
            values.len(),
            self.computation,
            self.limits,
            &mut self.counters,
            location,
        )?;
        if let GroundAggregate::Extrema(elements) = elements {
            let kind = extremum(function).expect("value aggregate is min/max");
            for slot in 0..values.len() {
                self.work(location)?;
                let key = values.key(slot, location)?;
                let value = self.term(&key, location)?;
                let root = self.extremum_root(
                    elements.slice(),
                    kind,
                    AggregateComparison::Eq,
                    &key,
                    location,
                )?;
                roots.insert(
                    value,
                    root,
                    None,
                    Context::new(
                        &*self.computation,
                        self.limits,
                        &mut self.counters,
                        location,
                    ),
                )?;
            }
        } else {
            let GroundAggregate::Numeric(elements) = elements else {
                unreachable!("handled extrema");
            };
            let guards = self.assignment_guards(&values, location)?;
            let first = self.nodes.len();
            let family = self.append_guard_family(
                elements.slice(),
                guards.slice(),
                self.limits.max_assignment_values,
                location,
            )?;
            let canonical = self.intern_appended(first, location)?;
            for (slot, root) in family.build.roots().iter().enumerate() {
                self.work(location)?;
                let key = values.key(slot, location)?;
                let value = self.term(&key, location)?;
                roots.insert(
                    value,
                    remap(*root, first, &canonical),
                    None,
                    Context::new(
                        &*self.computation,
                        self.limits,
                        &mut self.counters,
                        location,
                    ),
                )?;
            }
        }
        self.work(location)?;
        Ok(Arc::new(roots))
    }
    /// Pair each numeric proposal with its equality guard in proposal order.
    fn assignment_guards(
        &mut self,
        values: &Binding,
        location: Location,
    ) -> Result<Buffer<NumericGuard>, FormulaFailure> {
        let mut guards = Buffer::new(self.computation, self.limits, &mut self.counters, location)?;
        guards.reserve(
            values.len(),
            self.computation,
            self.limits,
            &mut self.counters,
            location,
        )?;
        for slot in 0..values.len() {
            self.work(location)?;
            let ValueNodeRef::Number(value) = values
                .read(slot, self.computation.read(), location)?
                .descriptor()
            else {
                unreachable!("count/sum proposals are numeric");
            };
            guards.push(
                NumericGuard {
                    comparison: AggregateComparison::Eq,
                    bound: i64::from(value),
                },
                self.computation,
                self.limits,
                &mut self.counters,
                location,
            )?;
        }
        Ok(guards)
    }

    /// Enumerate the complete assignment proposal family before building roots.
    fn aggregate_candidates(
        &mut self,
        function: AggregateFunction,
        elements: &GroundAggregate,
        location: Location,
    ) -> Result<Binding<'static>, FormulaFailure> {
        match elements {
            GroundAggregate::Numeric(elements) => crate::formula_assignment::candidates(
                function,
                elements.iter().map(|element| element.weight),
                self.computation,
                self.limits,
                &mut self.counters,
                location,
            ),
            GroundAggregate::Extrema(elements) => {
                let mut firsts =
                    Binding::new(self.computation, self.limits, &mut self.counters, location)?;
                firsts.extend_scope(
                    elements.len(),
                    self.computation,
                    self.limits,
                    &mut self.counters,
                    location,
                )?;
                for (slot, element) in elements.iter().enumerate() {
                    self.work(location)?;
                    let key = self.terms.key(element.value, location)?;
                    firsts.set(slot, &key, self.limits, &mut self.counters, location)?;
                }
                crate::formula_assignment::extrema_candidates(
                    function,
                    firsts.slots(),
                    self.computation,
                    self.limits,
                    &mut self.counters,
                    location,
                )
            }
        }
    }
    fn cache_key(
        &mut self,
        target: usize,
        assignment: &Binding,
        location: Location,
    ) -> Result<Buffer<Option<usize>>, FormulaFailure> {
        let count = assignment.len() - usize::from(target < assignment.len());
        let mut outer = Buffer::new(self.computation, self.limits, &mut self.counters, location)?;
        outer.resize(
            count,
            None,
            self.computation,
            self.limits,
            &mut self.counters,
            location,
        )?;
        let mut at = 0;
        for slot in 0..assignment.len() {
            self.work(location)?;
            if slot != target {
                let key = assignment
                    .slots()
                    .key(slot)
                    .map_err(|error| crate::formula_binding::assignment(error, location))?;
                outer.slice_mut()[at] = key
                    .as_ref()
                    .map(|key| self.term(key, location))
                    .transpose()?;
                at += 1;
            }
        }
        Ok(outer)
    }
    fn aggregate_elements(
        &mut self,
        aggregate: &AggregateIr,
        assignment: &Binding,
        support: &Support,
        location: Location,
    ) -> Result<GroundAggregate, FormulaFailure> {
        let is_extremum = extremum(aggregate.function).is_some();
        let mut grouped = CoordinateMap::<GroundKey, (Measure, usize)>::new(
            self.computation,
            self.limits,
            &mut self.counters,
            location,
        )?;
        for element in &aggregate.elements {
            let mut local = Join::new(
                &element.condition,
                assignment,
                element.variables,
                support,
                self.budget,
                Context::new(
                    &*self.computation,
                    self.limits,
                    &mut self.counters,
                    location,
                ),
            )?;
            while let Some(binding) = local.next(
                self.computation,
                self.limits,
                self.budget,
                &mut self.counters,
                location,
            )? {
                let key = self.aggregate_key(&element.key, &binding, location)?;
                let weight = match &key {
                    GroundKey::Tuple(tuple) if is_extremum => {
                        let read = self.computation.read();
                        let value = self
                            .terms
                            .value(*tuple, read, location)?
                            .child(0)
                            .expect("admitted nonempty extremum tuple");
                        crate::formula_assignment::extremum_value(value, location)?;
                        let key = read.term_key(value).map_err(|error| {
                            crate::formula_binding::assignment(
                                zetesis_core::catalog::AssignmentError::Read(error),
                                location,
                            )
                        })?;
                        Measure::Extremum(self.term(&key, location)?)
                    }
                    GroundKey::Tuple(tuple) => {
                        let value = self
                            .terms
                            .value(*tuple, self.computation.read(), location)?
                            .child(0);
                        let Some(weight) = crate::formula_assignment::contribution(
                            aggregate.function,
                            value,
                            location,
                        )?
                        else {
                            continue;
                        };
                        Measure::Numeric(weight)
                    }
                    GroundKey::Atom(_) => Measure::Numeric(1),
                };
                let condition = self.body(&element.condition, &binding, location, support)?;
                let previous = grouped
                    .find(
                        &key,
                        self.computation,
                        self.limits,
                        &mut self.counters,
                        location,
                    )?
                    .map_or(FALSUM, |(_, condition)| condition);
                let condition = self.or(previous, condition, location)?;
                grouped.insert(
                    key,
                    (weight, condition),
                    Some((
                        FormulaResource::AggregateElements,
                        self.limits.aggregate.max_elements,
                    )),
                    Context::new(
                        &*self.computation,
                        self.limits,
                        &mut self.counters,
                        location,
                    ),
                )?;
            }
        }
        self.coalesced_aggregate(grouped, is_extremum, location)
    }

    /// Publish the measured contribution family after full tuple coalescing.
    fn coalesced_aggregate(
        &mut self,
        grouped: CoordinateMap<GroundKey, (Measure, usize)>,
        is_extremum: bool,
        location: Location,
    ) -> Result<GroundAggregate, FormulaFailure> {
        let grouped = aggregate_order::ordered(
            grouped,
            &self.terms,
            &self.aggregate_atoms,
            Context::new(
                &*self.computation,
                self.limits,
                &mut self.counters,
                location,
            ),
        )?;
        if is_extremum {
            let mut elements =
                Buffer::new(self.computation, self.limits, &mut self.counters, location)?;
            elements.reserve(
                grouped.len(),
                self.computation,
                self.limits,
                &mut self.counters,
                location,
            )?;
            for entry in grouped.iter() {
                self.work(location)?;
                let (_, (value, condition)) = *entry;
                let Measure::Extremum(value) = value else {
                    unreachable!("extremum contribution");
                };
                elements.push(
                    ExtremumElement { value, condition },
                    self.computation,
                    self.limits,
                    &mut self.counters,
                    location,
                )?;
            }
            self.work(location)?;
            return Ok(GroundAggregate::Extrema(Arc::new(elements)));
        }
        let mut elements =
            Buffer::new(self.computation, self.limits, &mut self.counters, location)?;
        elements.reserve(
            grouped.len(),
            self.computation,
            self.limits,
            &mut self.counters,
            location,
        )?;
        for entry in grouped.iter() {
            self.work(location)?;
            let (_, (value, condition)) = *entry;
            let Measure::Numeric(weight) = value else {
                unreachable!("numeric contribution");
            };
            elements.push(
                AggregateElement { weight, condition },
                self.computation,
                self.limits,
                &mut self.counters,
                location,
            )?;
        }
        self.work(location)?;
        Ok(GroundAggregate::Numeric(Arc::new(elements)))
    }
    fn aggregate_key(
        &mut self,
        key: &AggregateKey,
        assignment: &Binding,
        location: Location,
    ) -> Result<GroundKey, FormulaFailure> {
        match key {
            AggregateKey::Tuple(terms) => self
                .head_tuple(terms, assignment, location)
                .map(GroundKey::Tuple),
            AggregateKey::Atom(pattern) => {
                let pattern = self.computation.static_pattern(
                    *pattern,
                    self.limits,
                    &mut self.counters,
                    location,
                )?;
                let atom = self.computation.atom(
                    pattern,
                    assignment,
                    self.limits,
                    &mut self.counters,
                    location,
                )?;
                let (position, _) = self.aggregate_atoms.insert(
                    &atom,
                    (FormulaResource::Atoms, self.limits.theory.max_atoms),
                    self.computation,
                    self.limits,
                    &mut self.counters,
                    location,
                )?;
                Ok(GroundKey::Atom(position))
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
            .min(self.limits.max_work - self.counters.accounting.work);
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
        if guards.len() > 1 && kind.is_none() {
            let GroundAggregate::Numeric(elements) = elements else {
                unreachable!("numeric contribution")
            };
            if self.nonnegative_elements(elements.slice(), location)? {
                return self.numeric_guard_family(
                    elements.slice(),
                    guards,
                    assignment,
                    location,
                    capture,
                );
            }
        }
        let mut result = VERUM;
        for guard in guards {
            let bound = formula_support::expression(
                &guard.bound,
                assignment,
                self.computation,
                self.limits,
                &mut self.counters,
                location,
            )?;
            if let Some(kind) = kind {
                let GroundAggregate::Extrema(elements) = elements else {
                    unreachable!("extrema contribution")
                };
                let root = self.extremum_root(
                    elements.slice(),
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
            let bound = match numeric_comparison(
                guard.relation,
                self.computation.read().term(&bound).map_err(|error| {
                    crate::formula_binding::assignment(
                        zetesis_core::catalog::AssignmentError::Read(error),
                        location,
                    )
                })?,
                self.limits,
                &mut self.counters,
                location,
            )? {
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
                elements.slice(),
                aggregate_comparison(guard.relation),
                i64::from(bound),
                limits,
                &zetesis_cpu::Cancellation::default(),
            )
            .map_err(|error| FormulaFailure::Aggregate { error, location })?;
            self.counters.accounting.work += build.statistics().work;
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
    bound: TermRef<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<NumericComparison, FormulaFailure> {
    if let ValueNodeRef::Number(bound) = bound.descriptor() {
        return Ok(NumericComparison::Threshold(bound));
    }
    // Every integer has the same order against a nonnumeric logical value.
    // This represents the numeric class, not an extremal or missing value.
    let zero = zetesis_core::Value::Number(0);
    formula_support::compare((&zero).into(), relation, bound, limits, counters, location)
        .map(NumericComparison::Constant)
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
impl Builder<'_, '_, '_> {
    fn extremum_root(
        &mut self,
        elements: &[ExtremumElement],
        kind: AggregateExtremum,
        comparison: AggregateComparison,
        bound: &TermKey,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        let read = self.computation.read();
        let bound = read.term(bound).map_err(|error| {
            crate::formula_binding::assignment(
                zetesis_core::catalog::AssignmentError::Read(error),
                location,
            )
        })?;
        crate::formula_assignment::extremum_value(bound, location)?;
        let first = self.nodes.len();
        let limits = self.aggregate_limits();
        let terms = &self.terms;
        let values = elements.iter().map(|element| ValueExtremumElement {
            value: terms
                .value(element.value, read, location)
                .expect("retained extrema name admitted source terms"),
            condition: element.condition,
        });
        let build = append_value_extremum_refs(
            &mut self.nodes,
            values,
            kind,
            comparison,
            bound,
            limits,
            &zetesis_cpu::Cancellation::default(),
        )
        .map_err(|error| FormulaFailure::Aggregate { error, location })?;
        self.counters.accounting.work += build.statistics().work;
        let canonical = self.intern_appended(first, location)?;
        Ok(remap(build.root(), first, &canonical))
    }
}
