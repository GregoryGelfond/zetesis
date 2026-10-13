//! An emission refusal between metered boundaries reports itself without
//! observing a requested stop; settled where a metered operation returns, as
//! emission's own refusals are, it becomes the stop.

use super::{Builder, RestrictionErrorKind, RestrictionLimits};
use crate::{AdmissionError, FormulaNodes, NodeView};
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
