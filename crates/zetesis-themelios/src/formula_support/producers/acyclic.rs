//! One authenticated dependency order for the complete signed normal source.
//!
//! Unlike positive support scheduling, every body predicate must have a
//! strictly smaller component rank, except inside a certain
//! component. No producer or negative edge is omitted.
//!
//! A component is *certain* when each default-negated atom its rules read
//! lies in a strictly lower certain component, and each atom they read
//! positively lies in it or in a lower certain component. Its predicates are
//! then certain in the sense of support completion (`super::certain`): their
//! rules are definite once the decided negations over completed lower
//! components are read, so completed possible support restricted to them is
//! their least model, which holds in every answer set. Such a component may be
//! recursive. Recursion through a component that is not certain declines the
//! class.
//!
//! A recursive certain component, or one whose rules negate nothing, is
//! *saturated*: evaluation publishes its completed support directly. A
//! non-recursive certain component that negates is still folded once against
//! the completed lower activity, as an acyclic component is.

use super::{Memory, Node, find, graph_nodes, invalid};
use crate::formula_facts::Item;
use crate::formula_ir::{AggregateIr, HeadIr, LiteralIr, Prepared};
use crate::formula_support::components::Pattern;
use crate::formula_support::{Counters, SourceScope, StorageLease, SupportCatalog};
use crate::{AnalysisBasis, FormulaFailure, FormulaLimits, ProgramSite};
use themelios_program::program::DefaultNegation;
use zetesis_core::TemplateComponentsRef;
use zetesis_core::{catalog::PredicateRef, relation::Failure};

pub(crate) struct AcyclicPlan {
    scope: SourceScope,
    // Component rank followed by item position: a rule's, or a fact block's
    // where its fact rules stood. Constraints sort last.
    items: Vec<(usize, usize)>,
    // By component rank: how the component is closed.
    closed: Vec<Closure>,
    _lease: StorageLease,
}

