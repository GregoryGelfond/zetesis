//! Packed storage preserves the closure's logical tails and stopping boundary.

use zetesis_algebra::{Cancellation, Class, Stop};

use crate::{
    AdmissionLimits, FormulaParts, FrozenSubject, Knowledge, Narrower, NarrowingScratch, Node,
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
            .result
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

fn assert_mask_padding<C: Class>(
    theory: &Theory,
    index: &Narrower,
    source: &Knowledge<C>,
    narrow: impl FnOnce(&mut Region, &mut Knowledge<C>),
) {
    let mut state = source.clone();
    let mut region = Region::all_open(theory.atom_count());
    narrow(&mut region, &mut state);
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

#[test]
fn narrowing_keeps_mask_padding_clear() {
    let theory = implication_chain(65);
    let index = Narrower::new(&theory);
    let compact = index.knowledge();
    let frozen_compact = index.frozen_knowledge();
    let truth = vec![true; theory.view().len()];
    for (source, frozen_source) in [
        (compact.clone(), frozen_compact.clone()),
        (native(&compact), native(&frozen_compact)),
    ] {
        assert_mask_padding(&theory, &index, &source, |region, state| {
            index
                .narrow_known(
                    OriginalSubject::new(&theory, None),
                    region,
                    state,
                    &mut NarrowingScratch::default(),
                    RegionLimits::default(),
                    &Cancellation::default(),
                )
                .result
                .unwrap();
        });
        assert_mask_padding(&theory, &index, &frozen_source, |region, state| {
            index
                .narrow_frozen_known(
                    FrozenSubject::new(&theory, &truth),
                    region,
                    state,
                    &mut NarrowingScratch::default(),
                    RegionLimits::default(),
                    &Cancellation::default(),
                )
                .result
                .unwrap();
        });
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
        assert_eq!(result.result, Err(Stop::Cancelled));
        assert_eq!(region, before);
        same_knowledge(&copied, &source);
    }
}

#[test]
fn known_views_keep_the_same_disjoint_spans() {
    use super::super::{Counters, Known};

    let mut known = Known::<u16>::empty(130, 65, Counters::zeros(65));
    let pointer = known.masks.words.as_ptr();
    let expected = known.masks.slices().map(|span| (span.as_ptr(), span.len()));
    {
        let mut view = known.borrow();
        let spans = [
            &mut view.sure,
            &mut view.never,
            &mut view.atom_sure,
            &mut view.atom_never,
            &mut view.seen,
            &mut view.witnessed,
        ];
        for (index, span) in spans.into_iter().enumerate() {
            assert_eq!((span.as_ptr(), span.len()), expected[index]);
            span.fill(1 << index);
        }
    }
    assert_eq!(known.masks.words.as_ptr(), pointer);
    for (index, span) in known.masks.slices().into_iter().enumerate() {
        assert!(span.iter().all(|&word| word == 1 << index));
    }
}

#[test]
fn known_view_keeps_the_original_borrowed_record_layout() {
    use super::super::{Counters, KnownMut, support, thresholds};
    use std::mem::{align_of, offset_of, size_of};

    struct Original<'a> {
        sure: &'a mut [u64],
        never: &'a mut [u64],
        atom_sure: &'a mut [u64],
        atom_never: &'a mut [u64],
        seen: &'a mut [u64],
        witnessed: &'a mut [u64],
        neutral_operands: &'a mut Counters<u16>,
        unknown: &'a mut Counters<u16>,
        thresholds: &'a mut [thresholds::State],
        seeded: &'a mut bool,
        founded_closed: &'a mut bool,
        support_progress: &'a mut support::Progress,
    }
    assert_eq!(size_of::<KnownMut<'_, u16>>(), size_of::<Original<'_>>());
    assert_eq!(align_of::<KnownMut<'_, u16>>(), align_of::<Original<'_>>());
    macro_rules! offset {
        ($field:ident) => {
            assert_eq!(
                offset_of!(KnownMut<'_, u16>, $field),
                offset_of!(Original<'_>, $field)
            );
        };
    }
    offset!(sure);
    offset!(never);
    offset!(atom_sure);
    offset!(atom_never);
    offset!(seen);
    offset!(witnessed);
    offset!(neutral_operands);
    offset!(unknown);
    offset!(thresholds);
    offset!(seeded);
    offset!(founded_closed);
    offset!(support_progress);
}
