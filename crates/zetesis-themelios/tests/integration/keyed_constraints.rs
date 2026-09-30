//! A disequality over a keyed value asks for the one atom the key admits.

use crate::support::stable_models;

use std::collections::BTreeSet;

use stable_models::stable;
use zetesis_clingo_support as oracle;
use zetesis_reference_support::{admit, canonical, exhaustive, formula};
use zetesis_themelios::{
    AnalysisBasis, ExpansionFailure, FormulaFailure, FormulaLimits, KeyAnalysis,
    observation::EvaluationError,
};

const CHOICES: &str = "letter(a;b;c). digit(0..9). carry_value(0;1). idx(1). \
    1 { assign(L,D) : digit(D) } 1 :- letter(L). \
    1 { carry(I,V) : carry_value(V) } 1 :- idx(I). ";

/// The two programs ground to theories of the same shape and the same family.
fn same_shape(left: &str, right: &str) {
    let (left, right) = (formula(left), formula(right));
    assert_eq!(left.atoms(), right.atoms());
    assert_eq!(left.theory().roots().len(), right.theory().roots().len());
    assert_eq!(left.theory().nodes().len(), right.theory().nodes().len());
    assert_eq!(stable(&left), stable(&right));
}

#[test]
fn a_disequality_over_one_keyed_value_asks_for_the_one_atom() {
    // X is assigned exactly one digit, so "no digit but Y+1" is "the digit
    // Y+1 is missing": ten constraint instances where the product had ninety.
    let written = format!("{CHOICES} :- assign(a,X), assign(b,Y), X != Y + 1.");
    let asked = format!("{CHOICES} :- assign(b,Y), letter(a), not assign(a, Y + 1).");
    same_shape(&written, &asked);
    assert_eq!(formula(&written).keyed_constraints(), 1);
    assert_eq!(formula(&asked).keyed_constraints(), 0);
    // Nine pairs with X = Y + 1, times the free letter and the carry.
    assert_eq!(stable(&formula(&written)).len(), 9 * 10 * 2);
}

#[test]
fn a_digit_and_carry_column_asks_for_both_atoms() {
    // A + B = C + 10 K with C a digit and K a carry has the one solution
    // C = (A + B) mod 10, K = (A + B) div 10.
    let written = format!(
        "{CHOICES} :- assign(a,A), assign(b,B), assign(c,C), carry(1,K), A + B != C + 10 * K."
    );
    let asked = format!(
        "{CHOICES} :- assign(a,A), assign(b,B), letter(c), idx(1), not assign(c, (A + B) \\ 10). \
         :- assign(a,A), assign(b,B), letter(c), idx(1), not carry(1, (A + B) / 10)."
    );
    same_shape(&written, &asked);
    assert_eq!(stable(&formula(&written)).len(), 100);
}

#[test]
fn the_written_forms_of_the_column_are_all_read() {
    let forms = [
        "A + B != C + 10 * K",
        "A + B != 10 * K + C",
        "C + 10 * K != A + B",
        "K * 10 + C != A + B",
    ];
    let reference = format!(
        "{CHOICES} :- assign(a,A), assign(b,B), letter(c), idx(1), not assign(c, (A + B) \\ 10). \
         :- assign(a,A), assign(b,B), letter(c), idx(1), not carry(1, (A + B) / 10)."
    );
    for form in forms {
        let written =
            format!("{CHOICES} :- assign(a,A), assign(b,B), assign(c,C), carry(1,K), {form}.");
        same_shape(&written, &reference);
    }
}

#[test]
fn a_value_read_elsewhere_keeps_the_written_constraint() {
    // X also decides X > 1, so the constraint is not about one demanded
    // digit; the product form stays, and grounds to ninety instances less
    // the ones X > 1 excludes.
    let written = format!("{CHOICES} :- assign(a,X), assign(b,Y), X != Y + 1, X > 1.");
    let input = formula(&written);
    assert_eq!(input.keyed_constraints(), 0);
    let constraints = input
        .theory()
        .roots()
        .len()
        .saturating_sub(formula(CHOICES).theory().roots().len());
    assert_eq!(constraints, 8 * 10 - 8);
    // Twenty pairs with X <= 1 and eight with X = Y + 1 > 1 remain, times
    // the free letter and the carry.
    assert_eq!(stable(&input).len(), 28 * 10 * 2);
}

