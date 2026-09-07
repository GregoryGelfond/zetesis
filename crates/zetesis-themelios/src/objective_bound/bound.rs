use std::collections::BTreeMap;

use zetesis_cpu::Control;
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison, AggregateElement, AggregateFamilyBuild,
    AggregateFamilyLimits, AggregateGuard, Node, Theory, append_aggregate_family,
};
use zetesis_objective::Score;

use super::{
    ObjectiveBound, ObjectiveBoundError, ObjectiveBoundErrorKind as Kind, ObjectiveBoundLimits,
    ObjectiveBoundStatistics, ObjectivePlan, ObjectivePlanLimits, Work,
};

mod normalize;

pub(super) fn compile(
    plan: &ObjectivePlan,
    incumbent: &Score,
    limits: ObjectiveBoundLimits,
    control: &Control,
) -> Result<ObjectiveBound, ObjectiveBoundError> {
    let mut work = Work {
        control,
        limits: ObjectivePlanLimits {
            max_nodes: limits.aggregate.max_nodes,
            max_work: limits.max_work,
            ..ObjectivePlanLimits::default()
        },
        template: None,
        statistics: ObjectiveBoundStatistics::default(),
    };
    work.tick()?;
    let mut nodes = Vec::new();
    for node in &plan.nodes {
        work.node(&mut nodes, *node)?;
    }
    let falsum = work.node(&mut nodes, Node::False)?;
    let mut root = work.node(&mut nodes, Node::Implies(falsum, falsum))?;
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
        let suffix = work.node(&mut nodes, Node::And(build.roots()[1], root))?;
        root = work.node(&mut nodes, Node::Or(build.roots()[0], suffix))?;
    }
    // Account for the final independent topology validation before admitting it.
    for _ in &nodes {
        work.tick()?;
    }
    let theory = Theory::new(
        plan.original.atom_count(),
        nodes,
        vec![root],
        AdmissionLimits {
            max_atoms: plan.original.atom_count(),
            max_nodes: limits.aggregate.max_nodes,
            max_roots: 1,
        },
    )
    .map_err(|error| work.error(Kind::Theory(error)))?;
    Ok(ObjectiveBound {
        original: plan.original.clone(),
        theory,
        statistics: work.statistics,
    })
}

fn family(
    nodes: &mut Vec<Node>,
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
        work.control,
    );
    let build = match result {
        Ok(build) => build,
        Err(error) => {
            work.account(error.statistics().work)?;
            return Err(work.error(Kind::Aggregate(error)));
        }
    };
    work.account(build.statistics().work)?;
    work.statistics.nodes = nodes.len();
    Ok(build)
}
