//! Models as sets of atoms, as the formula propositions compare them.

use std::collections::BTreeSet;

use serde_json::Value as Json;
use zetesis_clingo_support as oracle;
use zetesis_core::{Atom, PatternRef, Predicate, Program, TemplateTerm, ValueLimits};
use zetesis_reference_support::normal_answer_sets;
use zetesis_themelios::{AdmissionOptions, admit};

/// Models as sets of atoms.
pub(crate) type Models = BTreeSet<BTreeSet<Atom>>;

/// Every answer set of the normal program `program`, as sets of atoms; no two
/// candidates yield the same one.
pub(crate) fn answer_sets(program: &Program) -> Models {
    let mut models = Models::new();
    for model in normal_answer_sets(program) {
        let atoms = model
            .atoms()
            .iter()
            .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
            .collect();
        assert!(models.insert(atoms), "one candidate per answer set");
    }
    models
}

/// The atom a complete ground atom's clingo spelling denotes, read by admitting
/// it as a fact under the default options.
pub(crate) fn atom(source: &str) -> Atom {
    // Parsing complete atoms preserves strings and numeric identities; output
    // atoms are never split at whitespace or compared through display filters.
    let input = admit(format!("{source}."), AdmissionOptions::default())
        .expect("campaign atoms belong to the scalar profile");
    assert_eq!(input.program().templates().len(), 1);
    ground_atom(
        input
            .program()
            .templates()
            .at(0)
            .unwrap()
            .head()
            .expect("ground atom"),
    )
}

/// The atom an admitted fact's ground head denotes.
pub(crate) fn ground_atom(head: PatternRef<'_>) -> Atom {
    let values = head
        .terms()
        .iter()
        .map(|term| match term {
            TemplateTerm::Constant(value) => value.to_value(ValueLimits::default()).unwrap(),
            TemplateTerm::Variable(_) => panic!("a fact's head is ground"),
        })
        .collect();
    let predicate = head.predicate();
    Atom::new(
        Predicate::with_sign(predicate.name(), predicate.arity(), predicate.sign()).unwrap(),
        values,
    )
    .expect("canonical atom arity")
}

/// The full models of a complete clingo report of a program without an
/// objective, read as atoms; reading keeps every model and atom distinct.
pub(crate) fn clingo_models(report: &Json) -> Models {
    parsed(&oracle::objective_free_models(report), atom)
}

/// `models` with every atom spelling read by `parse`, which must keep every
/// atom of a model, and every model, distinct.
///
/// # Panics
/// Panics if two spellings of one model, or two models, read as one.
fn parsed<T: Ord>(
    models: &BTreeSet<BTreeSet<String>>,
    mut parse: impl FnMut(&str) -> T,
) -> BTreeSet<BTreeSet<T>> {
    let parsed: BTreeSet<BTreeSet<T>> = models
        .iter()
        .map(|model| {
            let atoms: BTreeSet<T> = model.iter().map(|spelling| parse(spelling)).collect();
            assert_eq!(
                atoms.len(),
                model.len(),
                "parsing keeps a model's atoms distinct"
            );
            atoms
        })
        .collect();
    assert_eq!(
        parsed.len(),
        models.len(),
        "parsing keeps the models distinct"
    );
    parsed
}

/// Models spelled as clingo spells their atoms.
fn spelled(models: &[&[&str]]) -> BTreeSet<BTreeSet<String>> {
    models
        .iter()
        .map(|atoms| atoms.iter().map(|&atom| atom.to_owned()).collect())
        .collect()
}

#[test]
fn parsed_models_keep_every_model_and_atom() {
    let upper = parsed(&spelled(&[&["a"], &["b", "c"]]), str::to_uppercase);
    assert_eq!(upper, spelled(&[&["A"], &["B", "C"]]));
}

#[test]
#[should_panic(expected = "parsing keeps a model's atoms distinct")]
fn parsing_that_merges_atoms_is_refused() {
    let _ = parsed(&spelled(&[&["a"], &["b", "c"]]), |_| 0);
}

#[test]
#[should_panic(expected = "parsing keeps the models distinct")]
fn parsing_that_merges_models_is_refused() {
    let _ = parsed(&spelled(&[&["a"], &["b"]]), |_| 0);
}