const LETTERS: &str = "letter(a;b). val(3;4). 1 { assign(K,V) : val(V) } 1 :- letter(K). ";

#[test]
fn an_anonymous_key_keeps_the_written_constraint() {
    // The written constraint forbids a value other than 3 at any key. Asked
    // by key it would read `not assign(_, 3)`, no key at all holding 3, which
    // forbids less: the key must be named for the one atom to be the key's.
    let anonymous = formula(&format!("{LETTERS} :- assign(_, Y), Y != 3."));
    assert_eq!(anonymous.keyed_constraints(), 0);
    // Both letters hold 3, as they do when the key is named.
    let named = formula(&format!("{LETTERS} :- assign(K, Y), Y != 3."));
    assert_eq!(stable(&anonymous).len(), 1);
    assert_eq!(stable(&anonymous), stable(&named));
}

const COLUMN: &str = "letter(a;b). digit(0..1). carry_value(0;1). idx(1). \
    1 { assign(L,D) : digit(D) } 1 :- letter(L). \
    1 { carry(I,V) : carry_value(V) } 1 :- idx(I). ";

#[test]
fn an_anonymous_key_keeps_the_written_column() {
    // As above for the digit and the carry: 3 = C + 2K over C and K in 0..1
    // has the one solution C = 1, K = 1, and with the digit's key anonymous
    // every letter's digit must be that one, not some letter's.
    let anonymous = formula(&format!(
        "{COLUMN} :- assign(_, C), carry(1, K), 3 != C + 2 * K."
    ));
    assert_eq!(anonymous.keyed_constraints(), 0);
    let named = formula(&format!(
        "{COLUMN} :- assign(L, C), carry(1, K), 3 != C + 2 * K."
    ));
    assert_eq!(stable(&anonymous).len(), 1);
    assert_eq!(stable(&anonymous), stable(&named));
}

#[test]
fn a_relation_with_another_producer_keeps_the_written_constraint() {
    let choices = format!("{CHOICES} assign(a,0) :- not assign(a,1).");
    let written = format!("{choices} :- assign(a,X), assign(b,Y), X != Y + 1.");
    let asked = format!("{choices} :- assign(b,Y), letter(a), not assign(a, Y + 1).");
    let (written, asked) = (formula(&written), formula(&asked));
    assert_eq!(written.keyed_constraints(), 0);
    assert!(written.theory().roots().len() > asked.theory().roots().len());
}

#[test]
fn a_digit_outside_the_carry_base_keeps_the_written_column() {
    // With digits up to 12 the column has more than one solution, so the
    // inversion does not apply and the product form stays.
    let choices = CHOICES.replace("digit(0..9)", "digit(0..12)");
    let written = format!(
        "{choices} :- assign(a,A), assign(b,B), assign(c,C), carry(1,K), A + B != C + 10 * K."
    );
    let asked = format!(
        "{choices} :- assign(a,A), assign(b,B), letter(c), idx(1), not assign(c, (A + B) \\ 10). \
         :- assign(a,A), assign(b,B), letter(c), idx(1), not carry(1, (A + B) / 10)."
    );
    let (written, asked) = (formula(&written), formula(&asked));
    assert_eq!(written.keyed_constraints(), 0);
    assert!(written.theory().roots().len() > asked.theory().roots().len());
    assert_ne!(stable(&written), stable(&asked));
}

#[test]
fn asked_constraints_keep_the_written_constraint_as_their_origin() {
    let written = format!("{CHOICES} :- assign(a,X), assign(b,Y), X != Y + 1.");
    let start = written.find(":- assign(a,X)").unwrap();
    let input = formula(&written);
    let spans: BTreeSet<_> = input
        .formula_origins()
        .iter()
        .flatten()
        .map(|origin| origin.span.start().get())
        .collect();
    assert!(spans.contains(&u32::try_from(start).unwrap()), "{spans:?}");
}

