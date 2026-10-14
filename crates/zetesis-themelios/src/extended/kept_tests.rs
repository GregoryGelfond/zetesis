//! A kept statement against its rewrite, on both routes.
//!
//! Each program is admitted twice on this thread: once with every statement
//! rewritten, the reference path, and once as the kept-statement predicate
//! allows. Both must give the same result and charge the same work, and a
//! failing program must fail the same way at the same charge. The statements
//! the predicate keeps are counted, so each case also asserts which path it
//! took.

use super::testing::{kept_statements, with_every_statement_rewritten};
use crate::{AdmissionOptions, ExpansionLimits, FormulaLimits};

/// What the extended route's two paths must agree on: the emitted templates,
/// compared as templates since the program's own rendering includes hash
/// indexes whose order differs between programs, their origins and the
/// receipt, or the failure.
fn extended(text: &str, limits: &ExpansionLimits) -> Result<crate::Admitted, String> {
    crate::admit_extended(text.to_owned(), AdmissionOptions::default(), *limits)
        .map_err(|failure| format!("{failure:?}"))
}

fn extended_agree(text: &str, limits: &ExpansionLimits) -> usize {
    let reference = with_every_statement_rewritten(|| extended(text, limits));
    let before = kept_statements();
    let kept = extended(text, limits);
    match (&kept, &reference) {
        (Ok(kept), Ok(reference)) => {
            assert!(
                zetesis_test_support::programs::rules(kept.program())
                    .eq(zetesis_test_support::programs::rules(reference.program())),
                "{text} under {limits:?}: templates differ",
            );
            assert_eq!(
                kept.template_origins(),
                reference.template_origins(),
                "{text}"
            );
            assert_eq!(kept.fact_origins(), reference.fact_origins(), "{text}");
            assert_eq!(
                kept.expansion_usage(),
                reference.expansion_usage(),
                "{text}"
            );
        }
        (Err(kept), Err(reference)) => assert_eq!(kept, reference, "{text} under {limits:?}"),
        _ => panic!("{text} under {limits:?}: one path failed: {kept:?} against {reference:?}"),
    }
    kept_statements() - before
}

/// What the formula route's two paths must agree on: the analyzed program,
/// whose `Debug` rendering carries provenance at every nesting level, the
/// IR's origins, the theory and the receipt, or the failure.
fn formula(text: &str, limits: &ExpansionLimits) -> String {
    match crate::admit_formula(
        text.to_owned(),
        AdmissionOptions::default(),
        *limits,
        FormulaLimits::default(),
    ) {
        Ok(admitted) => format!(
            "analyzed {:?}\norigins {:?}\ntheory {:?}\nusage {:?}",
            admitted.analyzed_program(),
            admitted.formula_origins(),
            admitted.theory(),
            admitted.expansion_usage(),
        ),
        Err(failure) => format!("{failure:?}"),
    }
}

/// Admit `text` on the formula route both ways and require the same
/// rendering; the statements the predicate kept on the second admission.
fn formula_agree(text: &str, limits: &ExpansionLimits) -> usize {
    let reference = with_every_statement_rewritten(|| formula(text, limits));
    let before = kept_statements();
    let kept = formula(text, limits);
    if let Some(at) = kept
        .bytes()
        .zip(reference.bytes())
        .position(|(a, b)| a != b)
    {
        let from = at.saturating_sub(160);
        panic!(
            "{text} under {limits:?}: the paths differ at byte {at}\n  kept:      {}\n  rewritten: {}",
            &kept[from..(at + 160).min(kept.len())],
            &reference[from..(at + 160).min(reference.len())],
        );
    }
    assert_eq!(kept.len(), reference.len(), "{text} under {limits:?}");
    kept_statements() - before
}

const KEPT_FACTS: [&str; 5] = [
    r#"p(a). p("s"). p(3). p(-4)."#,
    // Alternatives of an atom's arguments, each of them scalars.
    "p(1; 2). q(f(a); g(b)).",
    r#"p(f(a, g(b, "x"))). p(f(-1, -a))."#,
    r#"p((1, 2)). p(f((a, "x"), 3)). p(((a,), b))."#,
    "p(f(a), b). q(g(h(c))).",
];

const NOT_KEPT: [&str; 6] = [
    "#const c = 5. p(c).",
    "#const c = 5. p(f(c)).",
    "#const c = 5. p(f((a, c))).",
    "p(f(1; 2)).",
    "p(1..3).",
    "p(1 + 2). q(f(1 + 2)).",
];

#[test]
fn kept_facts_match_their_rewrite_on_the_extended_route() {
    for text in KEPT_FACTS {
        assert!(
            extended_agree(text, &ExpansionLimits::default()) > 0,
            "{text}"
        );
    }
}

#[test]
fn kept_facts_match_their_rewrite_on_the_formula_route() {
    for text in KEPT_FACTS {
        assert!(
            formula_agree(text, &ExpansionLimits::default()) > 0,
            "{text}"
        );
    }
}

#[test]
fn a_rule_with_a_compound_symbol_is_kept_on_the_extended_route() {
    let text = "p(f(a)). q(X) :- p(X), X = f(a).";
    assert_eq!(extended_agree(text, &ExpansionLimits::default()), 2);
}

#[test]
fn only_fact_statements_are_kept_on_the_formula_route() {
    // The rule's terms are leaves, but its analyzed statement is rebuilt from
    // the rewrite, so the formula route keeps the fact alone.
    let text = "p(f(a)). q(X) :- p(X), X = f(a).";
    assert_eq!(formula_agree(text, &ExpansionLimits::default()), 1);
}

#[test]
fn sentinel_facts_are_kept_on_the_formula_route() {
    let text = "p(#inf). p(#sup). q(f(#sup)).";
    assert_eq!(formula_agree(text, &ExpansionLimits::default()), 3);
}

#[test]
fn definitions_pools_intervals_and_arithmetic_are_rewritten_on_both_routes() {
    for text in NOT_KEPT {
        assert_eq!(
            extended_agree(text, &ExpansionLimits::default()),
            0,
            "{text}"
        );
        assert_eq!(
            formula_agree(text, &ExpansionLimits::default()),
            0,
            "{text}"
        );
    }
}

#[test]
fn a_kept_statement_after_a_rewritten_one_matches_on_both_routes() {
    let text = "#const c = 5. p(c). p(f(a, b)). p(f(c)).";
    assert_eq!(extended_agree(text, &ExpansionLimits::default()), 1);
    assert_eq!(formula_agree(text, &ExpansionLimits::default()), 1);
}

#[test]
fn a_term_work_ceiling_fails_at_the_same_charge_on_both_paths() {
    let text = r#"p(f(a, g(b, "x"))). p((1, (2, c))). p(h)."#;
    for max_term_work in 0..40 {
        let limits = ExpansionLimits {
            max_term_work,
            ..ExpansionLimits::default()
        };
        extended_agree(text, &limits);
        formula_agree(text, &limits);
    }
}

#[test]
fn a_scalar_byte_ceiling_fails_at_the_same_charge_on_both_paths() {
    let text = r#"p(f(a, g(b, "xyz"))). p((1, (2, c))). p(h)."#;
    for max_scalar_bytes in 0..120 {
        let limits = ExpansionLimits {
            max_scalar_bytes,
            ..ExpansionLimits::default()
        };
        extended_agree(text, &limits);
        formula_agree(text, &limits);
    }
}
