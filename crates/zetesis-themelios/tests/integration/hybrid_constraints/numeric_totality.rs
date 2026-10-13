use super::*;
use std::collections::BTreeSet;
use zetesis_ferraris::{Interpretation, Limits, check};
use zetesis_reference_support as reference;
use zetesis_reference_support::prepared;

fn full_family(owner: &HybridFormula) -> BTreeSet<BTreeSet<String>> {
    let count = owner.atom_catalog().atoms().len();
    assert!(count <= 8, "bounded independent complete enumeration");
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let cancellation = Cancellation::default();
    let mut answers = BTreeSet::new();
    for mask in 0..1_usize << count {
        let candidate = Interpretation::new(
            owner.core_theory(),
            (0..count).filter(|atom| mask & (1 << atom) != 0),
        )
        .unwrap();
        if !check(
            owner.core_theory(),
            &candidate,
            Limits::default(),
            &cancellation,
        )
        .unwrap()
        .accepted()
        {
            continue;
        }
        let model = Model::from_positions(owner.atom_catalog(), candidate.atoms()).unwrap();
        if checker.check(&model, &cancellation).unwrap() == ConstraintVerdict::Satisfied {
            answers.insert(
                candidate
                    .atoms()
                    .map(|atom| {
                        reference::canonical(owner.atom_catalog().atoms().at(atom).unwrap())
                    })
                    .collect(),
            );
        }
    }
    answers
}

#[test]
fn normal_arithmetic_preserves_eager_and_hybrid_families() {
    let source = include_str!("../../fixtures/column-totality/normal-selection.lp");
    let eager = prepared(source).ground().unwrap();
    let hybrid = hybrid(source);
    assert_eq!(
        hybrid.streamed_templates(),
        1,
        "the original remainder constraint is streamed"
    );
    let expected: BTreeSet<BTreeSet<String>> = [
        &[][..],
        &["p(9,3)"][..],
        &["p(5,2)", "kept(5,2)"][..],
        &["p(9,3)", "p(5,2)", "kept(5,2)"][..],
    ]
    .into_iter()
    .map(|atoms| atoms.iter().map(|atom| (*atom).to_owned()).collect())
    .collect();
    let eager = reference::exhaustive(&eager)
        .into_iter()
        .map(|(atoms, cost)| {
            assert!(cost.is_none());
            atoms
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(expected, eager);
    assert_eq!(expected, full_family(&hybrid));
}

#[test]
fn false_bodies_do_not_evaluate_generated_heads() {
    let source = include_str!("../../fixtures/column-totality/false-generated-head.lp");
    let expected = BTreeSet::from([BTreeSet::from(["d(0)".to_owned(), "e(1)".to_owned()])]);
    let eager = prepared(source).ground().unwrap();
    let hybrid = hybrid(source);
    let eager = reference::exhaustive(&eager)
        .into_iter()
        .map(|(atoms, cost)| {
            assert!(cost.is_none());
            atoms
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(expected, eager);
    assert_eq!(expected, full_family(&hybrid));
}

#[test]
fn normal_siblings_keep_the_original_family_warning() {
    let source = include_str!("../../fixtures/column-totality/normal-sibling-warning.lp");
    let eager = prepared(source).ground().unwrap();
    let hybrid = hybrid(source);
    assert_eq!(eager.warnings().len(), 1);
    assert_eq!(hybrid.warnings().len(), 1);
    for warning in eager.warnings().iter().chain(hybrid.warnings()) {
        let span = warning.location().unwrap().span;
        let start = usize::try_from(span.start().get()).unwrap();
        let end = usize::try_from(span.end().get()).unwrap();
        assert_eq!(source.get(start..end), Some("p(X):-d(X),e(Y),X/(Y;X)=1."));
    }
    assert_eq!(
        full_family(&hybrid),
        BTreeSet::from([BTreeSet::from(["d(0)".to_owned(), "e(1)".to_owned()])])
    );
}

#[test]
fn independent_fatal_arithmetic_keeps_its_source_site() {
    use zetesis_themelios::observation::EvaluationError;
    for (source, expected) in [
        (
            "d(0).e(1).p(X):-d(X),e(Y),Y/X=1,(2147483647+Y)*Y=0.",
            EvaluationError::Overflow,
        ),
        (
            "d(a).e(1).p(X):-d(X),e(Y),X*Y=0.",
            EvaluationError::Undefined,
        ),
    ] {
        for hybrid in [false, true] {
            let prepared = prepared(source);
            let result = if hybrid {
                prepared.ground_hybrid().map(|_| ())
            } else {
                prepared.ground().map(|_| ())
            };
            let error = result.unwrap_err();
            assert!(
                matches!(&error, FormulaFailure::Expansion(ExpansionFailure::Evaluation { error, .. }) if *error == expected)
            );
            let diagnostics = error.diagnostics();
            assert!(!diagnostics.is_empty());
            for diagnostic in diagnostics {
                let span = diagnostic.primary().location.span;
                let start = usize::try_from(span.start().get()).unwrap();
                let end = usize::try_from(span.end().get()).unwrap();
                assert_eq!(
                    source.get(start..end),
                    source.get(source.find("p(X)").unwrap()..)
                );
            }
        }
    }
}

#[test]
fn total_body_evidence_does_not_supply_a_head_binding() {
    let source = "d(1).e(1).p(Z):-d(X),e(Y),X/Y=1.";
    let result = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .and_then(zetesis_themelios::PreparedFormula::ground);
    assert!(matches!(result, Err(FormulaFailure::UnsafeVariable { .. })));
}
