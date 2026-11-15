//! A selection shows the same atoms of a model by either traversal.

use zetesis_core::{Atom, Model, Predicate, Sign, Value};

use super::{AtomChannel, ShownAtoms, runs_compare_fewer};
use crate::{AtomSelectionLimits, OutputSelection};

const POSITIVE: Sign = Sign::Positive;
const NEGATIVE: Sign = Sign::Negative;

fn atoms(name: &str, sign: Sign, count: i32) -> impl Iterator<Item = Atom> {
    let predicate = Predicate::with_sign(name, 1, sign).unwrap();
    (0..count).map(move |value| Atom::new(predicate.clone(), vec![Value::Number(value)]).unwrap())
}

/// `each` atoms of every signed unary predicate.
fn model(predicates: &[(&str, Sign)], each: i32) -> Model {
    Model::new(
        predicates
            .iter()
            .flat_map(|&(name, sign)| atoms(name, sign, each)),
    )
    .unwrap()
}

fn selection(signatures: &[(&str, Sign)]) -> OutputSelection {
    let signatures: Vec<_> = signatures
        .iter()
        .map(|&(name, sign)| Predicate::with_sign(name, 1, sign).unwrap())
        .collect();
    OutputSelection::from_signatures(&signatures, AtomSelectionLimits::default()).unwrap()
}

fn by_runs<'a>(model: &'a Model, selection: &'a OutputSelection) -> ShownAtoms<'a> {
    ShownAtoms::Runs {
        atoms: model.atoms(),
        signatures: selection.signatures(),
        next: 0,
        run: None,
    }
}

fn by_tests<'a>(model: &'a Model, selection: &'a OutputSelection) -> ShownAtoms<'a> {
    ShownAtoms::Tested {
        atoms: model.atoms().iter(),
        selection: AtomChannel::Policy(selection),
    }
}

#[test]
fn runs_are_read_when_their_searches_compare_fewer_atoms() {
    // One signature searches a model of eight or nine atoms in at most four
    // comparisons, twice.
    assert!(!runs_compare_fewer(1, 8));
    assert!(runs_compare_fewer(1, 9));
    // Two signatures search twenty or twenty-one atoms in at most five.
    assert!(!runs_compare_fewer(2, 20));
    assert!(runs_compare_fewer(2, 21));
    // No signature needs no search; an empty model has no atom to save.
    assert!(runs_compare_fewer(0, 1));
    assert!(!runs_compare_fewer(0, 0));
    assert!(!runs_compare_fewer(usize::MAX, usize::MAX));
}

#[test]
fn a_selection_of_few_signatures_reads_its_runs() {
    let model = model(&[("p", POSITIVE), ("q", POSITIVE)], 5);
    let one = selection(&[("q", POSITIVE)]);
    let shown = ShownAtoms::new(model.atoms(), AtomChannel::Policy(&one));
    assert!(matches!(shown, ShownAtoms::Runs { .. }));
    assert_eq!(
        shown.collect::<Vec<_>>(),
        atoms("q", POSITIVE, 5).collect::<Vec<_>>()
    );
}

#[test]
fn a_prepared_selection_reads_the_runs_of_its_policy() {
    let model = model(&[("p", POSITIVE), ("q", POSITIVE)], 5);
    let one = selection(&[("p", POSITIVE)]);
    let prepared = one
        .prepare_with(model.catalog().read(), |_| Ok::<(), ()>(()))
        .unwrap();
    assert!(prepared.is_prepared());
    let shown = ShownAtoms::new(model.atoms(), AtomChannel::Prepared(&prepared));
    assert!(matches!(shown, ShownAtoms::Runs { .. }));
    assert_eq!(
        shown.collect::<Vec<_>>(),
        atoms("p", POSITIVE, 5).collect::<Vec<_>>()
    );
}

#[test]
fn a_selection_of_many_signatures_tests_each_atom() {
    let model = model(&[("p", POSITIVE), ("q", POSITIVE)], 5);
    let both = selection(&[("p", POSITIVE), ("q", POSITIVE)]);
    let shown = ShownAtoms::new(model.atoms(), AtomChannel::Policy(&both));
    assert!(matches!(shown, ShownAtoms::Tested { .. }));
    assert_eq!(shown.count(), 10);
}

#[test]
fn an_implicit_selection_tests_each_atom() {
    let model = model(&[("p", POSITIVE), ("q", POSITIVE)], 5);
    let all = OutputSelection::all();
    let shown = ShownAtoms::new(model.atoms(), AtomChannel::Policy(&all));
    assert!(matches!(shown, ShownAtoms::Tested { .. }));
    assert_eq!(shown.count(), 10);
}

#[test]
fn an_empty_explicit_selection_shows_no_atom_by_its_runs() {
    let model = model(&[("p", POSITIVE)], 5);
    let none = OutputSelection::none();
    let shown = ShownAtoms::new(model.atoms(), AtomChannel::Policy(&none));
    assert!(matches!(shown, ShownAtoms::Runs { .. }));
    assert_eq!(shown.count(), 0);
}

#[test]
fn the_runs_are_the_tested_atoms_in_model_order() {
    // `a` and `z` name no atom of the model; `p` occurs under both signs.
    let present = [
        ("p", POSITIVE),
        ("p", NEGATIVE),
        ("q", POSITIVE),
        ("r", POSITIVE),
        ("s", POSITIVE),
    ];
    let model = model(&present, 7);
    let candidates = [
        ("a", POSITIVE),
        ("p", POSITIVE),
        ("p", NEGATIVE),
        ("q", POSITIVE),
        ("s", POSITIVE),
        ("z", POSITIVE),
    ];
    let mut shown = 0;
    for subset in 0_u32..1 << candidates.len() {
        let chosen: Vec<_> = candidates
            .iter()
            .enumerate()
            .filter(|&(index, _)| subset >> index & 1 == 1)
            .map(|(_, &signature)| signature)
            .collect();
        let selection = selection(&chosen);
        let runs: Vec<_> = by_runs(&model, &selection).collect();
        let present = chosen
            .iter()
            .filter(|&&(name, _)| name != "a" && name != "z");
        assert_eq!(runs.len(), 7 * present.count(), "{chosen:?}");
        assert_eq!(
            runs,
            by_tests(&model, &selection).collect::<Vec<_>>(),
            "{chosen:?}"
        );
        shown += runs.len();
    }
    // Each of the four present signatures is chosen by half of the subsets.
    assert_eq!(shown, 7 * 4 * 32);
}

#[test]
fn runs_follow_signature_order_whatever_order_names_them() {
    let model = model(&[("p", POSITIVE), ("q", POSITIVE), ("r", POSITIVE)], 3);
    let selection = selection(&[("r", POSITIVE), ("p", POSITIVE)]);
    let expected: Vec<_> = atoms("p", POSITIVE, 3)
        .chain(atoms("r", POSITIVE, 3))
        .collect();
    assert_eq!(by_runs(&model, &selection).collect::<Vec<_>>(), expected);
}