#[test]
#[ignore = "requires clingo: every interpretation of a small asked program matches clingo"]
fn every_interpretation_of_a_small_asked_program_matches_clingo() {
    let source = "letter(a;b). digit(0..1). 1 { assign(L,D) : digit(D) } 1 :- letter(L). \
        :- assign(a,X), assign(b,Y), X != Y.";
    let admitted = formula(source);
    assert_eq!(exhaustive(&admitted), oracle::records(source), "{source}");
}

#[test]
#[ignore = "requires clingo: a constraint with an anonymous key matches clingo as written"]
fn a_constraint_with_an_anonymous_key_matches_clingo_as_written() {
    // The rewrite declines an anonymous key, so the written constraint
    // decides the family: both letters hold 3, and the column has its one
    // solution at every letter.
    for source in [
        format!("{LETTERS} :- assign(_, Y), Y != 3."),
        format!("{COLUMN} :- assign(_, C), carry(1, K), 3 != C + 2 * K."),
    ] {
        assert_eq!(formula(&source).keyed_constraints(), 0, "{source}");
        let family: BTreeSet<_> = stable(&formula(&source))
            .into_iter()
            .map(|atoms| (atoms.iter().map(canonical).collect(), None))
            .collect();
        assert_eq!(family, oracle::records(&source), "{source}");
    }
}

#[test]
#[ignore = "requires clingo: asked constraints match clingo"]
fn asked_constraints_match_clingo() {
    // Four digits keep the families within the oracle's capture; the digits
    // still lie within the column's base.
    let choices = CHOICES.replace("digit(0..9)", "digit(0..3)");
    for source in [
        format!("{choices} :- assign(a,X), assign(b,Y), X != Y + 1."),
        format!(
            "{choices} :- assign(a,A), assign(b,B), assign(c,C), carry(1,K), A + B != C + 10 * K."
        ),
        format!("{choices} :- assign(a,X), carry(1,K), K != X."),
    ] {
        assert_eq!(formula(&source).keyed_constraints(), 1, "{source}");
        let family: BTreeSet<_> = stable(&formula(&source))
            .into_iter()
            .map(|atoms| (atoms.iter().map(canonical).collect(), None))
            .collect();
        assert_eq!(family, oracle::records(&source), "{source}");
    }
}

#[test]
fn the_rest_of_a_keyed_program_is_prepared_once() {
    // The written constraint is compiled once, charged its one origin, and
    // replaced by the asked constraint, charged one more: the receipt exceeds
    // the hand-asked program's by exactly the written constraint's origin,
    // and the facts and choices are charged once.
    let written = format!("{CHOICES} :- assign(a,X), assign(b,Y), X != Y + 1.");
    let asked = format!("{CHOICES} :- assign(b,Y), letter(a), not assign(a, Y + 1).");
    let (written, asked) = (formula(&written), formula(&asked));
    assert_eq!(
        written.expansion_usage().origin_locations,
        asked.expansion_usage().origin_locations + 1
    );
}

#[test]
fn a_digit_produced_by_a_rule_keeps_the_written_column() {
    // The column pattern reads the digits' range from the facts of the
    // condition binding the digit; a digit predicate produced by a rule has
    // no facts to read, so the constraint stays as written, and the answer
    // sets are the same.
    let choices = CHOICES.replace("digit(0..9).", "span(0..9). digit(D) :- span(D).");
    let written = format!(
        "{choices} :- assign(a,A), assign(b,B), assign(c,C), carry(1,K), A + B != C + 10 * K."
    );
    let asked = format!(
        "{choices} :- assign(a,A), assign(b,B), letter(c), idx(1), not assign(c, (A + B) \\ 10). \
         :- assign(a,A), assign(b,B), letter(c), idx(1), not carry(1, (A + B) / 10)."
    );
    let (written, asked) = (formula(&written), formula(&asked));
    assert_eq!(written.keyed_constraints(), 0);
    assert_eq!(stable(&written), stable(&asked));
}

