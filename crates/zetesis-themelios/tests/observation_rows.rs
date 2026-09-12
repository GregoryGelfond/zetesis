//! Predicate-local traversal preserves the whole-model observation relation.

use zetesis_core::{Atom, Model, Predicate, Sign, Value};
use zetesis_cpu::Control;
use zetesis_themelios::observation::{ErrorKind, Limits, Resource, Symbol};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

fn admit(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}

fn atom(name: &str, sign: Sign, values: &[i32]) -> Atom {
    Atom::new(
        Predicate::with_sign(name, values.len(), sign).unwrap(),
        values.iter().copied().map(Value::Number).collect(),
    )
    .unwrap()
}

fn symbols(input: &AdmittedFormula, model: &Model) -> Vec<Symbol> {
    input
        .metadata()
        .observations()
        .evaluate(model, Limits::default(), &Control::default())
        .unwrap()
        .symbols()
        .to_vec()
}

fn tuple(left: i32, right: i32) -> Symbol {
    Symbol::Tuple(vec![Symbol::Number(left), Symbol::Number(right)])
}

fn unary(model: &Model, name: &str, sign: Sign) -> Vec<i32> {
    model
        .atoms()
        .iter()
        .filter_map(|atom| {
            if atom.predicate().name() == name
                && atom.predicate().sign() == sign
                && let [Value::Number(value)] = atom.values()
            {
                return Some(*value);
            }
            None
        })
        .collect()
}

#[test]
fn observation_joins_match_full_row_enumeration() {
    let input = admit("#show (X,Y):p(X),q(Y),X<=Y.");
    let pool: Vec<_> = ["p", "q"]
        .into_iter()
        .flat_map(|name| (0..4).map(move |value| atom(name, Sign::Positive, &[value])))
        .collect();
    for mask in 0..256_u16 {
        let model = Model::new(
            pool.iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, atom)| atom.clone())
                .chain([
                    atom("p", Sign::Negative, &[7]),
                    atom("p", Sign::Positive, &[]),
                    atom("p", Sign::Positive, &[0, 1]),
                    atom("q", Sign::Negative, &[7]),
                    atom("unrelated", Sign::Positive, &[8]),
                ]),
        );
        let expected: Vec<_> = unary(&model, "p", Sign::Positive)
            .into_iter()
            .flat_map(|left| {
                unary(&model, "q", Sign::Positive)
                    .into_iter()
                    .filter(move |right| left <= *right)
                    .map(move |right| tuple(left, right))
            })
            .collect();
        assert_eq!(symbols(&input, &model), expected, "mask={mask}");
    }
}

#[test]
fn negative_observations_use_complete_signed_identity() {
    let input = admit("#show X: -p(X),not q(X).");
    let model = Model::new([
        atom("p", Sign::Negative, &[1]),
        atom("p", Sign::Negative, &[2]),
        atom("p", Sign::Positive, &[3]),
        atom("q", Sign::Positive, &[2]),
        atom("q", Sign::Negative, &[1]),
        atom("q", Sign::Positive, &[1, 2]),
    ]);
    assert_eq!(symbols(&input, &model), [Symbol::Number(1)]);
}

#[test]
fn nested_aggregate_queries_retain_join_bindings() {
    let input = admit("#show (X,N):p(X),N=#count{Y:q(Y),Y>=X}.");
    let model = Model::new([
        atom("p", Sign::Positive, &[1]),
        atom("p", Sign::Positive, &[2]),
        atom("q", Sign::Positive, &[1]),
        atom("q", Sign::Positive, &[2]),
        atom("q", Sign::Positive, &[3]),
        atom("q", Sign::Negative, &[4]),
    ]);
    assert_eq!(symbols(&input, &model), [tuple(1, 3), tuple(2, 2)]);
}

#[test]
fn tuple_aliases_match_one_complete_row() {
    let input = admit("#show X:edge(X,X).");
    let model = Model::new([
        atom("edge", Sign::Positive, &[1, 1]),
        atom("edge", Sign::Positive, &[1, 2]),
        atom("edge", Sign::Positive, &[2, 1]),
        atom("edge", Sign::Negative, &[2, 2]),
    ]);
    assert_eq!(symbols(&input, &model), [Symbol::Number(1)]);
}

#[test]
fn sparse_query_work_excludes_unrelated_row_products() {
    let input = admit("#show (X,Y):p(X),q(Y).");
    let model = Model::new(
        (0..256)
            .map(|value| atom("other", Sign::Positive, &[value]))
            .chain([
                atom("p", Sign::Positive, &[1]),
                atom("q", Sign::Positive, &[2]),
            ]),
    );
    // The old two-depth whole-model scan exceeds this budget. The new budget
    // includes all 258 model references, predicate probes and complete output.
    let evaluation = input
        .metadata()
        .observations()
        .evaluate(
            &model,
            Limits {
                max_work: 1_000,
                ..Limits::default()
            },
            &Control::default(),
        )
        .unwrap();
    assert_eq!(evaluation.symbols(), [tuple(1, 2)]);
    assert_eq!(evaluation.statistics().bindings, 1);
}

#[test]
fn query_work_refusals_publish_no_partial_terms() {
    let input = admit("#show (X,Y):p(X),q(Y).");
    let model = Model::new([
        atom("p", Sign::Positive, &[1]),
        atom("q", Sign::Positive, &[1]),
        atom("q", Sign::Positive, &[2]),
    ]);
    let program = input.metadata().observations();
    let complete = program
        .evaluate(&model, Limits::default(), &Control::default())
        .unwrap();
    for ceiling in 0..complete.statistics().work {
        let failure = program
            .evaluate(
                &model,
                Limits {
                    max_work: ceiling,
                    ..Limits::default()
                },
                &Control::default(),
            )
            .unwrap_err();
        assert!(matches!(
            failure.kind(),
            ErrorKind::Limit {
                resource: Resource::Work,
                ..
            }
        ));
        assert!(failure.statistics().work <= ceiling);
    }
    let exact = program
        .evaluate(
            &model,
            Limits {
                max_work: complete.statistics().work,
                ..Limits::default()
            },
            &Control::default(),
        )
        .unwrap();
    assert_eq!(exact.symbols(), complete.symbols());
    assert_eq!(exact.statistics(), complete.statistics());
}
