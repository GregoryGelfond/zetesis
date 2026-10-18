//! Scalar assignments keep their arithmetic duties when their results select joins.

use zetesis_cpu::Cancellation;
use zetesis_reference_support::prepare;
use zetesis_sat::{Limits, StableModels};
use zetesis_themelios::{
    ExpansionFailure, FormulaFailure, FormulaLimits, observation::EvaluationError,
};

use super::{Observer, compile};

#[test]
fn scalar_chains_filter_before_independent_joins() {
    for source in [
        include_str!("../../fixtures/scalar-chains/selection.lp"),
        include_str!("../../fixtures/scalar-chains/constructed-head.lp"),
    ] {
        let observer = Observer::default();
        let admitted = compile(source, &FormulaLimits::default(), Some(&observer)).unwrap();
        let rows: u64 = observer
            .records
            .borrow()
            .iter()
            .map(|record| record.work.join_rows.unwrap())
            .sum();
        // Each successful X has ten frames. An independent 101 × 5 × 10
        // relational product must not precede the scalar chain's selection.
        assert!(rows < 2_000, "joined {rows} rows");
        let mut search = StableModels::new(
            admitted.theory(),
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        let model = search.next().unwrap().unwrap();
        let derived = model
            .atoms()
            .map(|index| admitted.atoms().at(index).unwrap())
            .filter(|atom| atom.predicate().name() == "p")
            .count();
        assert_eq!(derived, 50);
        assert!(search.next().is_none());
        assert!(search.exhausted());
    }
}

fn ground(source: &str, hybrid: bool) -> Result<usize, FormulaFailure> {
    let prepared = prepare(source)?;
    let observer = Observer::default();
    if hybrid {
        prepared
            .ground_hybrid_with_observer(Some(&observer))
            .map(|admitted| admitted.warnings().len())
    } else {
        prepared
            .ground_with_observer(Some(&observer))
            .map(|admitted| admitted.warnings().len())
    }
}

#[test]
fn generated_false_checks_do_not_hide_zero_division() {
    for source in [
        "d(0). p :- d(X), A=X+1, A=0, Z=1/X.",
        "d(0). p :- d(X), Z=1/X, A=0, A=X+1.",
    ] {
        for hybrid in [false, true] {
            let error = ground(source, hybrid).unwrap_err();
            assert!(
                matches!(
                    error,
                    FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                        error: EvaluationError::Undefined,
                        ..
                    })
                ),
                "{source}, hybrid={hybrid}: {error}"
            );
        }
    }
}

#[test]
fn unused_assignments_retain_overflow_errors() {
    for source in [
        "d(2147483647). p :- d(X), Z=X+1.",
        "d(2147483647). p :- d(X), A=X-1, A=0, Z=X+1.",
    ] {
        for hybrid in [false, true] {
            let error = ground(source, hybrid).unwrap_err();
            assert!(
                matches!(
                    error,
                    FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                        error: EvaluationError::Overflow,
                        ..
                    })
                ),
                "{source}, hybrid={hybrid}: {error}"
            );
        }
    }
}

#[test]
fn generated_zero_divisors_retain_family_warnings() {
    let source = "d(0;1). p(Y) :- d(X), D=X+0, Y=1/D.";
    for hybrid in [false, true] {
        assert_eq!(ground(source, hybrid).unwrap(), 1, "hybrid={hybrid}");
    }
}

#[test]
fn selected_false_bodies_do_not_evaluate_heads() {
    let source = "d(0). p(1/X) :- d(X), A=X+1, A=0.";
    for hybrid in [false, true] {
        assert_eq!(ground(source, hybrid).unwrap(), 0, "hybrid={hybrid}");
    }
}

#[test]
fn incomplete_prefix_failures_disappear_after_relational_rejection() {
    use std::collections::BTreeSet;
    use zetesis_core::Model;
    use zetesis_reference_support::canonical;
    use zetesis_themelios::{ConstraintCheckLimits, ConstraintVerdict};

    // narrow(X) certifies division on complete bindings. The expanded check
    // can be decided by wide(X,Y) first, where X=0 has no complete extension.
    // The producer and streamed constraint exercise both uses of that body.
    let source = "wide(0,0). wide(1,0). narrow(1). {block}. \
        p(X,Y) :- wide(X,Y), narrow(X), A=1/X, B=A+Y, B=1. \
        :- wide(X,Y), narrow(X), A=1/X, B=A+Y, B=1, not block.";
    let expected = BTreeSet::from([BTreeSet::from([
        "wide(0,0)".to_owned(),
        "wide(1,0)".to_owned(),
        "narrow(1)".to_owned(),
        "p(1,0)".to_owned(),
        "block".to_owned(),
    ])]);
    let eager = prepare(source).unwrap().ground().unwrap();
    assert!(eager.warnings().is_empty());
    let mut search =
        StableModels::new(eager.theory(), Limits::default(), Cancellation::default()).unwrap();
    let mut eager_answers = BTreeSet::new();
    for answer in search.by_ref() {
        eager_answers.insert(
            answer
                .unwrap()
                .atoms()
                .map(|atom| canonical(eager.atoms().at(atom).unwrap()))
                .collect::<BTreeSet<_>>(),
        );
    }
    assert!(search.exhausted());
    assert_eq!(eager_answers, expected);

    let hybrid = prepare(source).unwrap().ground_hybrid().unwrap();
    assert!(hybrid.warnings().is_empty());
    assert_eq!(
        hybrid.streamed_templates(),
        1,
        "the arithmetic constraint must be streamed"
    );
    let cancellation = Cancellation::default();
    let mut checker = hybrid.checker(ConstraintCheckLimits::default()).unwrap();
    let mut search = StableModels::new(
        hybrid.core_theory(),
        Limits::default(),
        cancellation.clone(),
    )
    .unwrap();
    let mut hybrid_answers = BTreeSet::new();
    let mut candidate_count = 0;
    for answer in search.by_ref() {
        let answer = answer.unwrap();
        let model = Model::from_positions(hybrid.atom_catalog(), answer.atoms()).unwrap();
        candidate_count += 1;
        if checker.check(&model, &cancellation).unwrap() == ConstraintVerdict::Satisfied {
            hybrid_answers.insert(
                answer
                    .atoms()
                    .map(|atom| canonical(hybrid.atom_catalog().atoms().at(atom).unwrap()))
                    .collect::<BTreeSet<_>>(),
            );
        }
    }
    assert!(search.exhausted());
    assert_eq!(
        candidate_count, 2,
        "both retained-core block candidates reach the checker"
    );
    assert!(checker.statistics().substitutions > 0);
    assert_eq!(hybrid_answers, expected);
}
