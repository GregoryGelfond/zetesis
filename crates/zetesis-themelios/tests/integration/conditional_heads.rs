//! Conditional heads preserve finite existential scope and original reducts.

use crate::support::finite_bindings as reference;
use zetesis_reference_support::{admit, canonical, formula, resource_limits};
use zetesis_test_support::thresholds::first_success;

use crate::support::finite_bindings::expected;
use reference::{external, holds, native, values};
use std::collections::BTreeSet;
use zetesis_clingo_support as oracle;
use zetesis_themelios::{FormulaFailure, FormulaLimits, FormulaResource};

const CASES: &[(&str, &[&[&str]])] = &[
    ("a:#false;b.", &[&["b"]]),
    ("a:1=2;b.", &[&["b"]]),
    ("a:1=1;b.", &[&["a"], &["b"]]),
    ("a:q;b.{q}.", &[&["b"], &["q", "b"], &["q", "a"]]),
    ("#true:q;b.{q}.", &[&["b"], &["q"]]),
    ("a:q. q:-a.", &[]),
    ("a:q;b. q:-a.", &[&["b"]]),
    ("a(X):missing(X);b.", &[&["b"]]),
    (
        "d(1..2).a(X):d(X).",
        &[&["d(1)", "d(2)", "a(1)"], &["d(1)", "d(2)", "a(2)"]],
    ),
    (
        "{p}.h(N):N=0..1|z:-N=#count{1:p}.",
        &[&["h(0)"], &["z"], &["p", "h(1)"], &["p", "z"]],
    ),
];

/// A head that divides by zero in each of its instances under a condition
/// binding is true there: the disjunct holds wherever its condition does and
/// derives no atom, as a written `#true : condition` does. A defined
/// instance beside an undefined one is kept, and a condition that divides by
/// zero has no instance.
const UNDEFINED_HEADS: &[(&str, &[&[&str]])] = &[
    ("d(0).p(1/X):d(X).", &[&["d(0)"]]),
    ("d(0;1).p(1/X):d(X).", &[&["d(0)", "d(1)"]]),
    (
        "{d(0)}.d(1).p(1/X):d(X).",
        &[&["d(0)", "d(1)"], &["d(1)", "p(1)"]],
    ),
    (
        "{d(0)}.d(1).r|p(1/X):d(X).",
        &[&["d(0)", "d(1)"], &["d(1)", "p(1)"], &["d(1)", "r"]],
    ),
    (
        "d(0;1).{a}.p(1/X):d(X):-a.",
        &[&["d(0)", "d(1)"], &["d(0)", "d(1)", "a"]],
    ),
    (
        "{d(0;1)}.{q(1)}.not q(1/X):d(X).",
        &[
            &["d(0)"],
            &["d(0)", "d(1)"],
            &["d(0)", "d(1)", "q(1)"],
            &["d(0)", "q(1)"],
            &["d(1)"],
        ],
    ),
    // The divisors 0 and 1: one instance is undefined and the other is kept.
    ("{d(1)}.p(2/(X-1..X)):d(X).", &[&["d(1)", "p(2)"]]),
    (
        "{d(0;1)}.p(2/X):d(X)|q(2/Y):d(Y),Y<1.",
        &[&["d(0)"], &["d(0)", "d(1)"], &["d(1)", "p(2)"]],
    ),
    ("d(0;1).p(X):d(X),1/X=1.", &[&["d(0)", "d(1)", "p(1)"]]),
    (
        "{d(0;1)}.e(2).p(Z/X):d(X):-e(Z).",
        &[
            &["d(0)", "d(1)", "e(2)"],
            &["d(0)", "e(2)"],
            &["d(1)", "e(2)", "p(2)"],
        ],
    ),
    // A condition that binds the divisor by an interval. Each of its values
    // is a binding of its own, and the head is true under the one at which
    // it divides by zero.
    ("{d(1)}.p(2/Y):d(X),Y=0..X.", &[&["d(1)"]]),
    (
        "{d(1;2)}.p(2/Y):d(X),Y=0..X.",
        &[&["d(1)"], &["d(1)", "d(2)"], &["d(2)"]],
    ),
    ("{d(1)}.p(2/(Y-1)):d(X),Y=1..2.", &[&["d(1)"]]),
    ("{d(2)}.p(4/(Y-X)):d(X),Y=1..3.", &[&["d(2)"]]),
    ("{d(1)}.q|p(2/Y):d(X),Y=0..X.", &[&["d(1)"], &["q"]]),
    ("p(2/Y):Y=0..1.", &[&[]]),
    // A pooled head. Its alternatives are read together under one binding:
    // one that is defined there is kept, and the head is true only where
    // none is.
    ("{d(0)}.p(1/X;2):d(X).", &[&["d(0)", "p(2)"]]),
    ("{d(0)}.p(1/X;2/X):d(X).", &[&["d(0)"]]),
    (
        "{d(0)}.q|p(1/X;2):d(X).",
        &[&["d(0)", "p(2)"], &["d(0)", "q"], &["q"]],
    ),
];

