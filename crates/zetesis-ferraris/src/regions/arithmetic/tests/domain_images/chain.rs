//! A finite mandatory chain whose values must increase at each edge.
//!
//! This uses only the generic source groups and pair constraints accepted by
//! the parent prototype. There is no at-most-one premise, source-program name,
//! grounding transformation or production execution policy.

use super::{Images, contains, fixture};
use crate::{
    EvaluationLimits, EvaluationWorkspace, Narrower, Narrowing, NarrowingScratch, Region, Theory,
    test_support::{close, world},
};
use zetesis_algebra::Cancellation;

fn chain(length: usize, values: usize) -> Theory {
    let groups: Vec<_> = (0..length)
        .map(|position| (position * values..(position + 1) * values).collect())
        .collect();
    let mut forbidden = Vec::new();
    for position in 0..length - 1 {
        for left in 0..values {
            for right in 0..=left {
                forbidden.push((position * values + left, (position + 1) * values + right));
            }
        }
    }
    fixture(length * values, &groups, &forbidden)
}

fn boolean_root(owner: &Theory, region: &mut Region) -> Narrowing {
    let index = Narrower::new(owner);
    let mut knowledge = index.knowledge();
    close(
        owner,
        &index,
        region,
        &mut knowledge,
        &mut NarrowingScratch::default(),
    )
    .0
}

fn all_original_models(owner: &Theory) -> Vec<usize> {
    let mut evaluation = EvaluationWorkspace::default();
    (0..1 << owner.atom_count())
        .filter(|&mask| {
            evaluation
                .evaluate(
                    &world(owner, mask),
                    EvaluationLimits::default(),
                    &Cancellation::default(),
                )
                .result
                .unwrap()
                .is_model()
        })
        .collect()
}

#[test]
fn mandatory_images_expose_the_unique_chain_assignment() {
    for length in 2..=3 {
        let values = length;
        let owner = chain(length, values);
        let images = Images::prepare(&owner);
        assert_eq!(images.groups.len(), length);
        let expected: usize = (0..length)
            .map(|position| 1 << (position * values + position))
            .sum();
        let models = all_original_models(&owner);
        assert_eq!(models, [expected]);

        let mut region = Region::all_open(owner.atom_count());
        assert!(matches!(
            boolean_root(&owner, &mut region),
            Narrowing::Fixed { .. }
        ));
        assert!(region.open().next().is_some());
        assert!(models.iter().all(|&model| contains(&region, model)));

        assert_eq!(
            images.narrow(&owner, &mut region).unwrap(),
            Narrowing::Fixed { changed: true }
        );
        assert!(models.iter().all(|&model| contains(&region, model)));
        for position in 0..length {
            let possible: Vec<_> = (position * values..(position + 1) * values)
                .filter(|&atom| !region.is_cut(atom))
                .collect();
            assert_eq!(possible, [position * values + position]);
        }

        // Image exclusion supplies singleton supports. The existing original
        // formula narrower, rather than a new holding rule, forces their members.
        assert_eq!(
            boolean_root(&owner, &mut region),
            Narrowing::Fixed { changed: true }
        );
        assert!(region.open().next().is_none());
        assert!(contains(&region, expected));
    }
}

#[test]
fn mandatory_images_refute_a_chain_one_value_too_short() {
    let owner = chain(3, 2);
    assert!(all_original_models(&owner).is_empty());
    let images = Images::prepare(&owner);
    let mut region = Region::all_open(owner.atom_count());
    assert!(matches!(
        boolean_root(&owner, &mut region),
        Narrowing::Fixed { .. }
    ));
    assert!(region.open().next().is_some());
    assert_eq!(
        images.narrow(&owner, &mut region).unwrap(),
        Narrowing::Refuted
    );
}
