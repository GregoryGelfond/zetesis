//! An emission refusal between metered boundaries reports itself without
//! observing a requested stop; settled where a metered operation returns, as
//! emission's own refusals are, it becomes the stop.

use super::{Builder, RestrictionErrorKind, RestrictionLimits};
use crate::{
    AdmissionError, AggregateComparison, AggregateElement, AggregateErrorKind, FormulaNodes,
    NodeView,
};
use zetesis_cpu::{Cancellation, Refusal, Stop};

/// Run `test` on an emitter whose meter has admitted seven operations, which is
/// not a metered boundary; with `stop`, a stop is requested once it has. The
/// metered exit settles what `test` returns.
fn with_builder<T>(
    limits: RestrictionLimits,
    stop: bool,
    test: impl FnOnce(&mut Builder<'_, '_>) -> Result<T, RestrictionErrorKind>,
) -> Result<T, RestrictionErrorKind> {
    let cancellation = Cancellation::default();
    cancellation
        .metered(limits.max_work, |meter| {
            for _ in 0..7 {
                meter.charge().unwrap();
            }
            if stop {
                cancellation.cancel();
            }
            test(&mut Builder {
                nodes: FormulaNodes::default(),
                roots: Vec::new(),
                limits,
                cancellation: &cancellation,
                meter,
            })
        })
        .result
}

#[test]
fn a_requested_stop_precedes_the_work_limit_between_boundaries() {
    let limits = RestrictionLimits {
        max_work: 7,
        ..RestrictionLimits::default()
    };
    // The tick's own report, returned as a value so the exit leaves it alone.
    let tick = |stop| with_builder(limits, stop, |builder| Ok(builder.tick())).unwrap();
    assert_eq!(tick(false), Err(RestrictionErrorKind::Work));
    assert_eq!(
        tick(true),
        Err(RestrictionErrorKind::Stopped(Stop::Cancelled))
    );
}

#[test]
fn node_and_root_refusals_settle_to_a_requested_stop() {
    let mut nodes = RestrictionLimits::default();
    nodes.theory.max_nodes = 0;
    let mut roots = RestrictionLimits::default();
    roots.theory.max_roots = 0;
    // Between boundaries the emitter reports each refusal itself; the metered
    // exit reports the stop.
    let node = with_builder(nodes, true, |builder| {
        let refusal = builder.node(NodeView::False).map(|_| ());
        assert_eq!(refusal, Err(RestrictionErrorKind::Nodes));
        refusal
    });
    let root = with_builder(roots, true, |builder| {
        let refusal = builder.root(0);
        assert_eq!(refusal, Err(RestrictionErrorKind::Roots));
        refusal
    });
    for settled in [node, root] {
        assert_eq!(settled, Err(RestrictionErrorKind::Stopped(Stop::Cancelled)));
    }
}

#[test]
fn emission_errors_classify_resource_refusals() {
    for refusal in [
        RestrictionErrorKind::Atoms,
        RestrictionErrorKind::Nodes,
        RestrictionErrorKind::Roots,
        RestrictionErrorKind::Elements,
        RestrictionErrorKind::Work,
        RestrictionErrorKind::Overflow,
        RestrictionErrorKind::Theory(AdmissionError::Limit),
        RestrictionErrorKind::Theory(AdmissionError::Allocation),
        RestrictionErrorKind::Stopped(Stop::Allocation),
    ] {
        assert!(refusal.is_resource_refusal(), "{refusal:?}");
    }
    for shape in [
        RestrictionErrorKind::Theory(AdmissionError::Root),
        RestrictionErrorKind::Stopped(Stop::Cancelled),
    ] {
        assert!(!shape.is_resource_refusal(), "{shape:?}");
    }
}

/// The work cardinality lowering reports for `members` under the lower bound
/// `lower`, over a DAG holding just their atom nodes, as an emitter builds it.
fn lowering_work(members: &[usize], lower: usize, limits: RestrictionLimits) -> u64 {
    let mut nodes = FormulaNodes::default();
    let mut elements = Vec::new();
    for &atom in members {
        let mut transaction = nodes.transaction();
        let condition = transaction
            .push(
                NodeView::Atom(atom),
                limits.theory.max_nodes,
                limits.theory.max_operands,
            )
            .unwrap();
        transaction.commit();
        elements.push(AggregateElement {
            condition,
            weight: 1,
        });
    }
    let mut aggregate = limits.aggregate;
    aggregate.max_nodes = aggregate.max_nodes.min(limits.theory.max_nodes);
    aggregate.max_operands = aggregate.max_operands.min(limits.theory.max_operands);
    nodes
        .append_aggregate(
            &elements,
            AggregateComparison::Ge,
            i64::try_from(lower).unwrap(),
            aggregate,
            &Cancellation::default(),
        )
        .unwrap()
        .statistics()
        .work
}

#[test]
fn emission_counts_its_cardinality_lowering_work() {
    let limits = RestrictionLimits::default();
    let members = [0, 1, 2, 3];
    let lowering = lowering_work(&members, 2, limits);
    let charged = with_builder(limits, false, |builder| {
        builder.lower(&members, 2)?;
        Ok(builder.meter.charged())
    })
    .unwrap();
    // Seven charged before, one per atom node, and the lowering's own report.
    assert_eq!(charged, 7 + members.len() as u64 + lowering);
}

#[test]
fn a_refused_lowering_counts_the_work_it_did() {
    let members = [0, 1, 2, 3];
    let lowering = lowering_work(&members, 2, RestrictionLimits::default());
    // The lowering's allowance falls one operation short of its work.
    let limits = RestrictionLimits {
        max_work: 7 + members.len() as u64 + lowering - 1,
        ..RestrictionLimits::default()
    };
    let (refusal, charged) = with_builder(limits, false, |builder| {
        let refusal = builder.lower(&members, 2).map(|_| ());
        Ok((refusal, builder.meter.charged()))
    })
    .unwrap();
    assert!(matches!(refusal, Err(RestrictionErrorKind::Aggregate(_))));
    assert_eq!(charged, limits.max_work);
}

#[test]
fn a_lowering_refused_at_its_own_ceiling_stays_below_the_limit() {
    let members = [0, 1, 2, 3];
    let lowering = lowering_work(&members, 2, RestrictionLimits::default());
    // The per-translation ceiling falls one operation short of the lowering's
    // work, far inside the emission's remaining allowance.
    let mut limits = RestrictionLimits::default();
    limits.aggregate.max_work = lowering - 1;
    let (refusal, charged) = with_builder(limits, false, |builder| {
        let refusal = builder.lower(&members, 2).map(|_| ());
        Ok((refusal, builder.meter.charged()))
    })
    .unwrap();
    assert!(matches!(
        refusal,
        Err(RestrictionErrorKind::Aggregate(error))
            if error.kind() == AggregateErrorKind::WorkLimit
    ));
    // Seven charged before, one per atom node, and the lowering's work up to
    // the ceiling that bound it.
    assert_eq!(
        charged,
        7 + members.len() as u64 + limits.aggregate.max_work
    );
}