fn cases() -> impl Iterator<Item = &'static (&'static str, &'static [&'static [&'static str]])> {
    CASES.iter().chain(UNDEFINED_HEADS)
}

#[test]
fn complete_families_preserve_conditional_eligibility() {
    for &(source, records) in cases() {
        assert_eq!(native(&formula(source)), expected(records), "{source}");
    }
}

#[test]
fn an_undefined_head_warns_once() {
    for &(source, _) in UNDEFINED_HEADS {
        assert_eq!(formula(source).warnings().len(), 1, "{source}");
    }
}

#[test]
fn membership_matches_independent_subset_enumeration() {
    reference::stability_matches_exhaustion(cases().map(|&(source, _)| source), formula);
}

#[test]
fn original_occurrences_keep_their_source() {
    let source = "{q}.a:q;b.";
    let admitted = formula(source);
    assert_eq!(admitted.source().expect("source input").text(), source);
    assert!(!admitted.formula_origins().is_empty());
    for origins in admitted.formula_origins() {
        assert!(!origins.is_empty());
        for origin in origins {
            assert!(
                admitted
                    .source()
                    .expect("source input")
                    .slice(origin.location().expect("parsed source").span)
                    .is_ok()
            );
        }
    }
}

#[test]
#[ignore = "requires clingo: original sources match clingo full models"]
fn original_sources_match_clingo_full_models() {
    let mut models = 0;
    for &(source, records) in cases() {
        let actual = oracle::objective_free_models(&external(source, true));
        assert_eq!(actual, expected(records), "{source}");
        assert_eq!(native(&formula(source)), actual, "{source}");
        models += actual.len();
    }
    println!("complete_sources={} full_models={models}", cases().count());
}

#[derive(Clone)]
enum Formula {
    Boolean(bool),
    Atom(usize),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
    Implies(Box<Self>, Box<Self>),
}
impl Formula {
    fn truth(&self, tested: usize, frozen: Option<usize>) -> bool {
        if frozen.is_some_and(|outer| !self.truth(outer, None)) {
            return false;
        }
        match self {
            Self::Boolean(value) => *value,
            Self::Atom(atom) => tested & (1 << atom) != 0,
            Self::And(a, b) => a.truth(tested, frozen) && b.truth(tested, frozen),
            Self::Or(a, b) => a.truth(tested, frozen) || b.truth(tested, frozen),
            Self::Implies(a, b) => !a.truth(tested, frozen) || b.truth(tested, frozen),
        }
    }
    fn neg(self) -> Self {
        Self::Implies(Box::new(self), Box::new(Self::Boolean(false)))
    }
    fn or(self, other: Self) -> Self {
        Self::Or(Box::new(self), Box::new(other))
    }
    fn signed(self, count: usize) -> Self {
        (0..count).fold(self, |value, _| value.neg())
    }
}

