use std::collections::BTreeMap;

use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison, AggregateElement, AggregateFamilyBuild,
    AggregateFamilyLimits, AggregateGuard, FormulaNodes, NodeView, Theory, append_aggregate_family,
};
use zetesis_objective::Score;

use super::{
    ObjectiveBound, ObjectiveBoundError, ObjectiveBoundErrorKind as Kind, ObjectiveBoundLimits,
    ObjectiveBoundStatistics, ObjectivePlan, ObjectivePlanLimits, Work,
};

pub(super) mod normalize;

pub(super) fn compile(
    plan: &ObjectivePlan,
    incumbent: &Score,
    limits: ObjectiveBoundLimits,
    cancellation: &Cancellation,
) -> Result<ObjectiveBound, ObjectiveBoundError> {
    let mut work = Work {
        cancellation,
        limits: ObjectivePlanLimits {
            max_nodes: limits.aggregate.max_nodes,
            max_operands: limits.aggregate.max_operands,
            max_work: limits.max_work,
            ..ObjectivePlanLimits::default()
        },
        template: None,
        statistics: ObjectiveBoundStatistics::default(),
    };
    work.tick()?;
    let mut nodes = FormulaNodes::default();
    for index in 0..plan.nodes.view().len() {
        let node = plan
            .nodes
            .view()
            .node(index)
            .map_err(|error| work.error(Kind::Theory(error)))?;
        work.node(&mut nodes, node)?;
    }
    let falsum = work.node(&mut nodes, NodeView::False)?;
    let mut root = work.node(&mut nodes, NodeView::Implies(falsum, falsum))?;
    let mut costs = BTreeMap::new();
    for priority in plan.levels.keys() {
        work.tick()?;
        costs.insert(*priority, 0);
    }
    for (priority, value) in incumbent.costs() {
        work.tick()?;
        costs.insert(*priority, *value);
    }
    for (priority, bound) in costs {
        work.tick()?;
        let (elements, bound) = normalize::prepare(
            plan.levels.get(&priority).map_or(&[], Vec::as_slice),
            bound,
            falsum,
            &mut nodes,
            limits,
            &mut work,
        )?;
        let build = family(&mut nodes, &elements, bound, limits, &mut work)?;
        let suffix = work.node(&mut nodes, NodeView::And(&[build.roots()[1], root]))?;
        root = work.node(&mut nodes, NodeView::Or(&[build.roots()[0], suffix]))?;
    }
    // Account for the admission's independent count and topology passes.
    for index in 0..nodes.view().len() {
        work.tick()?;
        work.tick()?;
        let node = nodes
            .view()
            .node(index)
            .map_err(|error| work.error(Kind::Theory(error)))?;
        let occurrences = match node {
            NodeView::And(row) | NodeView::Or(row) => row.len(),
            NodeView::Implies(_, _) => 2,
            NodeView::Atom(_) | NodeView::False => 0,
        };
        for _ in 0..occurrences {
            work.tick()?;
        }
    }
    work.tick()?;
    let theory = Theory::new(
        plan.original.atom_count(),
        nodes.into_parts(),
        vec![root],
        AdmissionLimits {
            max_atoms: plan.original.atom_count(),
            max_nodes: limits.aggregate.max_nodes,
            max_roots: 1,
            max_operands: limits.aggregate.max_operands,
        },
    )
    .map_err(|error| work.error(Kind::Theory(error)))?;
    let exact = ObjectiveBound {
        original: plan.original.clone(),
        theory,
        statistics: work.statistics,
        choice_failure: None,
    };
    Ok(super::choices::strengthen(
        plan,
        exact,
        incumbent,
        limits,
        cancellation,
    ))
}

pub(super) fn family(
    nodes: &mut FormulaNodes,
    elements: &[AggregateElement],
    bound: i64,
    limits: ObjectiveBoundLimits,
    work: &mut Work<'_>,
) -> Result<AggregateFamilyBuild, ObjectiveBoundError> {
    let guards = [
        AggregateGuard {
            comparison: AggregateComparison::Lt,
            bound,
        },
        AggregateGuard {
            comparison: AggregateComparison::Eq,
            bound,
        },
    ];
    let mut aggregate = limits.aggregate;
    aggregate.max_work = aggregate
        .max_work
        .min(limits.max_work - work.statistics.work);
    let result = append_aggregate_family(
        nodes,
        elements,
        &guards,
        AggregateFamilyLimits {
            aggregate,
            max_guards: 2,
        },
        work.cancellation,
    );
    let build = match result {
        Ok(build) => build,
        Err(error) => {
            work.account(error.statistics().work)?;
            return Err(work.error(Kind::Aggregate(error)));
        }
    };
    work.account(build.statistics().work)?;
    work.statistics.nodes = nodes.view().len();
    Ok(build)
}
