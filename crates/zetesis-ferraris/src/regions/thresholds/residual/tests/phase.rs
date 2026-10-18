//! Optional arithmetic waits for the existing mandatory fixed point.

use crate::regions::{Subject, chain_position, witnesses};

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
    scratch.prepare(65);
    scratch
        .prepare_residuals(index.thresholds.rows.len())
        .unwrap();
    scratch.witnesses.prepare_dirty(index.chains.len()).unwrap();
    // Redundant pending notifications are legal. A zero allowance identifies
    // which consumer runs first without changing the already closed knowledge.
    scratch.queue_residual(0);
    scratch.witnesses.queue_dirty(chain);
    let mut closure = Closure {
        known: known.borrow(),
        lists: &mut scratch,
    };
    let result = closure.close(
        Subject {
            theory: &theory,
            producers: None,
            frozen: None,
            choices: &crate::regions::NO_CHOICES,
            generator_order: None,
        },
        &index,
        &region,
        &mut Work::new(0),
        &mut NarrowingStatistics::default(),
    );
    assert!(matches!(result, Err(Stop::WorkLimit)));
    assert_eq!(scratch.witnesses.next_dirty(), None);
    assert_eq!(scratch.next_residual(), Some(0));
}
