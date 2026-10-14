//! Settled associative subformulas need no further ancestor notifications.

use super::*;

fn shared_prefixes(disjunction: bool, length: usize) -> Theory {
    let mut nodes = FormulaNodes::new(
        FormulaParts::new((0..=length).map(Node::atom).collect(), Vec::new()).unwrap(),
    );
    let mut prefix = 0;
    for atom in 1..length {
        let operands = [prefix, atom];
        prefix = push(
            &mut nodes,
            if disjunction {
                NodeView::Or(&operands)
            } else {
                NodeView::And(&operands)
            },
        );
        // A second consumer keeps each prefix explicit in the shared graph.
        push(&mut nodes, NodeView::Implies(prefix, length));
    }
    Theory::new(
        length + 1,
        nodes.into_parts(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn settled_shared_prefixes_do_not_queue_dirty_notifications() {
    for disjunction in [false, true] {
        let theory = shared_prefixes(disjunction, 64);
        let index = Narrower::new(&theory);
        let mut region = Region::all_open(theory.atom_count());
        let mut known = index.knowledge();
        let mut scratch = NarrowingScratch::default();
        // One absorbing leaf settles every shared prefix before the next
        // decision. The retained knowledge has already propagated that truth.
        assert!(if disjunction {
            region.hold(0)
        } else {
            region.cut(0)
        });
        let close =
            |region: &mut Region, known: &mut crate::Knowledge, scratch: &mut NarrowingScratch| {
                index
                    .narrow_known(
                        OriginalSubject::new(&theory, None),
                        region,
                        known,
                        scratch,
                        RegionLimits::default(),
                        &Cancellation::default(),
                    )
                    .unwrap()
            };
        assert_ne!(
            close(&mut region, &mut known, &mut scratch).0,
            Narrowing::Refuted
        );
        assert!(if disjunction {
            region.cut(1)
        } else {
            region.hold(1)
        });
        let (outcome, statistics) = close(&mut region, &mut known, &mut scratch);
        assert_ne!(outcome, Narrowing::Refuted);
        // This leaf has one immediate chain parent. Its neutral change cannot
        // affect any already-settled ancestor. Only its ordinary truth event
        // should enter the propagation loop; no witness check remains.
        assert_eq!(
            statistics.propagations, 1,
            "disjunction={disjunction}: {statistics:?}"
        );
        assert_eq!(region.decision(0), Some(disjunction));
        assert_eq!(region.decision(1), Some(!disjunction));
        assert!((2..theory.atom_count()).all(|atom| region.is_open(atom)));
    }
}

#[test]
fn newly_required_root_restarts_its_witness_check() {
    // P = a | b, R = P | a. With b false, R has one distinct open
    // witness even though its two immediate operands remain open.
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..4).map(Node::atom).collect(), vec![]).unwrap());
    let prefix = push(&mut nodes, NodeView::Or(&[0, 1]));
    let root = push(&mut nodes, NodeView::Or(&[prefix, 0]));
    let demand = push(&mut nodes, NodeView::Implies(2, root));
    let observer = push(&mut nodes, NodeView::Implies(prefix, 3));
    let theory = Theory::new(
        4,
        nodes.into_parts(),
        vec![demand, observer],
        AdmissionLimits::default(),
    )
    .unwrap();
    let index = Narrower::new(&theory);
    let close =
        |region: &mut Region, known: &mut crate::Knowledge, scratch: &mut NarrowingScratch| {
            index
                .narrow_known(
                    OriginalSubject::new(&theory, None),
                    region,
                    known,
                    scratch,
                    RegionLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap()
                .0
        };
    let mut inherited = Region::all_open(4);
    let mut known = index.knowledge();
    let mut scratch = NarrowingScratch::default();
    assert!(inherited.cut(1));
    assert_ne!(
        close(&mut inherited, &mut known, &mut scratch),
        Narrowing::Refuted
    );
    assert!([0, 2, 3].into_iter().all(|atom| inherited.is_open(atom)));

    // Earlier possibility notifications have drained. Requiring R through x
    // must enqueue its expanded obligation despite no changed operand of R.
    assert!(inherited.hold(2));
    assert_ne!(
        close(&mut inherited, &mut known, &mut scratch),
        Narrowing::Refuted
    );
    assert_eq!(inherited.decision(0), Some(true));
    assert_eq!(inherited.decision(3), Some(true));

    let mut fresh = Region::all_open(4);
    assert!(fresh.cut(1) && fresh.hold(2));
    assert_ne!(
        close(
            &mut fresh,
            &mut index.knowledge(),
            &mut NarrowingScratch::default()
        ),
        Narrowing::Refuted
    );
    // Retained branch preferences may differ; the closed atom decisions may not.
    for atom in 0..theory.atom_count() {
        assert_eq!(inherited.decision(atom), fresh.decision(atom));
    }
}
