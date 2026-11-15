//! Private semantic prototype: checked mandatory supports and forbidden pairs.
//!
//! This test-only carrier has no production admission or resource contract.
//! Source reads use existing checked readers; packed image tests use existing
//! Words operations. Test fixture allocations are not runtime memory receipts.

mod chain;
mod root_integration;
mod sparse;

use zetesis_algebra::{Cancellation, Meter, Stop, words::Words};

use crate::{
    EvaluationLimits, EvaluationWorkspace, FormulaTransaction, Narrowing, NodeView, Region, Theory,
    Threshold,
    regions::arithmetic::{
        budget::{Budget, BudgetError},
        cardinality_source, source,
        storage::Storage,
        uniform::BoundSide,
    },
    test_support::{constructed_theory, push, world},
};

struct Images {
    owner: Theory,
    groups: Vec<Words<u64>>,
    forbidden: Vec<Words<u64>>,
}

impl Images {
    fn prepare(owner: &Theory) -> Self {
        let cancellation = Cancellation::default();
        let mut work = Meter::local(u64::MAX);
        let mut budget = Budget::new(&mut work, u64::MAX, usize::MAX, &cancellation);
        let mut storage = Storage::default();
        let width = owner.atom_count().div_ceil(64);
        let mut groups = Vec::new();
        let mut forbidden: Vec<_> = (0..owner.atom_count())
            .map(|_| Words::new(vec![0_u64; width]))
            .collect();
        for ordinal in 0..owner.roots().len() {
            let mut proposed =
                cardinality_source::read_root(owner, ordinal, &mut storage, &mut budget).unwrap();
            for bound in proposed.drain(..) {
                assert!(
                    cardinality_source::check(owner, owner, &bound, &mut storage, &mut budget)
                        .unwrap()
                );
                if bound.side == BoundSide::Lower && bound.value >= 1 && bound.guard.is_empty() {
                    // Bound storage pads to a 128-bit boundary. The image carrier
                    // has exactly Region's mask width; source checking authenticates
                    // the full support, including zero padding, before this copy.
                    groups.push(Words::new(bound.support[..width].to_vec()));
                }
                cardinality_source::release(bound, &mut storage).unwrap();
            }
            storage.release(proposed).unwrap();
            let Some(pair) = source::clause(owner, ordinal, &mut storage, &mut budget).unwrap()
            else {
                continue;
            };
            if pair.support.len != 2 || pair.forbidden != 0b11 {
                continue;
            }
            // This is an original root read, never a frozen probe table. Reread
            // the authenticated owner before using the proposal as pair evidence.
            let checked = source::clause(owner, pair.root, &mut storage, &mut budget)
                .unwrap()
                .unwrap();
            assert_eq!(pair.root, checked.root);
            assert_eq!(pair.support.len, checked.support.len);
            assert_eq!(pair.support.atoms, checked.support.atoms);
            assert_eq!(pair.forbidden, checked.forbidden);
            let left = pair.support.atoms[0];
            let right = pair.support.atoms[1];
            forbidden[left].insert(right);
            forbidden[right].insert(left);
        }
        assert_eq!(storage.bytes(), 0);
        Self {
            owner: owner.clone(),
            groups,
            forbidden,
        }
    }

    fn narrow(&self, owner: &Theory, region: &mut Region) -> Result<Narrowing, BudgetError> {
        if !self.owner.same_instance(owner) || region.len() != owner.atom_count() {
            return Err(BudgetError::Stopped(Stop::InvalidProgram));
        }
        let cancellation = Cancellation::default();
        let mut work = Meter::local(u64::MAX);
        let mut budget = Budget::new(&mut work, u64::MAX, usize::MAX, &cancellation);
        let mut possible = Words::new(vec![0_u64; region.mask_words()]);
        let mut changed = false;
        loop {
            let mut round_changed = false;
            for group in &self.groups {
                let (_, cut) = region.decision_words();
                for ((destination, &members), &cut) in
                    possible.iter_mut().zip(group.iter()).zip(cut)
                {
                    budget.charge(3)?;
                    *destination = members & !cut;
                }
                if possible.next_from_with(0, || budget.charge(1))?.is_none() {
                    return Ok(Narrowing::Refuted);
                }
                for (target, forbidden) in self.forbidden.iter().enumerate() {
                    if region.is_cut(target) {
                        continue;
                    }
                    if possible
                        .is_subset_of(Words::new(&forbidden[..]), &mut || budget.charge(1))?
                    {
                        if !region.cut(target) {
                            return Ok(Narrowing::Refuted);
                        }
                        round_changed = true;
                    }
                }
            }
            changed |= round_changed;
            if !round_changed {
                return Ok(Narrowing::Fixed { changed });
            }
        }
    }
}

