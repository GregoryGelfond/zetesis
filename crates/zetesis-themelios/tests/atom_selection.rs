//! Atom-channel membership is shared by charged and uncharged consumers.

use zetesis_core::{Atom, Predicate, Sign, Value};
use zetesis_themelios::{AtomSelection, AtomSelectionLimits};

fn atom(name: &str, arity: usize, sign: Sign) -> Atom {
    Atom::new(
        Predicate::with_sign(name, arity, sign).unwrap(),
        vec![Value::Number(0); arity],
    )
    .unwrap()
}

#[test]
fn selection_matches_the_signed_signature_union() {
    let atoms = [
        atom("p", 0, Sign::Positive),
        atom("p", 0, Sign::Negative),
        atom("p", 1, Sign::Positive),
        atom("p", 1, Sign::Negative),
        atom("q", 0, Sign::Positive),
        atom("q", 1, Sign::Negative),
    ];
    for subset in 0..(1_usize << atoms.len()) {
        let mut signatures: Vec<_> = atoms
            .iter()
            .enumerate()
            .filter(|(index, _)| subset & (1 << index) != 0)
            .map(|(_, atom)| atom.predicate().clone())
            .collect();
        // Reversed input and repeated signatures retain set semantics.
        signatures.reverse();
        signatures.extend(signatures.clone());
        let selection =
            AtomSelection::from_signatures(&signatures, AtomSelectionLimits::default()).unwrap();
        for atom in &atoms {
            let expected = signatures.iter().any(|p| p == atom.predicate());
            assert_eq!(selection.includes(atom), expected);
            assert_eq!(
                selection.try_includes(atom, |_| Ok::<(), ()>(())),
                Ok(expected)
            );
        }
        assert!(
            selection
                .signatures()
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );
    }
}

#[test]
fn signature_probes_are_logarithmically_bounded() {
    let signatures: Vec<_> = (0..1_024)
        .map(|index| Predicate::new(format!("p{index:04}"), 0).unwrap())
        .collect();
    let selection =
        AtomSelection::from_signatures(&signatures, AtomSelectionLimits::default()).unwrap();
    for name in ["a", "p0000", "p0512", "p1023", "z"] {
        let query = atom(name, 0, Sign::Positive);
        let mut probes = 0;
        let mut units = 0;
        let selected = selection
            .try_includes(&query, |charge| {
                probes += 1;
                units += charge;
                Ok::<(), ()>(())
            })
            .unwrap();
        assert_eq!(selected, signatures.iter().any(|p| p == query.predicate()));
        assert!((1..=11).contains(&probes));
        assert_eq!(units, probes * (1 + name.len() as u128 + 5));
    }
}

#[test]
fn refused_membership_preserves_the_selection() {
    let query = atom("p", 0, Sign::Positive);
    let selection = AtomSelection::from_signatures(
        &[query.predicate().clone()],
        AtomSelectionLimits::default(),
    )
    .unwrap();
    let original = selection.clone();
    let mut calls = 0;
    let result = selection.try_includes(&query, |_| {
        calls += 1;
        Err("stopped")
    });
    assert_eq!(result, Err("stopped"));
    assert_eq!(calls, 1);
    assert_eq!(selection, original);
    assert!(selection.includes(&query));
}

#[test]
fn constant_selections_need_no_signature_probe() {
    let query = atom("p", 0, Sign::Positive);
    assert_eq!(
        AtomSelection::all().try_includes(&query, |_| Err(())),
        Ok(true)
    );
    assert_eq!(
        AtomSelection::none().try_includes(&query, |_| Err(())),
        Ok(false)
    );
}
