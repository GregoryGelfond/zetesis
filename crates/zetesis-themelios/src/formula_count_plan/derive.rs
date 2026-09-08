//! Greedy source-group covers and strictly stronger conditional consequences.
//!
//! Each global group considers capacity groups in their retained grounding order,
//! choosing a group only if disjoint from the current cover. There is no
//! backtracking: an early large group can hide a better later cover. This finite
//! discovery policy is incomplete; every emitted consequence remains sound.

use themelios_base::span::Location;
use zetesis_ferraris::{Theory, partition};

use super::capture::Group;
use super::{
    CountPlan, CountPlanFailureKind as Fault, CountPlanResource as Resource, Work, ceiling,
};

pub(super) struct Consequence {
    pub location: Location,
    pub body: usize,
    pub members: Vec<usize>,
    pub lower: usize,
    pub origins: Vec<Location>,
}

pub(super) fn plan(
    groups: &[Group],
    theory: &Theory,
    work: &mut Work,
) -> Result<Option<CountPlan>, Fault> {
    work.poll()?;
    let mut consequences = Vec::new();
    for (global_index, global) in groups.iter().enumerate() {
        work.location = global.location;
        work.charge(1)?;
        if global.lower == 0 || global.members.len() < 2 {
            continue;
        }
        validate_source(global, theory, work)?;
        let next = work
            .statistics
            .attempts
            .checked_add(1)
            .ok_or(Fault::Overflow)?;
        ceiling(next, work.limits.max_attempts, Resource::Attempts)?;
        work.statistics.attempts = next;
        let Some(indices) = cover(global_index, groups, theory, work)? else {
            continue;
        };
        let plan = partition(global, &indices, groups, theory, work)?;
        // The first profile emits only improved local bounds. An inconsistent
        // cover is left to the original theory, rather than asserting falsum.
        if plan.inconsistent() {
            continue;
        }
        append_consequences(global, &indices, groups, &plan, &mut consequences, work)?;
    }
    if consequences.is_empty() {
        work.poll()?;
        return Ok(None);
    }
    let (restriction, origins) = super::emit::restriction(theory, &consequences, work)?;
    work.statistics.consequences = consequences.len();
    work.poll()?;
    Ok(Some(CountPlan {
        original: theory.clone(),
        restriction,
        origins,
        statistics: work.statistics,
    }))
}

fn validate_source(group: &Group, theory: &Theory, work: &mut Work) -> Result<(), Fault> {
    work.charge(u64::try_from(theory.roots().len()).map_err(|_| Fault::Overflow)? + 1)?;
    if group.body >= theory.nodes().len()
        || group.bound_root >= theory.nodes().len()
        || !theory.roots().contains(&group.asserted_root)
    {
        return Err(Fault::SourceMapping);
    }
    Ok(())
}

fn cover(
    global_index: usize,
    groups: &[Group],
    theory: &Theory,
    work: &mut Work,
) -> Result<Option<Vec<usize>>, Fault> {
    let global = &groups[global_index];
    let mut covered = work.vector::<bool>(theory.atom_count())?;
    for _ in 0..theory.atom_count() {
        work.charge(1)?;
        covered.push(false);
    }
    let mut indices = work.vector(groups.len())?;
    let mut count = 0_usize;
    for (index, group) in groups.iter().enumerate() {
        work.charge(1)?;
        if index == global_index
            || group.upper == group.members.len()
            || (group.body != 1 && group.body != global.body)
        {
            continue;
        }
        let mut fits = true;
        for &atom in &group.members {
            work.charge(u64::try_from(global.members.len()).map_err(|_| Fault::Overflow)? + 1)?;
            if covered[atom] || global.members.binary_search(&atom).is_err() {
                fits = false;
                break;
            }
        }
        if !fits {
            continue;
        }
        validate_source(group, theory, work)?;
        for &atom in &group.members {
            work.charge(1)?;
            covered[atom] = true;
        }
        count = count
            .checked_add(group.members.len())
            .ok_or(Fault::Overflow)?;
        indices.push(index);
    }
    Ok((count == global.members.len() && indices.len() >= 2).then_some(indices))
}
fn partition(
    global: &Group,
    indices: &[usize],
    groups: &[Group],
    theory: &Theory,
    work: &mut Work,
) -> Result<partition::Plan, Fault> {
    let mut caps = work.vector(indices.len())?;
    for &index in indices {
        work.charge(1)?;
        caps.push(partition::Group {
            members: &groups[index].members,
            upper: groups[index].upper,
        });
    }
    let mut limits = work.limits.partition;
    limits.max_work = limits
        .max_work
        .min(work.limits.max_work - work.statistics.work);
    limits.max_bytes = limits
        .max_bytes
        .min(work.limits.max_bytes - work.statistics.storage_bytes);
    let plan = partition::Plan::new(
        partition::Premises {
            atom_count: theory.atom_count(),
            members: &global.members,
            lower: global.lower,
            groups: &caps,
        },
        limits,
        &work.control,
    );
    let statistics = match &plan {
        Ok(plan) => plan.statistics(),
        Err(error) => error.statistics(),
    };
    work.record(statistics.work)?;
    if statistics.construction_bytes <= work.limits.max_bytes - work.statistics.storage_bytes {
        work.payload(statistics.construction_bytes)?;
    }
    let plan = plan.map_err(Fault::Partition)?;
    Ok(plan)
}
fn append_consequences(
    global: &Group,
    indices: &[usize],
    groups: &[Group],
    plan: &partition::Plan,
    consequences: &mut Vec<Consequence>,
    work: &mut Work,
) -> Result<(), Fault> {
    for (consequence, &index) in plan.consequences().zip(indices) {
        work.charge(1)?;
        if consequence.lower <= groups[index].lower {
            continue;
        }
        let mut members = work.vector(consequence.members.len())?;
        for &atom in consequence.members {
            work.charge(1)?;
            members.push(atom);
        }
        let origin_count = global
            .origins
            .len()
            .checked_add(
                indices
                    .iter()
                    .try_fold(0_usize, |n, &i| n.checked_add(groups[i].origins.len()))
                    .ok_or(Fault::Overflow)?,
            )
            .ok_or(Fault::Overflow)?;
        let total = work
            .statistics
            .origins
            .checked_add(origin_count)
            .ok_or(Fault::Overflow)?;
        ceiling(total, work.limits.max_origins, Resource::Origins)?;
        let mut origins = work.vector(origin_count)?;
        for &origin in global
            .origins
            .iter()
            .chain(indices.iter().flat_map(|&i| &groups[i].origins))
        {
            work.charge(1)?;
            origins.push(origin);
        }
        work.statistics.origins = total;
        work.payload(super::bytes::<Consequence>(1)?)?;
        consequences
            .try_reserve_exact(1)
            .map_err(|_| Fault::Stopped(zetesis_cpu::Stop::Allocation))?;
        consequences.push(Consequence {
            location: global.location,
            body: global.body,
            members,
            lower: consequence.lower,
            origins,
        });
    }
    Ok(())
}
