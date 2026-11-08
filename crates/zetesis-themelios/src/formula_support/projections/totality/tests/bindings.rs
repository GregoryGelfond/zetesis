//! Deterministic scalar chains retain every original arithmetic obligation.
use super::*;

fn certificate(source: &str, family: bool) -> bool {
    let mut certified = false;
    testing::with_completed_source(source, |program, support, computation, counters| {
        let rule = program
            .rules_without_facts()
            .iter()
            .find(|rule| !rule.body.is_empty())
            .unwrap();
        assert!(
            rule.body
                .iter()
                .any(|literal| matches!(literal, LiteralIr::Bind { .. }))
        );
        let limits = FormulaLimits::default();
        let mut projections = Projections::new(&Context::new(
            &*computation,
            &limits,
            counters,
            rule.location,
        ))
        .unwrap();
        certified = if family {
            let join = Join::rule(
                rule,
                support,
                computation,
                &limits,
                &mut testing::budget(),
                counters,
            )
            .unwrap();
            assert!(
                crate::formula_support::keys::covered_family(
                    rule,
                    &join.plan,
                    &mut Context::new(&*computation, &limits, counters, rule.location),
                )
                .unwrap()
            );
            projections.total_family(
                rule,
                support,
                &mut Evaluation::default(),
                Context::new(computation, &limits, counters, rule.location),
            )
        } else {
            projections.total_body(
                rule,
                support,
                &mut Evaluation::default(),
                Context::new(computation, &limits, counters, rule.location),
            )
        }
        .unwrap();
    });
    certified
}

#[test]
fn scalar_chains_propagate_checked_integer_hulls() {
    assert!(certificate(
        "d(0..9). :- d(X), K=1+1, Q=10/K, A=X+Q, B=A/2, B>0.",
        false,
    ));
}

#[test]
fn unused_bindings_keep_their_arithmetic_obligations() {
    for source in ["d(0). :- d(X), A=1/X.", "d(2147483647). :- d(X), A=X+1."] {
        assert!(!certificate(source, false), "{source}");
    }
}

#[test]
fn generated_false_comparisons_do_not_certify_errors_away() {
    for source in [
        "d(0). :- d(X), A=X+1, A=0, Z=1/X.",
        "d(2147483647). :- d(X), A=X-X, A=1, Z=X+1.",
    ] {
        assert!(!certificate(source, false), "{source}");
    }
}

#[test]
fn scalar_chains_decline_mixed_or_empty_columns() {
    for source in ["d(a;1). :- d(X), A=X+1, A>0.", ":- missing(X), A=1/X, A>0."] {
        assert!(!certificate(source, false), "{source}");
    }
}

#[test]
fn scalar_chains_check_every_arithmetic_intermediate() {
    assert!(!certificate(
        "d(2147483647). :- d(X), A=X+1, B=A-X, B=1.",
        false,
    ));
}

#[test]
fn constructed_heads_preserve_scalar_family_totality() {
    assert!(certificate("d(1..3). p(f(X,B)) :- d(X), B=X/2.", true,));
}

#[test]
fn body_totality_does_not_certify_head_arithmetic() {
    // The false body keeps this head unreached during support completion.
    // The body proof may succeed; the full-family proof remains conservative.
    let source = "d(2147483647). p(X+1) :- d(X), X=0.";
    assert!(certificate(source, false));
    assert!(!certificate(source, true));
}

#[test]
fn generated_hulls_do_not_publish_empty_projection_domains() {
    testing::with_completed_source(
        "d(1..3). :- d(X), A=X+1, A/2=1.",
        |program, support, computation, counters| {
            let rule = constraints(program).next().unwrap();
            let limits = FormulaLimits::default();
            let mut projections = Projections::new(&Context::new(
                &*computation,
                &limits,
                counters,
                rule.location,
            ))
            .unwrap();
            assert!(
                projections
                    .total_body(
                        rule,
                        support,
                        &mut Evaluation::default(),
                        Context::new(computation, &limits, counters, rule.location),
                    )
                    .unwrap()
            );
            let target = rule
                .body
                .iter()
                .find_map(|literal| match literal {
                    LiteralIr::Bind { target, .. } => Some(*target),
                    _ => None,
                })
                .unwrap();
            let projection = projections
                .values
                .iter()
                .find(|projection| projection.slot == target)
                .expect("the generated unary operand owns a memo descriptor");
            assert_eq!(projection.covered, None);
        },
    );
}
