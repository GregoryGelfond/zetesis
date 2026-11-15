//! Literal divisors defer only the redundant complete-family traversal.

use crate::support::warnings::zero_divisor_statement;
use zetesis_reference_support::prepare;
use zetesis_themelios::{
    ExpansionFailure, FormulaFailure, GroundingOutcome, GroundingPhase,
    observation::EvaluationError,
};

use super::{Observer, Record};

fn ground(source: &str, hybrid: bool, observer: &Observer) -> Result<usize, FormulaFailure> {
    let prepared = prepare(source)?;
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

/// The constraint of `source`, its last statement.
fn constraint(source: &str) -> &str {
    &source[source.find(":-").unwrap()..]
}

/// Both routes admit `source`, one fact and a constraint: the final family
/// pass evaluates the constraint's row and completes, the constraint keeps
/// the warning and contributes no instance.
fn omission(source: &str) {
    for hybrid in [false, true] {
        let observer = Observer::default();
        let prepared = prepare(source).unwrap();
        if hybrid {
            let admitted = prepared
                .ground_hybrid_with_observer(Some(&observer))
                .unwrap();
            assert_eq!(
                zero_divisor_statement(&admitted),
                constraint(source),
                "{source}"
            );
            assert_eq!(admitted.streamed_instances(), 0, "{source}");
        } else {
            let admitted = prepared.ground_with_observer(Some(&observer)).unwrap();
            assert_eq!(
                zero_divisor_statement(&admitted),
                constraint(source),
                "{source}"
            );
            // The fact's root is the only one.
            assert_eq!(admitted.theory().roots().len(), 1, "{source}");
        }
        let pass = family_pass(&observer);
        assert_eq!(pass.outcome, GroundingOutcome::Completed, "{source}");
        assert!(
            pass.work.join_rows.is_some_and(|rows| rows > 0),
            "{source}, hybrid={hybrid}"
        );
        assert!(
            pass.work
                .expression_evaluations
                .is_some_and(|count| count > 0),
            "{source}, hybrid={hybrid}"
        );
    }
}

#[test]
fn possible_zero_divisors_retain_the_family_warning() {
    for source in ["d(0). :- d(X), 1/X=0.", "d(1). :- d(X), X/0=0."] {
        omission(source);
    }
}

#[test]
fn nested_zero_divisors_retain_the_family_warning() {
    omission("d(0). :- d(X), (1/X)/2=0.");
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
fn unsafe_pool_siblings_retain_the_family_warning() {
    // The pool sets the variable divisor X beside the literal divisor 1, so
    // the family pass still reads the constraint's row, where X/X divides by
    // zero. Both routes keep one warning, located at the constraint as
    // written, pool included.
    let source = "d(0). :- d(X),X/(1;X)=0.";
    for hybrid in [false, true] {
        let observer = Observer::default();
        let prepared = prepare(source).unwrap();
        if hybrid {
            let admitted = prepared
                .ground_hybrid_with_observer(Some(&observer))
                .unwrap();
            assert_eq!(zero_divisor_statement(&admitted), constraint(source));
        } else {
            let admitted = prepared.ground_with_observer(Some(&observer)).unwrap();
            assert_eq!(zero_divisor_statement(&admitted), constraint(source));
        }
        assert!(
            family_pass(&observer)
                .work
                .join_rows
                .is_some_and(|count| count > 0),
            "hybrid={hybrid}"
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
    let owner = zetesis_reference_support::prepared("{p(1..6)}.q(2). :-p(X),q(Y),X/2=Y.")
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