impl AcyclicPlan {
    pub(crate) fn prepare(
        source: &Prepared,
        catalog: &SupportCatalog,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<Self>, FormulaFailure> {
        if !eligible(source, limits, counters, location)? {
            return Ok(None);
        }
        let external = catalog.bytes(location)? as u128 + counters.workspace_bytes() as u128;
        let mut memory = Memory::new(size_of::<Self>(), external, limits, counters, location);
        memory.add(0)?;
        let nodes = graph_nodes(source, &mut memory, limits, counters, location)?;
        let ranks = Ranks {
            nodes: &nodes,
            components: catalog.component_view(limits, counters, location)?,
        };
        let mut components_count = 0_usize;
        for node in &nodes {
            counters.work(limits, location)?;
            if let Some(component) = node.component {
                components_count = components_count.max(component + 1);
            }
        }
        // Per component: no rule with a default-negated literal (refined to
        // definite closure below), then a rule reading its own component.
        let mut closed = Vec::new();
        memory.reserve(&mut closed, components_count)?;
        closed.resize(components_count, Closure::Saturated);
        let mut recursive = Vec::new();
        memory.reserve(&mut recursive, components_count)?;
        recursive.resize(components_count, false);
        let mut items = Vec::new();
        memory.reserve(&mut items, source.items().len())?;
        for (position, item) in source.items().iter().enumerate() {
            let rule = match item {
                Item::Rule(_, rule) => rule,
                Item::Facts(block) => {
                    // A block's rows share its predicate, and so its rank;
                    // a fact reads no predicate.
                    let location = block.location();
                    counters.work(limits, location)?;
                    let predicate =
                        block
                            .predicate()
                            .get(ranks.view(location)?, limits, counters, location)?;
                    items.push((ranks.of(predicate, limits, counters, location)?, position));
                    continue;
                }
            };
            counters.work(limits, rule.location)?;
            let HeadIr::Normal(head) = &rule.head else {
                unreachable!("class checked")
            };
            let rank = match head {
                Some(head) => ranks.atom(*head, limits, counters, rule.location)?,
                None => usize::MAX,
            };
            for literal in &rule.body {
                counters.work(limits, rule.location)?;
                // An assignment is evaluated from completed support only.
                if matches!(literal, LiteralIr::Aggregate(_))
                    && let Some(flag) = recursive.get_mut(rank)
                {
                    *flag = true;
                }
                let LiteralIr::Atom(negation, atom) = literal else {
                    continue;
                };
                if *negation != DefaultNegation::None
                    && let Some(flag) = closed.get_mut(rank)
                {
                    *flag = Closure::Certain;
                }
                let dependency = ranks.atom(*atom, limits, counters, rule.location)?;
                if dependency > rank {
                    return Err(invalid(Failure::Owner, rule.location));
                }
                if dependency == rank {
                    recursive[rank] = true;
                }
            }
            items.push((rank, position));
        }
        // Sort physical source positions with the shared bounded operation;
        // semantic signatures remain in the original analysis owner.
        crate::formula_support::sort::by(
            &mut items,
            crate::formula_support::GroundingWork::new(limits, counters, location),
            |left, right, _| Ok(left.cmp(right)),
        )?;
        close(&mut closed, &items, source, &ranks, limits, counters)?;
        // Recursion through a component that is not certain declines the
        // class; a recursive certain component is saturated.
        for (&recursive, closed) in recursive.iter().zip(&mut closed) {
            counters.work(limits, location)?;
            if recursive {
                if *closed == Closure::Open {
                    return Ok(None);
                }
                *closed = Closure::Saturated;
            }
        }
        memory.release(size_of::<bool>() * recursive.capacity());
        drop(recursive);
        memory.release(size_of::<Vec<Node<'_>>>() + nodes.capacity() * size_of::<Node<'_>>());
        drop(nodes);
        let mut lease = counters.lease();
        lease.observe(memory.bytes, location)?;
        Ok(Some(Self {
            scope: catalog.source_scope().clone(),
            items,
            closed,
            _lease: lease,
        }))
    }

    pub(crate) fn authenticates(&self, catalog: &SupportCatalog) -> bool {
        self.scope.same(catalog.source_scope())
    }

    /// The plan's (component rank, item position) entries, in scan order.
    pub(crate) fn items(&self) -> &[(usize, usize)] {
        &self.items
    }

    /// Whether the component of `rank` is saturated: certain, and recursive
    /// or free of default negation. Constraints have no component and are
    /// never saturated.
    pub(crate) fn closed(&self, rank: usize) -> bool {
        self.closed.get(rank) == Some(&Closure::Saturated)
    }
}

/// Component ranks of the source's predicates.
struct Ranks<'plan, 'source> {
    nodes: &'plan [Node<'source>],
    components: Option<TemplateComponentsRef<'plan>>,
}

impl<'plan> Ranks<'plan, '_> {
    fn view(&self, location: ProgramSite) -> Result<TemplateComponentsRef<'plan>, FormulaFailure> {
        self.components
            .ok_or_else(|| crate::formula_support::components::missing(location))
    }

    /// The component rank of `predicate`'s node.
    fn of(
        &self,
        predicate: PredicateRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        let node = find(
            self.nodes.len(),
            |i| self.nodes[i].signature,
            predicate,
            limits,
            counters,
            location,
        )?;
        self.nodes[node]
            .component
            .ok_or_else(|| invalid(Failure::Owner, location))
    }

    /// The component rank of an atom's predicate.
    fn atom(
        &self,
        atom: Pattern,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        let predicate = atom
            .get(self.view(location)?, limits, counters, location)?
            .predicate();
        self.of(predicate, limits, counters, location)
    }
}

/// How a component is closed under what it reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Closure {
    /// Not certain.
    Open,
    /// Certain, with a default negation over a lower certain component.
    Certain,
    /// Certain, and evaluated from completed support.
    Saturated,
}

