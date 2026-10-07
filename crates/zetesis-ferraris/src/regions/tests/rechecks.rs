//! A support recheck is queued at most once until it runs, and a support that
//! falls after an atom's recheck has it rechecked again.

use zetesis_cpu::Cancellation;

use crate::{
    AdmissionLimits, Narrower, Narrowing, NarrowingScratch, NarrowingStatistics, Node,
    OriginalSubject, Producers, Region, RegionLimits, Theory,
};

/// Atom 0 with one producer per body atom `1..=bodies`, `a ← b_i`, each body
/// a free choice `{b_i}`, written `b_i ∨ ¬b_i`.
fn shared_head(bodies: usize) -> Theory {
    let mut nodes: Vec<Node> = (0..=bodies).map(Node::atom).collect();
    nodes.push(Node::falsum());
    let falsum = nodes.len() - 1;
    let mut roots = Vec::new();
    for body in 1..=bodies {
        nodes.push(Node::implies(body, 0));
        roots.push(nodes.len() - 1);
        nodes.push(Node::implies(body, falsum));
        nodes.push(Node::or_pair([body, nodes.len() - 1]));
        roots.push(nodes.len() - 1);
    }
    Theory::new(
        bodies + 1,
        crate::FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn producers(theory: &Theory) -> Producers {
    super::super::producers(theory, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .producers
        .expect("every root is a rule of the support fragment")
}

fn narrow(
    theory: &Theory,
    producers: &Producers,
    narrower: &Narrower,
    region: &mut Region,
    knowledge: &mut crate::Knowledge,
    scratch: &mut NarrowingScratch,
) -> (Narrowing, NarrowingStatistics) {
    narrower
        .narrow_known(
            OriginalSubject::new(theory, Some(producers)),
            region,
            knowledge,
            scratch,
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap()
}

/// The work of narrowing a held head with `bodies` producers, all but two of
/// whose bodies fail: each failure queues the head's recheck, which reads
/// every producer and decides nothing.
fn held_head_work(bodies: usize) -> u64 {
    let theory = shared_head(bodies);
    let producers = producers(&theory);
    let narrower = Narrower::new(&theory);
    let mut region = Region::all_open(theory.atom_count());
    region.hold(0);
    for body in 3..=bodies {
        region.cut(body);
    }
    let (narrowing, statistics) = narrow(
        &theory,
        &producers,
        &narrower,
        &mut region,
        &mut narrower.knowledge(),
        &mut NarrowingScratch::default(),
    );
    assert_eq!(narrowing, Narrowing::Fixed { changed: false });
    statistics.work
}

#[test]
fn a_repeatedly_queued_recheck_runs_once() {
    // One recheck per failed body makes the work quadratic in the bodies
    // (doubling them multiplies it by about 3.4 here); queued once, the
    // head's rechecks keep it linear.
    let (small, large) = (held_head_work(20), held_head_work(40));
    assert!(
        large * 2 < small * 5,
        "work {small} at 20 bodies, {large} at 40"
    );
}

#[test]
fn a_support_that_falls_after_a_recheck_is_rechecked() {
    let theory = shared_head(2);
    let producers = producers(&theory);
    let narrower = Narrower::new(&theory);
    let mut scratch = NarrowingScratch::default();
    let mut knowledge = narrower.knowledge();
    let mut region = Region::all_open(theory.atom_count());
    // The first narrowing rechecks the head: two producers support it.
    narrow(
        &theory,
        &producers,
        &narrower,
        &mut region,
        &mut knowledge,
        &mut scratch,
    );
    assert_eq!(region.decision(0), None);
    // Both bodies then fail, with the same scratch: the head loses its
    // support and must be rechecked again, and so cut.
    region.cut(1);
    region.cut(2);
    let (narrowing, _) = narrow(
        &theory,
        &producers,
        &narrower,
        &mut region,
        &mut knowledge,
        &mut scratch,
    );
    assert_eq!(narrowing, Narrowing::Fixed { changed: true });
    assert_eq!(region.decision(0), Some(false));
}