#[test]
fn conditional_instances_preserve_every_frozen_world() {
    let names = ["a", "b", "q"];
    for head_sign in 0..3 {
        for condition in 0..5 {
            let (condition_source, condition) = match condition {
                0 => ("q".to_owned(), Formula::Atom(2)),
                1 => ("not q".into(), Formula::Atom(2).neg()),
                2 => ("not not q".into(), Formula::Atom(2).neg().neg()),
                3 => ("#false".into(), Formula::Boolean(false)),
                _ => ("1=1".into(), Formula::Boolean(true)),
            };
            let source = format!(
                "{{a;b;q}}.{}a:{condition_source};b.",
                "not ".repeat(head_sign)
            );
            let admitted = formula(&source);
            let actual_names: Vec<_> = admitted.atoms().iter().map(canonical).collect();
            assert_eq!(
                actual_names
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                names.into()
            );
            let remap = |mask: usize| {
                names.iter().enumerate().fold(0, |result, (index, name)| {
                    result
                        | (usize::from(mask & (1 << index) != 0)
                            << actual_names.iter().position(|value| value == name).unwrap())
                })
            };
            let implication = Formula::Implies(
                Box::new(condition.clone()),
                Box::new(Formula::Atom(0).signed(head_sign)),
            );
            let conditional = Formula::And(Box::new(implication), Box::new(condition.neg().neg()))
                .or(Formula::Atom(1));
            let mut original: Vec<_> = (0..3)
                .map(|atom| Formula::Atom(atom).or(Formula::Atom(atom).neg()))
                .collect();
            original.push(conditional);
            for outer in 0..8 {
                let frozen = values(admitted.theory(), remap(outer), None);
                assert_eq!(
                    holds(admitted.theory(), &frozen),
                    original.iter().all(|f| f.truth(outer, None)),
                    "{source}: M={outer}"
                );
                for inner in 0..8 {
                    assert_eq!(
                        holds(
                            admitted.theory(),
                            &values(admitted.theory(), remap(inner), Some(&frozen))
                        ),
                        original.iter().all(|f| f.truth(inner, Some(outer))),
                        "{source}: M={outer} J={inner}"
                    );
                }
            }
        }
    }
}

#[test]
fn private_head_slots_do_not_capture_outer_suffixes() {
    let source = "d(1..2).{p}.u(1..2)|h(X+10):d(X),X=N+1:-N=#count{1:p}.";
    let expanded = "d(1..2).{p}.u(1)|h(X+10):d(X),X=1:-0=#count{1:p}.u(2)|h(X+10):d(X),X=1:-0=#count{1:p}.u(1)|h(X+10):d(X),X=2:-1=#count{1:p}.u(2)|h(X+10):d(X),X=2:-1=#count{1:p}.";
    assert_eq!(native(&formula(source)), native(&formula(expanded)));
}

#[test]
fn local_conditions_cannot_bind_the_outer_rule() {
    for source in ["a(X):q(X);b(X).", "a(X):not q(X);b."] {
        assert!(
            matches!(
                admit(source, &FormulaLimits::default()),
                Err(FormulaFailure::UnsafeVariable { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn local_instance_limit_is_inclusive() {
    let source = "d(1..3).a(X):d(X).";
    let admitted = admit(
        source,
        &resource_limits::formula(FormulaResource::DisjunctionElements, 3),
    )
    .unwrap();
    assert_eq!(native(&admitted), native(&formula(source)));
    assert!(matches!(
        admit(
            source,
            &resource_limits::formula(FormulaResource::DisjunctionElements, 2)
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::DisjunctionElements,
            limit: 2,
            observed: 3,
            ..
        })
    ));
}

#[test]
fn an_undefined_head_counts_as_a_disjunction_element() {
    // Three condition bindings, each with a head that divides by zero.
    let source = "d(0).e(1..3).p(Y/X):d(X),e(Y).";
    let admitted = admit(
        source,
        &resource_limits::formula(FormulaResource::DisjunctionElements, 3),
    )
    .unwrap();
    assert_eq!(native(&admitted), native(&formula(source)));
    assert!(matches!(
        admit(
            source,
            &resource_limits::formula(FormulaResource::DisjunctionElements, 2)
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::DisjunctionElements,
            limit: 2,
            observed: 3,
            ..
        })
    ));
}

#[test]
fn work_refusal_never_publishes_a_prefix() {
    let source = "d(1..2).a(X):d(X).";
    let attempt = |work| {
        admit(
            source,
            &resource_limits::formula(FormulaResource::Work, work),
        )
    };
    let lower = first_success(
        resource_limits::formula_default(FormulaResource::Work),
        |middle| attempt(middle).is_ok(),
    );
    assert_eq!(native(&attempt(lower).unwrap()), native(&formula(source)));
    assert!(
        matches!(attempt(lower-1), Err(FormulaFailure::Limit { resource: FormulaResource::Work, limit, observed, .. })
        if limit == u128::from(lower-1) && observed == u128::from(lower))
    );
}