fn threshold(nodes: &mut FormulaTransaction<'_>, members: &[usize], minimum: i128) -> usize {
    push(
        nodes,
        NodeView::Threshold(Threshold {
            conditions: members,
            weights: &vec![1; members.len()],
            bound: minimum,
        }),
    )
}

fn pair_constraint(
    nodes: &mut FormulaTransaction<'_>,
    falsum: usize,
    pair: (usize, usize),
) -> usize {
    let body = push(nodes, NodeView::And(&[pair.0, pair.1]));
    push(nodes, NodeView::Implies(body, falsum))
}

fn fixture(atoms: usize, groups: &[Vec<usize>], pairs: &[(usize, usize)]) -> Theory {
    constructed_theory(atoms, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let mut roots: Vec<_> = groups
            .iter()
            .map(|members| threshold(nodes, members, 1))
            .collect();
        roots.extend(
            pairs
                .iter()
                .map(|&pair| pair_constraint(nodes, falsum, pair)),
        );
        roots
    })
}

#[test]
fn checked_mandatory_sources_remove_an_unsupported_target() {
    let owner = fixture(3, &[vec![0, 1]], &[(0, 2), (1, 2)]);
    let images = Images::prepare(&owner);
    assert_eq!(images.groups.len(), 1);
    let mut region = Region::all_open(3);
    assert_eq!(
        images.narrow(&owner, &mut region).unwrap(),
        Narrowing::Fixed { changed: true }
    );
    assert!(region.is_cut(2));
    assert_eq!(region.open().collect::<Vec<_>>(), [0, 1]);
}

#[test]
fn a_held_source_remains_a_possible_image_witness() {
    let owner = fixture(3, &[vec![0, 1]], &[(0, 2)]);
    let images = Images::prepare(&owner);
    let mut region = Region::all_open(3);
    assert!(region.cut(0));
    assert!(region.hold(1));
    assert_eq!(
        images.narrow(&owner, &mut region).unwrap(),
        Narrowing::Fixed { changed: false }
    );
    assert!(region.is_open(2));
}

#[test]
fn an_empty_mandatory_group_or_unsupported_held_target_refutes() {
    let owner = fixture(3, &[vec![0, 1]], &[(0, 2), (1, 2)]);
    let images = Images::prepare(&owner);
    let mut empty = Region::all_open(3);
    assert!(empty.cut(0));
    assert!(empty.cut(1));
    assert_eq!(
        images.narrow(&owner, &mut empty).unwrap(),
        Narrowing::Refuted
    );
    let mut held = Region::all_open(3);
    assert!(held.hold(2));
    assert_eq!(
        images.narrow(&owner, &mut held).unwrap(),
        Narrowing::Refuted
    );
}

#[test]
fn upper_only_and_guarded_groups_supply_no_unconditional_witness() {
    for guarded in [false, true] {
        let owner = constructed_theory(4, |nodes| {
            let falsum = push(nodes, NodeView::False);
            let row = threshold(nodes, &[0, 1], if guarded { 1 } else { 2 });
            let negative = push(nodes, NodeView::Implies(row, falsum));
            let root = if guarded {
                let body = push(nodes, NodeView::And(&[3, negative]));
                push(nodes, NodeView::Implies(body, falsum))
            } else {
                negative
            };
            vec![
                root,
                pair_constraint(nodes, falsum, (0, 2)),
                pair_constraint(nodes, falsum, (1, 2)),
            ]
        });
        let images = Images::prepare(&owner);
        assert!(images.groups.is_empty());
        let mut region = Region::all_open(4);
        assert!(region.cut(0));
        assert!(region.cut(1));
        assert!(region.cut(3));
        assert!(region.hold(2));
        assert_eq!(
            images.narrow(&owner, &mut region).unwrap(),
            Narrowing::Fixed { changed: false },
            "guarded={guarded}"
        );
        let candidate = world(&owner, 1 << 2);
        assert!(
            EvaluationWorkspace::default()
                .evaluate(
                    &candidate,
                    EvaluationLimits::default(),
                    &Cancellation::default()
                )
                .result
                .unwrap()
                .is_model()
        );
    }
}

