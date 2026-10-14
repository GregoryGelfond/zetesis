//! One authenticated dependency order for the complete signed normal source.
//!
//! Unlike positive support scheduling, every body predicate must have a
//! strictly smaller component rank. No producer or negative edge is omitted.

use super::{Memory, Node, find, graph_nodes, invalid};
use crate::formula_facts::Item;
use crate::formula_ir::{HeadIr, LiteralIr, Prepared};
use crate::formula_support::{Counters, SourceScope, StorageLease, SupportCatalog};
use crate::{AnalysisBasis, FormulaFailure, FormulaLimits, ProgramSite};
use themelios_program::program::DefaultNegation;
use zetesis_core::relation::Failure;

pub(crate) struct AcyclicPlan {
    scope: SourceScope,
    // Component rank followed by item position: a rule's, or a fact block's
    // where its fact rules stood. Constraints sort last.
    items: Vec<(usize, usize)>,
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
        let components = catalog.component_view(limits, counters, location)?;
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
                    let view = components
                        .ok_or_else(|| crate::formula_support::components::missing(location))?;
                    let node = find(
                        nodes.len(),
                        |i| nodes[i].signature,
                        block.predicate().get(view, limits, counters, location)?,
                        limits,
                        counters,
                        location,
                    )?;
                    let rank = nodes[node]
                        .component
                        .ok_or_else(|| invalid(Failure::Owner, location))?;
                    items.push((rank, position));
                    continue;
                }
            };
            counters.work(limits, rule.location)?;
            let HeadIr::Normal(head) = &rule.head else {
                unreachable!("class checked")
            };
            let rank = if let Some(head) = head {
                let view = components
                    .ok_or_else(|| crate::formula_support::components::missing(rule.location))?;
                let node = find(
                    nodes.len(),
                    |i| nodes[i].signature,
                    head.get(view, limits, counters, rule.location)?.predicate(),
                    limits,
                    counters,
                    rule.location,
                )?;
                nodes[node]
                    .component
                    .ok_or_else(|| invalid(Failure::Owner, rule.location))?
            } else {
                usize::MAX
            };
            for literal in &rule.body {
                counters.work(limits, rule.location)?;
                let LiteralIr::Atom(_, atom) = literal else {
                    continue;
                };
                let view = components
                    .ok_or_else(|| crate::formula_support::components::missing(rule.location))?;
                let node = find(
                    nodes.len(),
                    |i| nodes[i].signature,
                    atom.get(view, limits, counters, rule.location)?.predicate(),
                    limits,
                    counters,
                    rule.location,
                )?;
                let dependency = nodes[node]
                    .component
                    .ok_or_else(|| invalid(Failure::Owner, rule.location))?;
                if dependency >= rank {
                    return Err(invalid(Failure::Owner, rule.location));
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
        memory.release(size_of::<Vec<Node<'_>>>() + nodes.capacity() * size_of::<Node<'_>>());
        drop(nodes);
        let mut lease = counters.lease();
        lease.observe(memory.bytes, location)?;
        Ok(Some(Self {
            scope: catalog.source_scope().clone(),
            items,
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
    for component in source.analysis.dependencies().components() {
        counters.work(limits, location)?;
        if component.is_recursive() {
            return Ok(false);
        }
    }
    // A fact block is a normal producer with an empty body: always in class.
    for rule in source.rules_without_facts() {
        counters.work(limits, rule.location)?;
        if !matches!(rule.head, HeadIr::Normal(_))
            || rule.bindings.is_some()
            || rule.body_variables != rule.variables
        {
            return Ok(false);
        }
        for literal in &rule.body {
            counters.work(limits, rule.location)?;
            if !matches!(
                literal,
                LiteralIr::Atom(DefaultNegation::None | DefaultNegation::Not, _)
                    | LiteralIr::Compare(..)
            ) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