#[test]
fn a_stopped_key_analysis_leaves_every_constraint_written_and_is_reported() {
    // Three steps read three statements and no key: the analysis stops, the
    // constraint is grounded as written, the admission succeeds and the
    // answer sets are those of the asked form. The three steps are charged
    // to the term work: against no step at all the receipt differs by three.
    let written = format!("{CHOICES} :- assign(a,X), assign(b,Y), X != Y + 1.");
    let under = |max_key_work| {
        admit(
            &written,
            &FormulaLimits {
                max_key_work,
                ..FormulaLimits::default()
            },
        )
        .unwrap()
    };
    let stopped = under(3);
    assert_eq!(stopped.keyed_constraints(), 0);
    assert!(matches!(stopped.key_analysis(), KeyAnalysis::Stopped(stop) if stop.limit == 3));
    let complete = formula(&written);
    assert_eq!(complete.keyed_constraints(), 1);
    assert_eq!(complete.key_analysis(), KeyAnalysis::Complete);
    assert_eq!(stable(&stopped), stable(&complete));
    assert_eq!(
        stopped.expansion_usage().term_work,
        under(0).expansion_usage().term_work + 3
    );
}

#[test]
fn a_producer_projected_for_a_pool_yields_no_key() {
    // A pool inside the choice outlives normalization into the analysis
    // input as one pool-free copy of the choice per alternative, each a
    // producer of the relation; with several producers no key is claimed,
    // though the program means one choice.
    let source = "b(1..2). c(1..2). d(1..2). \
        1 { p(K,V) : c(V), d((1;2)) } 1 :- b(K). :- p(K,Y), Y != 1.";
    let admitted = formula(source);
    assert_eq!(
        admitted.analysis_basis(),
        AnalysisBasis::DependencyProjection
    );
    assert_eq!(admitted.keyed_constraints(), 0);
}

#[test]
fn a_projection_elsewhere_leaves_the_keyed_constraint_asked() {
    // The projection copies only the statements with a pool; a keyed
    // choice without one stands in it as written, the relation's one
    // producer.
    let source = "b(1..2). c(1..2). 1 { p(K,V) : c(V) } 1 :- b(K). \
        { q(K) : b(K), c((1;2)) }. :- p(K,Y), Y != 1.";
    let admitted = formula(source);
    assert_eq!(
        admitted.analysis_basis(),
        AnalysisBasis::DependencyProjection
    );
    assert_eq!(admitted.keyed_constraints(), 1);
}

#[test]
fn a_keyed_column_preserves_checked_overflow() {
    let source = "digit(0). carry_value(1073741824). \
        1 { p(Y) : digit(Y) } 1. 1 { q(C) : carry_value(C) } 1. \
        :- p(Y), q(C), 0 != Y + 2*C.";
    for max_key_work in [0, FormulaLimits::default().max_key_work] {
        let Err(failure) = admit(
            source,
            &FormulaLimits {
                max_key_work,
                ..FormulaLimits::default()
            },
        ) else {
            panic!("key work {max_key_work}: admission succeeded; expected arithmetic overflow");
        };
        assert!(
            matches!(
                failure,
                FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                    error: EvaluationError::Overflow,
                    ..
                })
            ),
            "{failure}"
        );
    }
}

#[test]
fn a_keyed_column_keeps_nonnumeric_comparisons_defined() {
    let source = "digit(0;1). carry_value(0;1). \
        1 { p(Y) : digit(Y) } 1. 1 { q(C) : carry_value(C) } 1. \
        :- p(Y), q(C), a != Y + 2*C.";
    // A symbol differs from every numeric sum; no arithmetic operation is
    // applied to the symbol in the source.
    for max_key_work in [0, FormulaLimits::default().max_key_work] {
        let input = admit(
            source,
            &FormulaLimits {
                max_key_work,
                ..FormulaLimits::default()
            },
        )
        .unwrap();
        assert!(stable(&input).is_empty());
    }
}

