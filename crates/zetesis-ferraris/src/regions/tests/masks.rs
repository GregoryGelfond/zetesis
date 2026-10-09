//! Packed storage preserves the closure's logical tails and stopping boundary.

use zetesis_cpu::{Cancellation, Stop};

use crate::{FrozenSubject, Narrower, NarrowingScratch, OriginalSubject, Region, RegionLimits};

use super::super::Width;
use super::{
    counters::{native, same_knowledge},
    implication_chain,
};

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
                Width::Compact(known) => known.masks.slices(),
                Width::Native(known) => known.masks.slices(),
            };
            let lengths = [theory.view().len(), theory.view().len(), 65, 65, 65];
            for (mask, length) in masks.into_iter().zip(lengths) {
                assert_eq!(mask.len(), length.div_ceil(64));
                let valid = (1u64 << (length % 64)) - 1;
                assert_eq!(mask.last().unwrap() & !valid, 0);
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
