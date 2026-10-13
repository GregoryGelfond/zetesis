//! Models as sets of atoms, as the formula propositions compare them.

use std::collections::BTreeSet;

use serde_json::Value as Json;
use zetesis_clingo_support as oracle;
use zetesis_core::{Atom, Predicate, Program, TemplateTerm, ValueLimits};
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
    let head = input
        .program()
        .templates()
        .at(0)
        .unwrap()
        .head()
        .expect("ground atom");
    let values = head
        .terms()
        .iter()
        .map(|term| match term {
            TemplateTerm::Constant(value) => value.to_value(ValueLimits::default()).unwrap(),
            TemplateTerm::Variable(_) => panic!("oracle atom must be ground"),
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
    oracle::parsed(&oracle::objective_free_models(report), atom)
}
