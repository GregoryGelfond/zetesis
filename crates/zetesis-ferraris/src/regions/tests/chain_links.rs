//! Optional chain links preserve logical positions and the closure they select.

use std::mem::{size_of, size_of_val};
use std::num::NonZeroUsize;

use zetesis_cpu::Cancellation;

use crate::{
    AdmissionLimits, EvaluationLimits, EvaluationWorkspace, Interpretation, Narrower, Node, Region,
    RegionLimits, Theory,
};

use super::super::{chain_position, encoded_chain};

fn dissolved_chains() -> Theory {
    use Node::{And, Atom, Or};

    Theory::new(
        4,
        vec![
            Atom(0),
            Atom(1),
            Atom(2),
            Atom(3),
            Or(0, 1),
            Or(4, 2),
            And(1, 2),
            And(6, 3),
            Or(5, 7),
        ],
        vec![5, 8],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn chain_encoding_preserves_zero_based_positions() {
    for position in [0, 1, usize::MAX - 1] {
        assert_eq!(chain_position(encoded_chain(position)), position);
    }
}

#[test]
#[should_panic(expected = "allocated chain position")]
fn chain_encoding_checks_the_unrepresentable_position() {
    // No allocated chain vector can have this position. Exercise the checked
    // representation boundary without allocating a correspondingly large DAG.
    let _ = encoded_chain(usize::MAX);
}

#[test]
fn non_chain_nodes_have_no_chain_link() {
    for nodes in [
        vec![],
        vec![Node::Atom(0), Node::False, Node::Implies(0, 1)],
    ] {
        let theory = Theory::new(1, nodes, vec![], AdmissionLimits::default()).unwrap();
        let narrower = Narrower::new(&theory);
        assert_eq!(narrower.chain_of.len(), theory.nodes().len());
        assert!(narrower.chain_of.iter().all(Option::is_none));
    }
}

#[test]
fn live_chain_links_follow_dissolved_chain_removal() {
    let narrower = Narrower::new(&dissolved_chains());
    let positions: Vec<_> = narrower
        .chain_of
        .iter()
        .map(|link| link.map(chain_position))
        .collect();
    // Dissolving chains 4 and 6 leaves holes in the construction order. Chain 5
    // is a root and cannot dissolve into 8; chain 7 has a different connective.
    assert_eq!(
        positions,
        [
            None,
            None,
            None,
            None,
            None,
            Some(0),
            None,
            Some(1),
            Some(2)
        ]
    );
    assert_eq!(
        narrower
            .chains
            .iter()
            .map(|chain| (chain.root, chain.disjunction, chain.operands.as_slice()))
            .collect::<Vec<_>>(),
        [
            (5, true, &[0, 1, 2][..]),
            (7, false, &[1, 2, 3][..]),
            (8, true, &[5, 7][..])
        ]
    );
    assert_eq!(
        narrower.absorbed,
        [false, false, false, false, true, false, true, false, false]
    );
}

fn partial_region(mut code: usize) -> Region {
    let mut region = Region::all_open(4);
    for atom in 0..4 {
        match code % 3 {
            1 => assert!(region.cut(atom)),
            2 => assert!(region.hold(atom)),
            _ => {}
        }
        code /= 3;
    }
    region
}

fn compare_closures(theory: &Theory, frozen: Option<&[bool]>) {
    let original = Narrower::new(theory);
    let mut reordered = original.clone();
    reordered.chains.reverse();
    reordered.chain_of.fill(None);
    // Reconstruct links from each chain's authoritative root. The consumer
    // comparison must not obtain its reference links by decoding the original.
    for (position, chain) in reordered.chains.iter().enumerate() {
        reordered.chain_of[chain.root] = Some(encoded_chain(position));
    }
    assert_ne!(original.chains[0].root, reordered.chains[0].root);
    let cancellation = Cancellation::default();
    for code in 0..3_usize.pow(4) {
        let close = |index: &Narrower| {
            let mut region = partial_region(code);
            let mut knowledge = index.knowledge();
            let result = if let Some(truth) = frozen {
                index.narrow_frozen_known(
                    theory,
                    truth,
                    &mut region,
                    &mut knowledge,
                    RegionLimits::default(),
                    &cancellation,
                )
            } else {
                index.narrow_known(
                    theory,
                    None,
                    &mut region,
                    &mut knowledge,
                    RegionLimits::default(),
                    &cancellation,
                )
            }
            .unwrap();
            (result, region)
        };
        // This includes the complete work receipt and the region's split
        // preference, not only satisfaction of the final atom decisions.
        assert_eq!(close(&original), close(&reordered), "region {code}");
    }
}

#[test]
fn chain_positions_preserve_original_closure() {
    compare_closures(&dissolved_chains(), None);
}

#[test]
fn chain_positions_preserve_frozen_closure() {
    let theory = dissolved_chains();
    let mut workspace = EvaluationWorkspace::default();
    let cancellation = Cancellation::default();
    for mask in 0..16 {
        let candidate =
            Interpretation::new(&theory, (0..4).filter(|atom| mask & (1 << atom) != 0)).unwrap();
        let evaluation = workspace.evaluate(&candidate, EvaluationLimits::default(), &cancellation);
        compare_closures(&theory, Some(evaluation.result.unwrap().truth()));
    }
}

#[test]
fn compact_chain_links_reduce_retained_storage() {
    let atoms = 4096;
    let mut nodes: Vec<_> = (0..atoms).map(Node::Atom).collect();
    for first in (0..atoms).step_by(4) {
        nodes.push(Node::Or(first, first + 1));
        nodes.push(Node::Or(nodes.len() - 1, first + 2));
        nodes.push(Node::Or(nodes.len() - 1, first + 3));
    }
    let theory = Theory::new(atoms, nodes, vec![], AdmissionLimits::default()).unwrap();
    let narrower = Narrower::new(&theory);
    let compact = &narrower.chain_of;
    let mut previous = vec![None; compact.len()];
    for (decoded, link) in previous.iter_mut().zip(compact) {
        *decoded = link.map(chain_position);
    }
    assert_eq!(previous.iter().filter(|link| link.is_some()).count(), 1024);
    assert_eq!(size_of::<Option<NonZeroUsize>>(), size_of::<usize>());
    let previous_bytes = size_of_val(&previous) as u128
        + previous.capacity() as u128 * size_of::<Option<usize>>() as u128;
    let compact_bytes = size_of_val(compact) as u128
        + compact.capacity() as u128 * size_of::<Option<NonZeroUsize>>() as u128;
    assert!(compact_bytes < previous_bytes);
    // These are retained map capacities and headers, excluding other index
    // fields, construction scratch, Knowledge, allocator bookkeeping and RSS.
    println!(
        "chain_links nodes={} chains={} previous_capacity={} compact_capacity={} \
         previous_bytes={previous_bytes} compact_bytes={compact_bytes} saved_bytes={}",
        compact.len(),
        narrower.chains.len(),
        previous.capacity(),
        compact.capacity(),
        previous_bytes - compact_bytes
    );
}
