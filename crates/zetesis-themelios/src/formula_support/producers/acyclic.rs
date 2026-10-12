//! One authenticated dependency order for the complete signed normal source.
//!
//! Unlike positive support scheduling, every body predicate must have a
//! strictly smaller component rank. No producer or negative edge is omitted.

use super::{Memory, Node, find, graph_nodes, invalid};
use crate::formula_ir::{HeadIr, LiteralIr, Prepared};
use crate::formula_support::{Counters, SourceScope, StorageLease, SupportCatalog};
use crate::{AnalysisBasis, FormulaFailure, FormulaLimits, ProgramSite};
use themelios_program::program::DefaultNegation;
use zetesis_core::relation::Failure;

pub(crate) struct AcyclicPlan {
    scope: SourceScope,
    // Component rank followed by original rule position. Constraints sort last.
    rules: Vec<(usize, usize)>,
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
        let mut rules = Vec::new();
        memory.reserve(&mut rules, source.rules.len())?;
        for (index, rule) in source.rules.iter().enumerate() {
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
            rules.push((rank, index));
        }
        // Sort physical source positions with the shared bounded operation;
        // semantic signatures remain in the original analysis owner.
        crate::formula_support::sort::by(
            &mut rules,
            crate::formula_support::GroundingWork::new(limits, counters, location),
            |left, right, _| Ok(left.cmp(right)),
        )?;
        memory.release(size_of::<Vec<Node<'_>>>() + nodes.capacity() * size_of::<Node<'_>>());
        drop(nodes);
        let mut lease = counters.lease();
        lease.observe(memory.bytes, location)?;
        Ok(Some(Self {
            scope: catalog.source_scope().clone(),
            rules,
            _lease: lease,
        }))
    }

    pub(crate) fn authenticates(&self, catalog: &SupportCatalog) -> bool {
        self.scope.same(catalog.source_scope())
    }

    pub(crate) fn rules(&self) -> &[(usize, usize)] {
        &self.rules
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
    for rule in &source.rules {
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
