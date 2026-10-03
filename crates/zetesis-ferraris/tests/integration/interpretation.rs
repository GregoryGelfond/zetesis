//! Independent Boolean populations specify ordered membership and packed export.

use proptest::prelude::*;
use zetesis_ferraris::{AdmissionError, AdmissionLimits, Interpretation, Theory};

fn theory(atoms: usize) -> Theory {
    Theory::new(atoms, vec![], vec![], AdmissionLimits::default()).unwrap()
}

#[test]
fn invalid_atom_leaves_iterator_suffix_unconsumed() {
    let theory = theory(65);
    let mut atoms = [0, 64, 65, 32].into_iter();
    assert_eq!(
        Interpretation::new(&theory, atoms.by_ref()).unwrap_err(),
        AdmissionError::Atom
    );
    assert_eq!(atoms.next(), Some(32));
    assert_eq!(atoms.next(), None);
}

#[test]
fn packed_words_retain_exact_theory_identity() {
    let theory = theory(65);
    let equal = Theory::new(65, vec![], vec![], AdmissionLimits::default()).unwrap();
    let candidate = Interpretation::new(&theory, [0, 32, 64]).unwrap();
    let mut words = candidate.words32();
    assert!(theory.same_instance(words.theory()));
    assert!(!equal.same_instance(words.theory()));
    assert_eq!(words.next(), Some(1));
    let copy = words.clone();
    assert!(theory.same_instance(copy.theory()));
    assert_eq!(copy.collect::<Vec<_>>(), [1, 1]);
    assert_eq!(words.collect::<Vec<_>>(), [1, 1]);
}

#[test]
fn packed_iteration_has_an_exact_remaining_length() {
    for atoms in [0, 1, 31, 32, 33, 63, 64, 65, 127, 128, 129] {
        let theory = theory(atoms);
        let candidate = Interpretation::new(&theory, 0..atoms).unwrap();
        let mut words = candidate.words32();
        let length = atoms.div_ceil(32);
        for remaining in (1..=length).rev() {
            assert_eq!(words.len(), remaining);
            assert_eq!(words.size_hint(), (remaining, Some(remaining)));
            assert!(words.next().is_some());
        }
        assert_eq!(words.len(), 0);
        assert_eq!(words.size_hint(), (0, Some(0)));
        assert_eq!(words.next(), None);
        assert_eq!(words.next(), None);
    }
}

#[test]
fn packed_export_preserves_distinct_numeric_halves() {
    let theory = theory(97);
    let candidate = Interpretation::new(&theory, [0, 2, 31, 33, 63, 64, 96]).unwrap();
    assert_eq!(
        candidate.words32().collect::<Vec<_>>(),
        [0x8000_0005, 0x8000_0002, 1, 1]
    );
}

#[test]
fn empty_words_do_not_change_selected_atom_order() {
    let theory = theory(257);
    let candidate = Interpretation::new(&theory, [256, 0, 256, 192, 63, 128]).unwrap();
    assert_eq!(
        candidate.atoms().collect::<Vec<_>>(),
        [0, 63, 128, 192, 256]
    );
}

proptest! {
    #[test]
    fn packed_export_denotes_the_input_population(
        selected in prop::collection::vec(any::<bool>(), 0..2049)
    ) {
        let theory = theory(selected.len());
        let expected: Vec<_> = selected.iter().enumerate()
            .filter_map(|(atom, &present)| present.then_some(atom)).collect();
        let candidate = Interpretation::new(&theory, expected.iter().copied().rev()).unwrap();
        prop_assert_eq!(candidate.atoms().collect::<Vec<_>>(), expected);
        let words: Vec<_> = candidate.words32().collect();
        prop_assert_eq!(words.len(), selected.len().div_ceil(32));
        for (index, word) in words.iter().enumerate() {
            for bit in 0..32 {
                let atom = index * 32 + bit;
                prop_assert_eq!(word & (1 << bit) != 0, selected.get(atom).copied().unwrap_or(false));
            }
        }
    }
}

#[test]
fn fallible_clone_preserves_packed_words() {
    for atoms in [0, 1, 31, 32, 33, 63, 64, 65, 127, 128, 129] {
        let owner = theory(atoms);
        for selected in [
            Vec::new(),
            (0..atoms).collect(),
            (0..atoms).step_by(3).collect(),
        ] {
            let source = Interpretation::new(&owner, selected.iter().copied()).unwrap();
            let before = source.words32().collect::<Vec<_>>();
            let copied = source.try_clone().unwrap();
            assert_eq!(copied.atoms().collect::<Vec<_>>(), selected);
            assert_eq!(copied.words32().collect::<Vec<_>>(), before);
            if let Some(&last) = before.last() {
                let tail = atoms % 32;
                if tail != 0 {
                    assert_eq!(last >> tail, 0);
                }
            }
        }
    }
}

proptest! {
    #[test]
    fn fallible_clone_denotes_the_input_population(
        selected in prop::collection::vec(any::<bool>(), 0..2049)
    ) {
        let owner = theory(selected.len());
        let expected: Vec<_> = selected.iter().enumerate()
            .filter_map(|(atom, &present)| present.then_some(atom)).collect();
        let source = Interpretation::new(&owner, expected.iter().copied()).unwrap();
        let copied = source.try_clone().unwrap();
        prop_assert!(copied.theory().same_instance(&owner));
        prop_assert_eq!(copied.atoms().collect::<Vec<_>>(), expected.clone());
        prop_assert_eq!(source.atoms().collect::<Vec<_>>(), expected);
    }
}

#[test]
fn fallible_clone_retains_exact_owner() {
    let owner = theory(129);
    let equal = theory(129);
    let source = Interpretation::new(&owner, [0, 64, 128]).unwrap();
    let copied = source.try_clone().unwrap();
    assert!(copied.theory().same_instance(&owner));
    assert!(!copied.theory().same_instance(&equal));
}

#[test]
fn fallible_clone_leaves_its_source_unchanged() {
    let owner = theory(97);
    let source = Interpretation::new(&owner, [0, 63, 96]).unwrap();
    let before = source.words32().collect::<Vec<_>>();
    drop(source.try_clone().unwrap());
    assert_eq!(source.words32().collect::<Vec<_>>(), before);
}
