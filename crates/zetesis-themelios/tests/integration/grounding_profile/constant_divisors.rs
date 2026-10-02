//! Literal divisors defer only the redundant complete-family traversal.

use zetesis_themelios::{
    AdmissionOptions, ExpansionFailure, ExpansionLimits, FormulaFailure, FormulaLimits,
    GroundingOutcome, GroundingPhase, observation::EvaluationError, prepare_formula,
};

use super::{Observer, Record};

fn ground(source: &str, hybrid: bool, observer: &Observer) -> Result<usize, FormulaFailure> {
    let prepared = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?;
    if hybrid {
        prepared
            .ground_hybrid_with_observer(Some(observer))
            .map(|admitted| admitted.warnings().len())
    } else {
        prepared
            .ground_with_observer(Some(observer))
            .map(|admitted| admitted.warnings().len())
    }
}

fn family_pass(observer: &Observer) -> Record {
    *observer
        .records
        .borrow()
        .iter()
        .rfind(|record| record.phase == GroundingPhase::SupportCompletion)
        .expect("family admission is the final support-completion phase")
}

fn failure(source: &str, expected: &EvaluationError, phase: GroundingPhase) {
    for hybrid in [false, true] {
        let observer = Observer::default();
        let error = ground(source, hybrid, &observer).unwrap_err();
        assert!(
            matches!(
                &error,
                FormulaFailure::Expansion(ExpansionFailure::Evaluation { error, .. })
                    if error == expected
            ),
            "{source}, hybrid={hybrid}: {error}"
        );
        let records = observer.records.borrow();
        let failed = records.last().unwrap();
        assert_eq!(failed.phase, phase, "{source}, hybrid={hybrid}");
        assert_eq!(failed.outcome, GroundingOutcome::Failed);
        for diagnostic in error.diagnostics() {
            let span = diagnostic.primary().location.span;
            let text = &source[span.start().get() as usize..span.end().get() as usize];
            assert!(text.contains(":-"), "{text}");
        }
    }
}

#[test]
fn literal_divisors_omit_family_row_evaluation() {
    for source in [
        "d(1;2). :- d(X), X/2=3.",
        "d(1;2). :- d(X), X\\2=3.",
        "d(1;2). :- d(X), (X/2)/4=3.",
    ] {
        for hybrid in [false, true] {
            let observer = Observer::default();
            assert_eq!(ground(source, hybrid, &observer).unwrap(), 0);
            let pass = family_pass(&observer);
            assert_eq!(pass.work.join_rows, Some(0));
            assert_eq!(pass.work.expression_evaluations, Some(0));
            assert!(observer.records.borrow().iter().any(|record| {
                record.phase == GroundingPhase::RuleInstantiation
                    && record
                        .work
                        .expression_evaluations
                        .is_some_and(|count| count > 0)
            }));
        }
    }
}

#[test]
fn possible_zero_divisors_retain_family_refusal() {
    for source in ["d(0). :- d(X), 1/X=0.", "d(1). :- d(X), X/0=0."] {
        failure(
            source,
            &EvaluationError::Undefined,
            GroundingPhase::SupportCompletion,
        );
    }
}

#[test]
fn nested_zero_divisors_retain_family_refusal() {
    failure(
        "d(0). :- d(X), (1/X)/2=0.",
        &EvaluationError::Undefined,
        GroundingPhase::SupportCompletion,
    );
}

#[test]
fn nonnumeric_divisors_retain_family_refusal() {
    failure(
        "d(1). :- d(X), X/symbol=0.",
        &EvaluationError::Undefined,
        GroundingPhase::SupportCompletion,
    );
}

#[test]
fn nonnumeric_operands_remain_fatal_after_deferral() {
    failure(
        "d(symbol). :- d(X), X/2=0.",
        &EvaluationError::Undefined,
        GroundingPhase::RuleInstantiation,
    );
}

