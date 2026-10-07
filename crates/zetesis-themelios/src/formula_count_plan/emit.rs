//! Exact guarded consequence view; original nodes remain borrowed semantic data.

use crate::ProgramSite;
use zetesis_ferraris::{
    AggregateComparison, AggregateElement, FormulaNodes, Node, NodeView, Theory, append_aggregate,
};

use super::derive::Consequence;
use super::{CountPlanFailureKind as Fault, Work};

pub(super) fn restriction(
    original: &Theory,
    consequences: &[Consequence],
    work: &mut Work,
) -> Result<(Theory, Vec<ProgramSite>), Fault> {
    let limit = work.limits.theory;
    if original.atom_count() > limit.max_atoms {
        return Err(Fault::Theory(zetesis_ferraris::AdmissionError::Limit));
    }
    // A bounded prefix copy preserves every original activation ID exactly.
    // Its nodes are not asserted: only the new consequence roots are roots of
    // this separate candidate theory. No old root ID is replaced or aliased.
    let prefix = consequences
        .iter()
        .map(|c| c.body)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(Fault::Overflow)?;
    if prefix > limit.max_nodes {
        return Err(Fault::Theory(zetesis_ferraris::AdmissionError::Limit));
    }
    let mut nodes = FormulaNodes::default();
    for index in 0..prefix {
        let node = original.view().node(index).map_err(Fault::Theory)?;
        let copied = push(&mut nodes, node, work)?;
        debug_assert_eq!(copied, index, "admitted rows keep their original IDs");
    }
    let mut roots = work.vector(consequences.len())?;
    if consequences.len() > limit.max_roots {
        return Err(Fault::Theory(zetesis_ferraris::AdmissionError::Limit));
    }
    let origin_count = consequences
        .iter()
        .try_fold(0_usize, |n, c| n.checked_add(c.origins.len()))
        .ok_or(Fault::Overflow)?;
    let total_origins = work
        .statistics
        .origins
        .checked_add(origin_count)
        .ok_or(Fault::Overflow)?;
    super::ceiling(
        total_origins,
        work.limits.max_origins,
        super::CountPlanResource::Origins,
    )?;
    let mut origins = work.vector(origin_count)?;
    work.statistics.origins = total_origins;
    for consequence in consequences {
        work.location = consequence.location;
        work.charge(1)?;
        let lower = lower(&mut nodes, consequence, work)?;
        let root = if consequence.body == 1 {
            lower
        } else {
            push(&mut nodes, NodeView::Implies(consequence.body, lower), work)?
        };
        roots.push(root);
        for &origin in &consequence.origins {
            work.charge(1)?;
            origins.push(origin);
        }
    }
    let admission =
        2 * nodes.view().len() as u128 + nodes.parts().occurrences() as u128 + roots.len() as u128;
    work.charge(u64::try_from(admission).map_err(|_| Fault::Overflow)?)?;
    let result = Theory::new(original.atom_count(), nodes.into_parts(), roots, limit)
        .map_err(Fault::Theory)?;
    work.poll()?;
    Ok((result, origins))
}

fn lower(
    nodes: &mut FormulaNodes,
    consequence: &Consequence,
    work: &mut Work,
) -> Result<usize, Fault> {
    let limit = work.limits.theory;
    let mut elements = work.vector(consequence.members.len())?;
    for &atom in &consequence.members {
        let node = push(nodes, NodeView::Atom(atom), work)?;
        elements.push(AggregateElement {
            weight: 1,
            condition: node,
        });
    }
    let mut limits = work.limits.aggregate;
    limits.max_nodes = limits.max_nodes.min(limit.max_nodes);
    limits.max_operands = limits.max_operands.min(limit.max_operands);
    limits.max_work = limits
        .max_work
        .min(work.limits.max_work - work.statistics.work);
    // Positive unit-weight Ge emits two constants and at most two nodes
    // per member/threshold cell. Its two usize rows have width lower+1.
    // Preflight that exact algorithm's worst-case logical payload before
    // calling the separately limited, transactional aggregate constructor.
    let cells = consequence
        .lower
        .checked_add(1)
        .and_then(|n| n.checked_mul(2))
        .ok_or(Fault::Overflow)?;
    let appended = consequence
        .members
        .len()
        .checked_mul(consequence.lower)
        .and_then(|n| n.checked_mul(2))
        .and_then(|n| n.checked_add(2))
        .ok_or(Fault::Overflow)?;
    work.payload(super::bytes::<usize>(cells)?)?;
    work.payload(super::bytes::<Node>(appended)?)?;
    let build = append_aggregate(
        nodes,
        &elements,
        AggregateComparison::Ge,
        i64::try_from(consequence.lower).map_err(|_| Fault::Overflow)?,
        limits,
        &work.cancellation,
    );
    let statistics = match &build {
        Ok(build) => build.statistics(),
        Err(error) => error.statistics(),
    };
    work.record(statistics.work)?;
    let build = build.map_err(Fault::Aggregate)?;
    Ok(build.root())
}

fn push(nodes: &mut FormulaNodes, node: NodeView<'_>, work: &mut Work) -> Result<usize, Fault> {
    let edges = match node {
        NodeView::And(row) | NodeView::Or(row) => row.len(),
        NodeView::Implies(_, _) => 2,
        NodeView::Atom(_) | NodeView::False => 0,
    };
    let wide = matches!(node, NodeView::And(row) | NodeView::Or(row) if row.len() >= 3);
    let copies = if wide { edges } else { 0 };
    work.charge(
        u64::try_from(1_usize.saturating_add(edges).saturating_add(copies))
            .map_err(|_| Fault::Overflow)?,
    )?;
    if nodes.view().len() >= work.limits.theory.max_nodes
        || nodes.parts().occurrences() as u128 + edges as u128
            > work.limits.theory.max_operands as u128
    {
        return Err(Fault::Theory(zetesis_ferraris::AdmissionError::Limit));
    }
    work.payload(super::bytes::<Node>(1)?)?;
    work.payload(super::bytes::<usize>(copies)?)?;
    let mut transaction = nodes.transaction();
    let index = transaction
        .push(
            node,
            work.limits.theory.max_nodes,
            work.limits.theory.max_operands,
        )
        .map_err(Fault::Theory)?;
    transaction.commit();
    Ok(index)
}