#[test]
fn padded_supports_and_exact_owner_identity_are_preserved() {
    let owner = fixture(130, &[vec![0, 64, 129]], &[(0, 128), (64, 128), (129, 128)]);
    let images = Images::prepare(&owner);
    assert_eq!(images.groups[0].len(), 3);
    let mut region = Region::all_open(130);
    assert_eq!(
        images.narrow(&owner, &mut region).unwrap(),
        Narrowing::Fixed { changed: true }
    );
    assert!(region.is_cut(128));
    assert_eq!(region.open().count(), 129);
    let other = fixture(130, &[vec![0, 64, 129]], &[(0, 128), (64, 128), (129, 128)]);
    assert!(matches!(
        images.narrow(&other, &mut region),
        Err(BudgetError::Stopped(Stop::InvalidProgram))
    ));
}

fn contains(region: &Region, interpretation: usize) -> bool {
    (0..region.len()).all(|atom| match region.decision(atom) {
        Some(value) => (interpretation & (1 << atom) != 0) == value,
        None => true,
    })
}

fn scalar(groups: &[Vec<usize>], pairs: &[(usize, usize)], region: &mut Region) -> Narrowing {
    let mut changed = false;
    loop {
        let mut round_changed = false;
        for group in groups {
            let possible: Vec<_> = group
                .iter()
                .copied()
                .filter(|&a| !region.is_cut(a))
                .collect();
            if possible.is_empty() {
                return Narrowing::Refuted;
            }
            for target in 0..region.len() {
                if !region.is_cut(target)
                    && possible.iter().all(|&source| {
                        pairs.contains(&(source, target)) || pairs.contains(&(target, source))
                    })
                {
                    if !region.cut(target) {
                        return Narrowing::Refuted;
                    }
                    round_changed = true;
                }
            }
        }
        changed |= round_changed;
        if !round_changed {
            return Narrowing::Fixed { changed };
        }
    }
}

#[test]
fn every_small_pair_relation_preserves_all_original_models() {
    // Groups require at least one atom, not exactly one; overlapping support
    // is covered separately below. No independence/functionality assumption.
    let groups = [vec![0, 1], vec![2, 3]];
    let possible_pairs = [(0, 2), (0, 3), (1, 2), (1, 3)];
    for relation in 0..16 {
        let pairs: Vec<_> = possible_pairs
            .iter()
            .enumerate()
            .filter_map(|(index, &pair)| (relation & (1 << index) != 0).then_some(pair))
            .collect();
        let owner = fixture(4, &groups, &pairs);
        let images = Images::prepare(&owner);
        let mut evaluation = EvaluationWorkspace::default();
        let models: Vec<_> = (0..16)
            .filter(|&mask| {
                let candidate = world(&owner, mask);
                evaluation
                    .evaluate(
                        &candidate,
                        EvaluationLimits::default(),
                        &Cancellation::default(),
                    )
                    .result
                    .unwrap()
                    .is_model()
            })
            .collect();
        for mut code in 0..81 {
            let mut before = Region::all_open(4);
            for atom in 0..4 {
                match code % 3 {
                    1 => {
                        assert!(before.hold(atom));
                    }
                    2 => {
                        assert!(before.cut(atom));
                    }
                    _ => {}
                }
                code /= 3;
            }
            let mut packed = before.clone();
            let actual = images.narrow(&owner, &mut packed).unwrap();
            let mut reference = before.clone();
            assert_eq!(actual, scalar(&groups, &pairs, &mut reference));
            assert_eq!(packed.len(), reference.len());
            assert_eq!(packed.decision_words(), reference.decision_words());
            for &model in &models {
                if contains(&before, model) {
                    assert_ne!(
                        actual,
                        Narrowing::Refuted,
                        "relation={relation}, model={model}"
                    );
                    assert!(
                        contains(&packed, model),
                        "relation={relation}, model={model}"
                    );
                }
            }
        }
    }
}

#[test]
fn overlapping_mandatory_groups_do_not_assume_independent_values() {
    let groups = [vec![0, 1], vec![1, 2]];
    let owner = fixture(4, &groups, &[(0, 3), (1, 3)]);
    let images = Images::prepare(&owner);
    let mut region = Region::all_open(4);
    assert_eq!(
        images.narrow(&owner, &mut region).unwrap(),
        Narrowing::Fixed { changed: true }
    );
    assert!(region.is_cut(3));
    let mut evaluation = EvaluationWorkspace::default();
    for mask in 0..16 {
        let candidate = world(&owner, mask);
        if evaluation
            .evaluate(
                &candidate,
                EvaluationLimits::default(),
                &Cancellation::default(),
            )
            .result
            .unwrap()
            .is_model()
        {
            assert!(contains(&region, mask));
        }
    }
}
