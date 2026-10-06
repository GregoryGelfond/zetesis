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

fn terminal_with(source: &str, limits: &FormulaLimits) -> TerminalFormula {
    let FormulaMaterialization::Terminal(owner) = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        *limits,
    )
    .unwrap()
    .ground_adaptive()
    .unwrap() else {
        panic!("terminal profile required")
    };
    owner
}

const MANY: &str = "{seed(1..4)}. receipt(X):-seed(X).";

/// Admission's work and one reconstruction's of `seeds`, under default limits.
fn costs_of(seeds: &[&str]) -> (u64, u64) {
    let owner = terminal(MANY);
    let mut cursor = owner.reconstruction().unwrap();
    cursor
        .reconstruct(&selection(&owner, seeds), &Cancellation::default())
        .unwrap();
    let statistics = cursor.statistics();
    (statistics.admission.work, statistics.latest.work)
}

fn costs() -> (u64, u64) {
    costs_of(&["seed(1)", "seed(3)"])
}

#[test]
fn the_allowance_is_the_headroom_admission_left() {
    let limits = FormulaLimits::default();
    let owner = terminal(MANY);
    let statistics = owner.reconstruction().unwrap().statistics();
    assert!(statistics.admission.work > 0);
    assert_eq!(
        statistics.allowance.work,
        limits.max_work - statistics.admission.work
    );
    assert_eq!(
        statistics.allowance.substitutions,
        limits.max_substitutions - statistics.admission.substitutions
    );
}

#[test]
fn distinct_answers_do_not_exhaust_the_grounding_ceiling() {
    let all = ["seed(1)", "seed(2)", "seed(3)", "seed(4)"];
    let (admission, largest) = costs_of(&all);
    // Room for grounding and about two of the largest reconstructions.
    let owner = terminal_with(
        MANY,
        &FormulaLimits {
            max_work: admission + 2 * largest,
            ..FormulaLimits::default()
        },
    );
    let mut cursor = owner.reconstruction().unwrap();
    // Every nonempty subset of the seeds is its own answer.
    for mask in 1..16_usize {
        let seeds: Vec<&str> = (0..4)
            .filter(|bit| mask & (1 << bit) != 0)
            .map(|bit| all[bit])
            .collect();
        let result = cursor
            .reconstruct(&selection(&owner, &seeds), &Cancellation::default())
            .unwrap();
        assert_eq!(names(&result).len(), 2 * seeds.len());
    }
    let statistics = cursor.statistics();
    assert_eq!(statistics.completed, 15);
    assert_eq!(statistics.peak.work, largest);
}

#[test]
fn repeated_reconstructions_do_not_exhaust_the_grounding_ceiling() {
    let (admission, call) = costs();
    // Room for grounding and about three reconstructions in all.
    let owner = terminal_with(
        MANY,
        &FormulaLimits {
            max_work: admission + 3 * call + call / 2,
            ..FormulaLimits::default()
        },
    );
    let mut cursor = owner.reconstruction().unwrap();
    let model = selection(&owner, &["seed(1)", "seed(3)"]);
    for _ in 0..10 {
        let result = cursor
            .reconstruct(&model, &Cancellation::default())
            .unwrap();
        assert_eq!(
            names(&result),
            ["receipt(1)", "receipt(3)", "seed(1)", "seed(3)"]
        );
    }
    assert_eq!(cursor.statistics().completed, 10);
}

#[test]
fn one_answer_beyond_the_headroom_is_refused() {
    let (admission, call) = costs();
    let owner = terminal_with(
        MANY,
        &FormulaLimits {
            max_work: admission + call / 2,
            ..FormulaLimits::default()
        },
    );
    let mut cursor = owner.reconstruction().unwrap();
    let model = selection(&owner, &["seed(1)", "seed(3)"]);
    assert!(
        cursor
            .reconstruct(&model, &Cancellation::default())
            .is_err()
    );
    assert_eq!(cursor.statistics().completed, 0);
}

#[test]
fn an_answer_fits_exactly_the_headroom_admission_left() {
    let (admission, call) = costs();
    let model_names = ["seed(1)", "seed(3)"];
    // Published at admission + c, with exact receipts.
    let owner = terminal_with(
        MANY,
        &FormulaLimits {
            max_work: admission + call,
            ..FormulaLimits::default()
        },
    );
    let mut cursor = owner.reconstruction().unwrap();
    for _ in 0..3 {
        cursor
            .reconstruct(&selection(&owner, &model_names), &Cancellation::default())
            .unwrap();
        assert_eq!(cursor.statistics().latest.work, call);
    }
    assert_eq!(cursor.statistics().work, admission + 3 * call);
    // Refused at admission + c − 1: the refusal names the allowance and the
    // call's own charge.
    let owner = terminal_with(
        MANY,
        &FormulaLimits {
            max_work: admission + call - 1,
            ..FormulaLimits::default()
        },
    );
    let mut cursor = owner.reconstruction().unwrap();
    let error = cursor
        .reconstruct(&selection(&owner, &model_names), &Cancellation::default())
        .unwrap_err();
    let ReconstructionError::Source(cause) = &error else {
        panic!("a work refusal: {error:?}");
    };
    assert!(
        matches!(cause.as_ref(), zetesis_themelios::FormulaFailure::Limit {
            resource: zetesis_themelios::FormulaResource::Work,
            observed,
            limit,
            ..
        } if *limit == u128::from(call - 1) && *observed == u128::from(call)),
        "{cause:?}"
    );
    let statistics = cursor.statistics();
    assert_eq!(statistics.completed, 0);
    assert_eq!(statistics.allowance.work, call - 1);
    assert_eq!(statistics.latest.work, call - 1);
    // The refused call is the largest so far.
    assert_eq!(statistics.peak.work, call - 1);
}
