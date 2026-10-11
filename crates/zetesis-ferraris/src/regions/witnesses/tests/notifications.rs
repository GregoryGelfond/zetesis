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
fn settled_shared_prefixes_do_not_relay_dirty_notifications() {
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
        // affect any already-settled ancestor: only the leaf event and its
        // immediate pending witness notification may be visited.
        assert!(
            statistics.propagations <= 2,
            "disjunction={disjunction}: {statistics:?}"
        );
        assert_eq!(region.decision(0), Some(disjunction));
        assert_eq!(region.decision(1), Some(!disjunction));
        assert!((2..theory.atom_count()).all(|atom| region.is_open(atom)));
    }
}
