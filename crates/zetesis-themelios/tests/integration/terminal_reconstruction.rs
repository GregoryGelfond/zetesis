//! Public reconstruction owns selected truth independently of canonical storage.

use zetesis_core::{Atom, Model, Predicate, Value};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaLimits, FormulaMaterialization, ReconstructionError,
    TerminalFormula, prepare_formula,
};

fn terminal(source: &str) -> TerminalFormula {
    let FormulaMaterialization::Terminal(owner) = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_adaptive()
    .unwrap() else {
        panic!("terminal profile required")
    };
    owner
}

fn selection(owner: &TerminalFormula, names: &[&str]) -> Model {
    let positions = owner
        .base_atom_catalog()
        .atoms()
        .iter()
        .enumerate()
        .filter_map(|(position, atom)| {
            names.contains(&spelling(atom).as_str()).then_some(position)
        });
    Model::from_positions(owner.base_atom_catalog(), positions).unwrap()
}

fn names(model: &Model) -> Vec<String> {
    model.atoms().iter().map(spelling).collect()
}

fn spelling(atom: zetesis_core::catalog::AtomRef<'_>) -> String {
    assert_eq!(atom.predicate().sign(), zetesis_core::Sign::Positive);
    let name = atom.predicate().name();
    if atom.values().is_empty() {
        return name.into();
    }
    let values: Vec<_> = atom
        .values()
        .iter()
        .map(|value| value.to_string())
        .collect();
    format!("{name}({})", values.join(","))
}

#[test]
fn independent_answers_do_not_inherit_derived_truth() {
    let owner = terminal("{seed(1);seed(2)}. receipt(X):-seed(X).");
    let mut cursor = owner.reconstruction().unwrap();
    let control = Cancellation::default();
    let first = cursor
        .reconstruct(&selection(&owner, &["seed(1)"]), &control)
        .unwrap();
    let second = cursor
        .reconstruct(&selection(&owner, &["seed(2)"]), &control)
        .unwrap();
    let empty = cursor
        .reconstruct(&selection(&owner, &[]), &control)
        .unwrap();
    assert_eq!(names(&first), ["receipt(1)", "seed(1)"]);
    assert_eq!(names(&second), ["receipt(2)", "seed(2)"]);
    assert!(names(&empty).is_empty());
    assert_eq!(cursor.statistics().completed, 3);
}

#[test]
fn an_equal_foreign_catalog_is_refused() {
    let owner = terminal("seed(1). receipt(X):-seed(X).");
    let atom = Atom::new(Predicate::new("seed", 1).unwrap(), vec![Value::Number(1)]).unwrap();
    let foreign = Model::new([atom]).unwrap();
    let actual = selection(&owner, &["seed(1)"]);
    assert_eq!(foreign, actual);
    let mut cursor = owner.reconstruction().unwrap();
    assert!(matches!(
        cursor.reconstruct(&foreign, &Cancellation::default()),
        Err(ReconstructionError::ForeignInput)
    ));
    assert!(matches!(
        cursor.reconstruct(&actual, &Cancellation::default()),
        Err(ReconstructionError::Failed)
    ));
    assert_eq!(cursor.statistics().completed, 0);
}

#[test]
fn repeated_variables_filter_complete_rows() {
    let owner = terminal("edge(1,1). edge(1,2). edge(2,2). selected(X):-edge(X,X).");
    let model = selection(&owner, &["edge(1,1)", "edge(1,2)"]);
    let result = owner
        .reconstruction()
        .unwrap()
        .reconstruct(&model, &Cancellation::default())
        .unwrap();
    assert_eq!(names(&result), ["edge(1,1)", "edge(1,2)", "selected(1)"]);
}

#[test]
fn duplicate_witnesses_publish_one_head() {
    let owner = terminal("edge(1,1). edge(1,2). selected(X):-edge(X,Y).");
    let model = selection(&owner, &["edge(1,1)", "edge(1,2)"]);
    let mut cursor = owner.reconstruction().unwrap();
    let before = cursor.statistics().substitutions;
    let result = cursor
        .reconstruct(&model, &Cancellation::default())
        .unwrap();
    assert_eq!(names(&result), ["edge(1,1)", "edge(1,2)", "selected(1)"]);
    assert_eq!(cursor.statistics().substitutions - before, 2);
}

#[test]
fn cancellation_does_not_publish_an_empty_answer() {
    let owner = terminal("{seed(1)}. receipt(X):-seed(X).");
    let model = selection(&owner, &[]);
    let mut cursor = owner.reconstruction().unwrap();
    let control = Cancellation::default();
    control.cancel();
    assert_eq!(
        cursor.reconstruct(&model, &control).unwrap_err().stop(),
        Some(Stop::Cancelled)
    );
    assert_eq!(cursor.statistics().completed, 0);
    assert!(matches!(
        cursor.reconstruct(&model, &Cancellation::default()),
        Err(ReconstructionError::Failed)
    ));
}

#[test]
fn sessions_start_from_the_same_admission_history() {
    let owner = terminal("seed(1). receipt(X):-seed(X).");
    let model = selection(&owner, &["seed(1)"]);
    let mut first = owner.reconstruction().unwrap();
    let baseline = first.statistics();
    assert!(baseline.work > 0);
    first.reconstruct(&model, &Cancellation::default()).unwrap();
    let second = owner.reconstruction().unwrap();
    assert_eq!(second.statistics(), baseline);
    assert!(first.statistics().work > second.statistics().work);
}
