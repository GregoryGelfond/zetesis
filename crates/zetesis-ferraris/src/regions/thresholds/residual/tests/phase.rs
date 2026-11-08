//! Optional arithmetic waits for the existing mandatory fixed point.

use crate::regions::{Context, chain_position, witnesses};

use super::*;

#[test]
fn residual_checks_wait_for_witness_events() {
    let base = residual();
    let mut nodes = FormulaNodes::new(base.parts().try_clone().unwrap());
    let mut append = nodes.transaction();
    let prefix = append
        .push(NodeView::Or(&[1, 2]), usize::MAX, usize::MAX)
        .unwrap();
    append
        .push(NodeView::Or(&[prefix, 3]), usize::MAX, usize::MAX)
        .unwrap();
    append
        .push(NodeView::Or(&[prefix, 4]), usize::MAX, usize::MAX)
        .unwrap();
    append.commit();
    let theory = Theory::new(
        65,
        nodes.into_parts(),
        base.roots().to_vec(),
        AdmissionLimits::default(),
    )
    .unwrap();
    let index = Narrower::new(&theory);
    let chain = chain_position(index.chain_of[prefix].unwrap());
    assert!(witnesses::participates(&index, chain));
    let mut region = Region::all_open(65);
    let mut knowledge = index.knowledge();
    assert_ne!(
        close(
            &theory,
            &index,
            &mut region,
            &mut knowledge,
            &mut NarrowingScratch::default()
        )
        .0,
        Narrowing::Refuted
    );
    let Width::Compact16(known) = &mut knowledge.width else {
        panic!("the small residual fixture uses compact counters");
    };
    assert!(known.thresholds[0].has_residual_interval());
    let mut scratch = NarrowingScratch::default();
    scratch.common.prepare(65);
    scratch
        .common
        .prepare_residuals(index.thresholds.rows.len())
        .unwrap();
    scratch
        .common
        .witnesses
        .prepare_dirty(index.chains.len())
        .unwrap();
    // Redundant pending notifications are legal. A zero allowance identifies
    // which consumer runs first without changing the already closed knowledge.
    scratch.common.queue_residual(0);
    scratch.common.witnesses.queue_dirty(chain);
    let mut closure = Closure::<_, zetesis_algebra::AnswerSetsSought> {
        known: known.borrow(),
        lists: &mut scratch.common,
        learning: &mut scratch.learning,
    };
    let result = closure
        .close(
            Context::from(crate::OriginalSubject::new(&theory, None)),
            &index,
            &region,
            &mut Meter::local(0),
            &mut NarrowingStatistics::default(),
        )
        .map_err(zetesis_algebra::Failure::into_stop);
    assert!(matches!(result, Err(Stop::WorkLimit)));
    assert_eq!(scratch.common.witnesses.next_dirty(), None);
    assert_eq!(scratch.common.next_residual(), Some(0));
}