#[test]
fn a_keyed_comparison_keeps_its_excluded_substitutions_unreached() {
    let source = "val(1). 1 { p(Y) : val(Y) } 1. d(0). \
        :- d(X), p(Y), Y != 1, 1/X=1.";
    let written = admit(
        source,
        &FormulaLimits {
            max_key_work: 0,
            ..FormulaLimits::default()
        },
    )
    .unwrap();
    assert_eq!(
        exhaustive(&written),
        BTreeSet::from([(["val(1)", "p(1)", "d(0)"].map(str::to_owned).into(), None,)]),
    );
    assert_eq!(stable(&formula(source)), stable(&written));
}

fn same_evaluation_failure(source: &str, expected: &EvaluationError) {
    for max_key_work in [0, FormulaLimits::default().max_key_work] {
        let Err(failure) = admit(
            source,
            &FormulaLimits {
                max_key_work,
                ..FormulaLimits::default()
            },
        ) else {
            panic!("key work {max_key_work}: {source}: expected {expected}");
        };
        assert!(
            matches!(&failure, FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                error,
                ..
            }) if error == expected),
            "{failure}"
        );
        assert!(failure.diagnostics().iter().all(|diagnostic| {
            let span = diagnostic.primary().location.span;
            let start = usize::try_from(span.start().get()).unwrap();
            source[start..].starts_with(":-")
        }));
    }
}

#[test]
fn a_keyed_value_checks_every_arithmetic_intermediate() {
    // The mathematical final result fits, but its first addition does not.
    same_evaluation_failure(
        "value(0). input(2147483647). 1 { p(Y) : value(Y) } 1. \
         :- p(Y), input(X), Y != (X+1)-1.",
        &EvaluationError::Overflow,
    );
}

#[test]
fn a_keyed_value_keeps_nonnumeric_arithmetic_errors() {
    same_evaluation_failure(
        "value(0). input(a). 1 { p(Y) : value(Y) } 1. \
         :- p(Y), input(X), Y != X+1.",
        &EvaluationError::Undefined,
    );
}

#[test]
fn a_keyed_value_keeps_negative_exponent_errors() {
    same_evaluation_failure(
        "value(0). input(-1). 1 { p(Y) : value(Y) } 1. \
         :- p(Y), input(X), Y != 2**X.",
        &EvaluationError::Undefined,
    );
}

#[test]
fn a_safe_column_at_the_integer_boundary_is_asked() {
    let source = "digit(1). carry_value(1073741823). \
        1 { p(Y) : digit(Y) } 1. 1 { q(C) : carry_value(C) } 1. \
        :- p(Y), q(C), 2147483647 != Y + 2*C.";
    let written = admit(
        source,
        &FormulaLimits {
            max_key_work: 0,
            ..FormulaLimits::default()
        },
    )
    .unwrap();
    let asked = formula(source);
    assert_eq!(asked.keyed_constraints(), 1);
    assert_eq!(stable(&asked), stable(&written));
    assert_eq!(stable(&asked).len(), 1);
}

#[test]
fn independently_safe_arithmetic_guards_allow_asking() {
    let source = "value(0;1). input(1;2). 1 { p(Y) : value(Y) } 1. \
        :- p(Y), input(X), Y != 1, 1/X=1.";
    let written = admit(
        source,
        &FormulaLimits {
            max_key_work: 0,
            ..FormulaLimits::default()
        },
    )
    .unwrap();
    let asked = formula(source);
    assert_eq!(asked.keyed_constraints(), 1);
    assert_eq!(stable(&asked), stable(&written));
}

#[test]
fn an_arithmetic_bound_from_a_comparison_is_not_assumed() {
    let source = "value(0;1). input(0;1). 1 { p(Y) : value(Y) } 1. \
        :- p(Y), input(X), X>0, Y != 1, 1/X=1.";
    let written = admit(
        source,
        &FormulaLimits {
            max_key_work: 0,
            ..FormulaLimits::default()
        },
    )
    .unwrap();
    let input = formula(source);
    assert_eq!(input.keyed_constraints(), 0);
    assert_eq!(stable(&input), stable(&written));
}
