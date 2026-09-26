//! Atom-channel membership is shared by charged and uncharged consumers.

use zetesis_core::catalog::AtomCatalog;
use zetesis_core::{Atom, Predicate, Sign, Value};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_themelios::{AtomSelection, AtomSelectionLimits, PreparedSelection, observation};

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
                .iter()
                .zip(selection.signatures().iter().skip(1))
                .all(|(left, right)| left < right)
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

#[test]
fn source_selection_matches_explicit_union_construction() {
    use std::fmt::Write;
    use zetesis_themelios::{AdmissionOptions, ExpansionLimits, admit_extended};

    let signatures: Vec<_> = (0..256)
        .rev()
        .map(|index| {
            Predicate::with_sign(
                format!("p{index:03}"),
                index % 3,
                if index % 2 == 0 {
                    Sign::Positive
                } else {
                    Sign::Negative
                },
            )
            .unwrap()
        })
        .collect();
    let mut source = String::new();
    for signature in signatures.iter().chain(&signatures) {
        let prefix = if signature.sign() == Sign::Negative {
            "-"
        } else {
            ""
        };
        writeln!(
            source,
            "#show {prefix}{}/{}.",
            signature.name(),
            signature.arity()
        )
        .unwrap();
    }
    let admitted = admit_extended(
        source,
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap();
    let expected =
        AtomSelection::from_signatures(&signatures, AtomSelectionLimits::default()).unwrap();
    assert_eq!(admitted.metadata().atom_selection(), &expected);
    assert_eq!(admitted.metadata().directives().len(), 512);
}

fn canonical(atoms: &[Atom]) -> AtomCatalog {
    AtomCatalog::new(atoms.to_vec()).unwrap()
}

fn charges(
    decide: impl FnOnce(&mut dyn FnMut(u128) -> Result<(), ()>) -> Result<bool, ()>,
) -> (bool, Vec<u128>) {
    let mut charged = Vec::new();
    let selected = decide(&mut |units| {
        charged.push(units);
        Ok(())
    })
    .unwrap();
    (selected, charged)
}

#[test]
fn prepared_selection_matches_the_signed_signature_union() {
    let atoms = [
        atom("p", 0, Sign::Positive),
        atom("p", 0, Sign::Negative),
        atom("p", 1, Sign::Positive),
        atom("p", 1, Sign::Negative),
        atom("q", 0, Sign::Positive),
        atom("q", 1, Sign::Negative),
    ];
    let catalog = canonical(&atoms);
    for subset in 0..(1_usize << atoms.len()) {
        let signatures: Vec<_> = atoms
            .iter()
            .enumerate()
            .filter(|(index, _)| subset & (1 << index) != 0)
            .map(|(_, atom)| atom.predicate().clone())
            .collect();
        let selection =
            AtomSelection::from_signatures(&signatures, AtomSelectionLimits::default()).unwrap();
        let prepared = selection
            .prepare_with(catalog.read(), |_| Ok::<(), ()>(()))
            .unwrap();
        for atom in catalog.atoms() {
            assert_eq!(prepared.includes(atom), selection.includes(atom));
            assert_eq!(
                prepared.try_includes(atom, |_| Ok::<(), ()>(())),
                Ok(selection.includes(atom))
            );
        }
    }
}

#[test]
fn a_prepared_decision_charges_one_unit() {
    let signatures: Vec<_> = (0..1_024)
        .map(|index| Predicate::new(format!("p{index:04}"), 0).unwrap())
        .collect();
    let selection =
        AtomSelection::from_signatures(&signatures, AtomSelectionLimits::default()).unwrap();
    let catalog = canonical(&[
        atom("p0512", 0, Sign::Positive),
        atom("z", 0, Sign::Positive),
    ]);
    let prepared = selection
        .prepare_with(catalog.read(), |_| Ok::<(), ()>(()))
        .unwrap();
    for atom in catalog.atoms() {
        let (selected, charged) = charges(|charge| prepared.try_includes(atom, charge));
        assert_eq!(selected, selection.includes(atom));
        assert_eq!(charged, [1]);
    }
}

#[test]
fn preparation_charges_each_predicate_as_its_search() {
    // One decision per vocabulary predicate, each charged exactly as the
    // unprepared search charges an atom of that predicate.
    let selection = AtomSelection::from_signatures(
        &[
            Predicate::new("p", 1).unwrap(),
            Predicate::new("r", 2).unwrap(),
        ],
        AtomSelectionLimits::default(),
    )
    .unwrap();
    let catalog = canonical(&[
        atom("p", 1, Sign::Positive),
        atom("q", 0, Sign::Positive),
        atom("p", 1, Sign::Positive),
        atom("s", 3, Sign::Negative),
    ]);
    let (_, prepared) = charges(|charge| {
        selection
            .prepare_with(catalog.read(), charge)
            .map(|prepared| prepared.includes(catalog.atoms().at(0).unwrap()))
    });
    let searched: Vec<u128> = [0, 1, 3]
        .into_iter()
        .flat_map(|position| {
            let atom = catalog.atoms().at(position).unwrap();
            charges(|charge| selection.try_includes(atom, charge)).1
        })
        .collect();
    assert_eq!(prepared, searched);
}

#[test]
fn another_vocabulary_keeps_the_search_and_its_charges() {
    let selection = AtomSelection::from_signatures(
        &[
            Predicate::new("p", 1).unwrap(),
            Predicate::new("q", 0).unwrap(),
        ],
        AtomSelectionLimits::default(),
    )
    .unwrap();
    let atoms = [atom("p", 1, Sign::Positive), atom("r", 0, Sign::Positive)];
    let prepared_for = canonical(&atoms);
    let other = canonical(&atoms);
    let prepared = selection
        .prepare_with(prepared_for.read(), |_| Ok::<(), ()>(()))
        .unwrap();
    for atom in other.atoms() {
        assert_eq!(
            charges(|charge| prepared.try_includes(atom, charge)),
            charges(|charge| selection.try_includes(atom, charge))
        );
    }
}

#[test]
fn ingress_atoms_keep_the_search_and_its_charges() {
    let selection = AtomSelection::from_signatures(
        &[Predicate::new("p", 1).unwrap()],
        AtomSelectionLimits::default(),
    )
    .unwrap();
    let catalog = canonical(&[atom("p", 1, Sign::Positive)]);
    let prepared = selection
        .prepare_with(catalog.read(), |_| Ok::<(), ()>(()))
        .unwrap();
    for query in [atom("p", 1, Sign::Positive), atom("q", 1, Sign::Positive)] {
        assert_eq!(
            charges(|charge| prepared.try_includes(&query, charge)),
            charges(|charge| selection.try_includes(&query, charge))
        );
    }
}

#[test]
fn constant_selections_prepare_without_charges() {
    let catalog = canonical(&[atom("p", 0, Sign::Positive)]);
    for selection in [AtomSelection::all(), AtomSelection::none()] {
        let prepared = selection.prepare_with(catalog.read(), |_| Err(())).unwrap();
        let atom = catalog.atoms().at(0).unwrap();
        assert_eq!(prepared.includes(atom), selection.includes(atom));
        assert_eq!(
            prepared.try_includes(atom, |_| Err(())),
            Ok(selection.includes(atom))
        );
    }
}

#[test]
fn a_refused_preparation_returns_the_refusal() {
    let selection = AtomSelection::from_signatures(
        &[Predicate::new("p", 0).unwrap()],
        AtomSelectionLimits::default(),
    )
    .unwrap();
    let catalog = canonical(&[atom("p", 0, Sign::Positive), atom("q", 0, Sign::Positive)]);
    let mut calls = 0;
    let result = selection.prepare_with(catalog.read(), |_| {
        calls += 1;
        if calls == 2 { Err("stopped") } else { Ok(()) }
    });
    assert_eq!(result.err(), Some("stopped"));
    assert_eq!(calls, 2);
}

#[test]
fn a_prepared_selection_retains_one_word_per_64_predicates() {
    let selection = AtomSelection::from_signatures(
        &[Predicate::new("p", 0).unwrap()],
        AtomSelectionLimits::default(),
    )
    .unwrap();
    let catalog = canonical(&[atom("p", 0, Sign::Positive), atom("q", 0, Sign::Positive)]);
    let prepared = selection
        .prepare_with(catalog.read(), |_| Ok::<(), ()>(()))
        .unwrap();
    assert_eq!(
        prepared.retained_bytes(),
        size_of::<PreparedSelection<'_>>() + size_of::<u64>()
    );
    assert_eq!(
        PreparedSelection::from(&selection).retained_bytes(),
        size_of::<PreparedSelection<'_>>()
    );
}

#[test]
fn preparation_over_the_work_ceiling_keeps_the_search() {
    // Deciding both predicates needs two one-byte comparisons of three units.
    let selection = AtomSelection::from_signatures(
        &[Predicate::new("p", 0).unwrap()],
        AtomSelectionLimits::default(),
    )
    .unwrap();
    let catalog = canonical(&[atom("p", 0, Sign::Positive), atom("q", 0, Sign::Positive)]);
    let prepare = |max_work| {
        observation::prepare_selection(
            &selection,
            catalog.read(),
            observation::Limits {
                max_work,
                ..observation::Limits::default()
            },
            &Cancellation::default(),
        )
        .unwrap()
        .is_prepared()
    };
    assert!(prepare(6));
    assert!(!prepare(5));
}

#[test]
fn cancelled_preparation_stops() {
    let selection = AtomSelection::from_signatures(
        &[Predicate::new("p", 0).unwrap()],
        AtomSelectionLimits::default(),
    )
    .unwrap();
    let catalog = canonical(&[atom("p", 0, Sign::Positive)]);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let result = observation::prepare_selection(
        &selection,
        catalog.read(),
        observation::Limits::default(),
        &cancellation,
    );
    assert_eq!(result.err(), Some(Stop::Cancelled));
}
