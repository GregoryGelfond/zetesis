//! A requested stop keeps its precedence over emission refusals.

use super::{Builder, RestrictionErrorKind, RestrictionLimits};
use crate::{FormulaNodes, NodeView};
use zetesis_cpu::{Cancellation, Stop};

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
fn a_requested_stop_precedes_the_node_limit() {
    let mut limits = RestrictionLimits::default();
    limits.theory.max_nodes = 0;
    let cancellation = Cancellation::default();
    assert_eq!(
        builder(&cancellation, limits).node(NodeView::False),
        Err(RestrictionErrorKind::Nodes)
    );
    cancellation.cancel();
    assert_eq!(
        builder(&cancellation, limits).node(NodeView::False),
        Err(RestrictionErrorKind::Stopped(Stop::Cancelled))
    );
}

#[test]
fn a_requested_stop_precedes_the_root_limit() {
    let mut limits = RestrictionLimits::default();
    limits.theory.max_roots = 0;
    let cancellation = Cancellation::default();
    assert_eq!(
        builder(&cancellation, limits).root(0),
        Err(RestrictionErrorKind::Roots)
    );
    cancellation.cancel();
    assert_eq!(
        builder(&cancellation, limits).root(0),
        Err(RestrictionErrorKind::Stopped(Stop::Cancelled))
    );
}
