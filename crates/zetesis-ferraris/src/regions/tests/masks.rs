//! Packed storage preserves the closure's logical tails and stopping boundary.

use zetesis_cpu::{Cancellation, Stop};

use crate::{
    AdmissionLimits, FormulaParts, FrozenSubject, Narrower, NarrowingScratch, Node,
    OriginalSubject, Region, RegionLimits, Theory,
};

use super::super::Width;
use super::{
    counters::{native, same_knowledge},
    implication_chain,
};

#[test]
fn processed_witnesses_preserve_word_boundaries() {
    for chains in [1, 63, 64, 65, 130] {
        let atoms = 2 * chains;
        let mut nodes: Vec<_> = (0..atoms).map(Node::atom).collect();
        nodes.extend((0..chains).map(|chain| {
            let operands = [2 * chain, 2 * chain + 1];
            if chain % 2 == 0 {
                Node::or_pair(operands)
            } else {
                Node::and_pair(operands)
            }
        }));
        let theory = Theory::new(
            atoms,
            FormulaParts::new(nodes, vec![]).unwrap(),
            vec![],
            AdmissionLimits::default(),
        )
        .unwrap();
        let index = Narrower::new(&theory);
        assert_eq!(index.chains.len(), chains);
        let mut region = Region::all_open(atoms);
        for atom in 0..atoms {
            if (atom / 2) % 2 == 0 {
                assert!(region.hold(atom));
            } else {
                assert!(region.cut(atom));
            }
        }
        let expected = region.clone();
        let mut knowledge = index.knowledge();
        index
            .narrow_known(
                OriginalSubject::new(&theory, None),
                &mut region,
                &mut knowledge,
                &mut NarrowingScratch::default(),
                RegionLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(region, expected);
        let known = super::compact(&knowledge);
        let witnesses = known.masks.slices()[5];
        assert_eq!(witnesses.len(), chains.div_ceil(64));
        for chain in 0..chains {
            assert!(super::super::bit(witnesses, chain));
            assert_eq!(known.neutral_operands.get(chain), 0);
        }
        if chains % 64 != 0 {
            assert_eq!(witnesses.last().unwrap() >> (chains % 64), 0);
        }
        same_knowledge(&knowledge.clone(), &knowledge);
    }
}

#[test]
fn narrowing_keeps_mask_padding_clear() {
    let theory = implication_chain(65);
    let index = Narrower::new(&theory);
    let compact = index.knowledge();
    let truth = vec![true; theory.view().len()];
    for source in [compact.clone(), native(&compact)] {
        for frozen in [false, true] {
            let mut state = source.clone();
            let mut region = Region::all_open(theory.atom_count());
            let mut scratch = NarrowingScratch::default();
            if frozen {
                index
                    .narrow_frozen_known(
                        FrozenSubject::new(&theory, &truth),
                        &mut region,
                        &mut state,
                        &mut scratch,
                        RegionLimits::default(),
                        &Cancellation::default(),
                    )
                    .unwrap();
            } else {
                index
                    .narrow_known(
                        OriginalSubject::new(&theory, None),
                        &mut region,
                        &mut state,
                        &mut scratch,
                        RegionLimits::default(),
                        &Cancellation::default(),
                    )
                    .unwrap();
            }
            let mut copied = source.clone();
            copied.clone_from(&state);
            let masks = match &copied.width {
                Width::Compact16(known) => known.masks.slices(),
                Width::Compact32(known) => known.masks.slices(),
                Width::Native(known) => known.masks.slices(),
            };
            let lengths = [
                theory.view().len(),
                theory.view().len(),
                65,
                65,
                65,
                index.chains.len(),
            ];
            for (mask, length) in masks.into_iter().zip(lengths) {
                assert_eq!(mask.len(), length.div_ceil(64));
                if length % 64 != 0 {
                    let valid = (1u64 << (length % 64)) - 1;
                    assert_eq!(mask.last().unwrap() & !valid, 0);
                }
            }
        }
    }
}

#[test]
fn cancelled_narrowing_preserves_copied_knowledge() {
    let theory = implication_chain(65);
    let index = Narrower::new(&theory);
    let compact = index.knowledge();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    for source in [compact.clone(), native(&compact)] {
        let mut copied = source.clone();
        let mut region = Region::all_open(theory.atom_count());
        assert!(region.hold(64));
        let before = region.clone();
        let result = index.narrow_known(
            OriginalSubject::new(&theory, None),
            &mut region,
            &mut copied,
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &cancellation,
        );
        assert_eq!(result, Err(Stop::Cancelled));
        assert_eq!(region, before);
        same_knowledge(&copied, &source);
    }
}