/// Fold certainty over the rank-sorted items: a component stays certain when
/// each atom its rules negate lies in a strictly lower certain component and
/// each lower component they read positively is certain. Lower ranks are final
/// before a rank reads them.
fn close(
    closed: &mut [Closure],
    items: &[(usize, usize)],
    source: &Prepared,
    ranks: &Ranks<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<(), FormulaFailure> {
    for &(rank, position) in items {
        let Item::Rule(_, rule) = source.items().get(position) else {
            continue;
        };
        counters.work(limits, rule.location)?;
        if closed
            .get(rank)
            .is_none_or(|closure| *closure == Closure::Open)
        {
            continue;
        }
        for literal in &rule.body {
            counters.work(limits, rule.location)?;
            let certain = match literal {
                LiteralIr::Atom(negation, atom) => {
                    let dependency = ranks.atom(*atom, limits, counters, rule.location)?;
                    let lower = dependency < rank && closed[dependency] != Closure::Open;
                    (*negation == DefaultNegation::None && dependency == rank) || lower
                }
                LiteralIr::Aggregate(aggregate) => elements_certain(
                    aggregate,
                    rank,
                    closed,
                    ranks,
                    limits,
                    counters,
                    rule.location,
                )?,
                _ => true,
            };
            if !certain {
                closed[rank] = Closure::Open;
                break;
            }
        }
    }
    Ok(())
}

/// Whether every element of an assignment reads only positive atoms of
/// strictly lower certain components and literals its local join decides,
/// so that each of its tuples is required over completed support.
fn elements_certain(
    aggregate: &AggregateIr,
    rank: usize,
    closed: &[Closure],
    ranks: &Ranks<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    for element in &aggregate.elements {
        for literal in &element.condition {
            counters.work(limits, location)?;
            match literal {
                LiteralIr::Atom(DefaultNegation::None, atom) => {
                    let dependency = ranks.atom(*atom, limits, counters, location)?;
                    if dependency >= rank || closed[dependency] == Closure::Open {
                        return Ok(false);
                    }
                }
                LiteralIr::Compare(..) => {}
                _ => return Ok(false),
            }
        }
    }
    Ok(true)
}

fn eligible(
    source: &Prepared,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    counters.work(limits, location)?;
    if source.analysis_basis != AnalysisBasis::NormalizedProgram
        || !source.objectives.is_empty()
        || !source.objective_declarations.is_empty()
        || source.project_selection.is_explicit()
        || !source.projection.is_empty()
    {
        return Ok(false);
    }
    // Recursion is admitted only through certain components, which the plan
    // establishes; negative and aggregate cycles decline early.
    for component in source.analysis.dependencies().components() {
        counters.work(limits, location)?;
        if component.has_negative_cycle() || component.has_aggregate_cycle() {
            return Ok(false);
        }
    }
    // A fact block is a normal producer with an empty body: always in class.
    for rule in source.rules_without_facts() {
        counters.work(limits, rule.location)?;
        // An assignment aggregate binds its target outside the body's own
        // variables; any other binding plan keeps the ordinary door.
        let assigns = rule.body.iter().any(|literal| {
            matches!(literal, LiteralIr::Aggregate(aggregate)
                if aggregate.binding.is_some() && aggregate.negation == DefaultNegation::None)
        });
        if !matches!(rule.head, HeadIr::Normal(_))
            || (!assigns && (rule.bindings.is_some() || rule.body_variables != rule.variables))
        {
            return Ok(false);
        }
        for literal in &rule.body {
            counters.work(limits, rule.location)?;
            let admitted = match literal {
                LiteralIr::Atom(DefaultNegation::None | DefaultNegation::Not, _)
                | LiteralIr::Compare(..) => true,
                LiteralIr::Aggregate(aggregate) => {
                    aggregate.binding.is_some() && aggregate.negation == DefaultNegation::None
                }
                _ => false,
            };
            if !admitted {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
