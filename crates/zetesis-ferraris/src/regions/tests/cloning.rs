//! Storage reuse copies complete knowledge; a retired destination supplies no truth.

use zetesis_cpu::Cancellation;

use crate::{
    AdmissionLimits, FormulaParts, Knowledge, Narrower, Narrowing, NarrowingScratch, Region,
    RegionLimits, Theory,
};

use super::super::{Known, Width};
use super::{
    counters::{compact32, native, same_knowledge},
    implication_chain, shared_occurrences,
};

fn narrow(
    theory: &Theory,
    narrower: &Narrower,
    region: &mut Region,
    knowledge: &mut Knowledge,
    max_work: u64,
) -> Result<(Narrowing, crate::NarrowingStatistics), zetesis_cpu::Stop> {
    narrower.narrow_known(
        crate::OriginalSubject::new(theory, None),
        region,
        knowledge,
        &mut NarrowingScratch::default(),
        RegionLimits { max_work },
        &Cancellation::default(),
    )
}

fn refuted(theory: &Theory, narrower: &Narrower) -> Knowledge {
    let mut region = Region::all_open(theory.atom_count());
    assert!(region.cut(0));
    assert!(region.cut(1));
    let mut knowledge = narrower.knowledge();
    assert_eq!(
        narrow(theory, narrower, &mut region, &mut knowledge, u64::MAX)
            .unwrap()
            .0,
        Narrowing::Refuted,
    );
    knowledge
}

fn flag_addresses(knowledge: &Knowledge) -> [*const u64; 6] {
    fn addresses<C>(known: &Known<C>) -> [*const u64; 6] {
        known.masks.slices().map(<[u64]>::as_ptr)
    }
    match &knowledge.width {
        Width::Compact16(known) => addresses(known),
        Width::Compact32(known) => addresses(known),
        Width::Native(known) => addresses(known),
    }
}

#[test]
fn compatible_copies_reuse_independent_flag_storage() {
    let theory = shared_occurrences();
    let narrower = Narrower::new(&theory);
    let compact = narrower.knowledge();
    for source in [compact.clone(), compact32(&compact), native(&compact)] {
        let mut destination = source.clone();
        let before = flag_addresses(&destination);
        let source_addresses = flag_addresses(&source);
        destination.clone_from(&source);
        assert_eq!(flag_addresses(&destination), before);
        for (destination, source) in before.into_iter().zip(source_addresses) {
            assert_ne!(destination, source);
        }
        same_knowledge(&destination, &source);
        assert_eq!(destination.retained_bytes(), source.retained_bytes());
    }
}

#[test]
fn overwriting_refuted_knowledge_restores_fresh_prefixes() {
    let theory = shared_occurrences();
    let narrower = Narrower::new(&theory);
    let retired = refuted(&theory, &narrower);
    let fresh = narrower.knowledge();
    for source in [fresh.clone(), compact32(&fresh), native(&fresh)] {
        for max_work in [0, 1, 5, u64::MAX] {
            let mut reused = match &source.width {
                Width::Compact16(_) => retired.clone(),
                Width::Compact32(_) => compact32(&retired),
                Width::Native(_) => native(&retired),
            };
            reused.clone_from(&source);
            same_knowledge(&reused, &source);
            let mut expected = source.clone();
            let mut reused_region = Region::all_open(theory.atom_count());
            let mut expected_region = reused_region.clone();
            assert_eq!(
                narrow(
                    &theory,
                    &narrower,
                    &mut reused_region,
                    &mut reused,
                    max_work
                ),
                narrow(
                    &theory,
                    &narrower,
                    &mut expected_region,
                    &mut expected,
                    max_work
                ),
            );
            assert_eq!(reused_region, expected_region);
            same_knowledge(&reused, &expected);
        }
    }
}

#[test]
fn reused_siblings_start_from_the_same_parent() {
    let theory = shared_occurrences();
    let narrower = Narrower::new(&theory);
    let mut parent = narrower.knowledge();
    let mut region = Region::all_open(theory.atom_count());
    narrow(&theory, &narrower, &mut region, &mut parent, u64::MAX).unwrap();
    let (cut, held) = region.split(0);
    let saved_parent = parent.clone();
    let mut retired = refuted(&theory, &narrower);
    for mut child in [cut, held] {
        retired.clone_from(&parent);
        let mut expected = parent.clone();
        let mut expected_region = child.clone();
        assert_eq!(
            narrow(&theory, &narrower, &mut child, &mut retired, u64::MAX),
            narrow(
                &theory,
                &narrower,
                &mut expected_region,
                &mut expected,
                u64::MAX
            ),
        );
        assert_eq!(child, expected_region);
        same_knowledge(&retired, &expected);
        same_knowledge(&parent, &saved_parent);
    }
}

#[test]
fn changing_width_or_shape_copies_the_complete_state() {
    let empty = Theory::new(
        0,
        FormulaParts::new(vec![], vec![]).unwrap(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    let theories = [empty, shared_occurrences(), implication_chain(65)];
    let sources: Vec<_> = theories
        .iter()
        .flat_map(|theory| {
            let compact = Narrower::new(theory).knowledge();
            let middle = compact32(&compact);
            let wide = native(&compact);
            [compact, middle, wide]
        })
        .collect();
    for source in &sources {
        for previous in &sources {
            let mut copied = previous.clone();
            copied.clone_from(source);
            assert_eq!(
                std::mem::discriminant(&copied.width),
                std::mem::discriminant(&source.width),
            );
            same_knowledge(&copied, source);
            assert_eq!(copied.retained_bytes(), source.retained_bytes());
        }
    }
}