#[test]
fn independent_overflow_remains_fatal_after_deferral() {
    for body in ["X/2=0,X+2147483647=0", "X+2147483647=0,X/2=0"] {
        failure(
            &format!("d(1). :- d(X), {body}."),
            &EvaluationError::Overflow,
            GroundingPhase::RuleInstantiation,
        );
    }
}

#[test]
fn false_guards_do_not_hide_independent_overflow() {
    for body in ["not X/2=0,X+2147483647=0", "X+2147483647=0,not X/2=0"] {
        failure(
            &format!("d(1). :- d(X), {body}."),
            &EvaluationError::Overflow,
            GroundingPhase::RuleInstantiation,
        );
    }
}

#[test]
fn ordinary_false_comparisons_still_exclude_overflow() {
    for body in ["X/2=1,X+2147483647=0", "X+2147483647=0,X/2=1"] {
        for hybrid in [false, true] {
            let observer = Observer::default();
            assert_eq!(
                ground(&format!("d(1). :- d(X), {body}."), hybrid, &observer).unwrap(),
                0
            );
            assert_eq!(family_pass(&observer).work.join_rows, Some(0));
        }
    }
}

#[test]
fn empty_relational_extensions_remain_silent() {
    for hybrid in [false, true] {
        let observer = Observer::default();
        assert_eq!(
            ground(
                "d(1).e(2). :- d(X),e(X),X/2=0,X+2147483647=0.",
                hybrid,
                &observer,
            )
            .unwrap(),
            0
        );
    }
}

#[test]
fn generated_constraints_retain_family_validation() {
    failure(
        "d(2). :- d(X),Y=X/2,Y+2147483647=0.",
        &EvaluationError::Overflow,
        GroundingPhase::SupportCompletion,
    );
}

#[test]
fn nested_scopes_retain_family_row_evaluation() {
    let observer = Observer::default();
    ground(
        "d(1).e(1). :- d(X),#count{Y:e(Y),Y/2=0}>1.",
        false,
        &observer,
    )
    .unwrap();
    assert!(
        family_pass(&observer)
            .work
            .expression_evaluations
            .is_some_and(|count| count > 0)
    );
}

#[test]
fn false_local_guards_preserve_independent_overflow() {
    failure(
        "d(1).e(1). :- d(K),#count{X:e(X),not X/2=0,X+2147483647=0}>0.",
        &EvaluationError::Overflow,
        GroundingPhase::SupportCompletion,
    );
}

#[test]
fn empty_late_heads_preserve_reached_body_overflow() {
    for body in ["X+2147483647=0", "not X=1,X+2147483647=0"] {
        failure(
            &format!("d(1).p(X/2,(X+1)..X):-d(X),{body}."),
            &EvaluationError::Overflow,
            GroundingPhase::SupportCompletion,
        );
    }
}

#[test]
fn unsafe_pool_siblings_keep_shared_family_evidence() {
    for hybrid in [false, true] {
        let observer = Observer::default();
        assert_eq!(
            ground("d(0). :- d(X),X/(1;X)=0.", hybrid, &observer).unwrap(),
            1
        );
        assert!(
            family_pass(&observer)
                .work
                .join_rows
                .is_some_and(|count| count > 0)
        );
    }
}

#[test]
fn siblings_without_division_retain_family_validation() {
    failure(
        "d(1). :- d(X),(X/2;X+2147483647)=0.",
        &EvaluationError::Overflow,
        GroundingPhase::SupportCompletion,
    );
}

#[test]
fn hybrid_capture_uses_computed_domain_selection() {
    let observer = Observer::default();
    let owner = prepare_formula(
        "{p(1..6)}.q(2). :-p(X),q(Y),X/2=Y.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_hybrid_with_observer(Some(&observer))
    .unwrap();
    assert_eq!(owner.streamed_instances(), 2);
    // Indexed is the ordinary hybrid policy. An actual finite-table probe here
    // establishes that capture used the computed necessary domain.
    assert!(observer.records.borrow().iter().any(|record| {
        record.phase == GroundingPhase::RuleInstantiation
            && record.work.table_probes.is_some_and(|probes| probes > 0)
    }));
}
