//! A requested stop keeps its precedence over emission refusals.

use super::{Builder, RestrictionErrorKind, RestrictionLimits};
use crate::{AdmissionError, FormulaNodes, NodeView};
use zetesis_cpu::{Cancellation, Refusal, Stop};

/// An emitter that has charged seven operations, which is not a metered boundary.
fn builder(cancellation: &Cancellation, limits: RestrictionLimits) -> Builder<'_> {
    Builder {
        nodes: FormulaNodes::default(),
        roots: Vec::new(),
        work: 7,
        limits,
        cancellation,
    }
}

#[test]
fn a_requested_stop_precedes_the_work_limit_between_boundaries() {
    let limits = RestrictionLimits {
        max_work: 7,
        ..RestrictionLimits::default()
    };
    let cancellation = Cancellation::default();
    assert_eq!(
        builder(&cancellation, limits).tick(),
        Err(RestrictionErrorKind::Work)
    );
    cancellation.cancel();
    assert_eq!(
        builder(&cancellation, limits).tick(),
        Err(RestrictionErrorKind::Stopped(Stop::Cancelled))
    );
}

#[test]
fn node_and_root_refusals_settle_to_a_requested_stop() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut nodes = RestrictionLimits::default();
    nodes.theory.max_nodes = 0;
    let mut roots = RestrictionLimits::default();
    roots.theory.max_roots = 0;
    for (refusal, expected) in [
        (
            builder(&cancellation, nodes)
                .node(NodeView::False)
                .map(|_| ()),
            RestrictionErrorKind::Nodes,
        ),
        (
            builder(&cancellation, roots).root(0),
            RestrictionErrorKind::Roots,
        ),
    ] {
        // Between boundaries the emitter reports the refusal itself; emission
        // settles it at its exit.
        assert_eq!(refusal, Err(expected));
        assert_eq!(
            cancellation.settle(refusal),
            Err(RestrictionErrorKind::Stopped(Stop::Cancelled))
        );
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
