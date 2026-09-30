//! Universal rows and existential consequent alternatives have separate scopes.
use crate::support::finite_bindings as reference;
use crate::support::formula_trees::{Formula, world};
use crate::support::upstream;
use crate::support::witnesses::limited;
use reference::{Models, exhaustive, native, values};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use zetesis_reference_support::{canonical, formula};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, AnalysisBasis, ExpansionLimits, FormulaFailure,
    FormulaLimits, admit_formula, prepare_formula,
};

fn corpus_case() -> &'static zetesis_validation::curated::Case {
    upstream::corpus()
        .cases()
        .iter()
        .find(|case| case.id() == "lparse/conjunction/07")
        .unwrap()
}
fn corpus_models() -> Models {
    corpus_case()
        .contract()
        .full_models()
        .iter()
        .map(|model| model.iter().cloned().collect())
        .collect()
}
// Independent finite substitutions. OR consequents distribute into the complete
// rule family below; conditions retain their own universal rows.
const CASES: &[(&str, &str)] = &[
    ("q:-not p(f(_,(1;2))):#true.", "q."),
    (
        "{p(f(1,1));p(f(2,2))}.q:-not p(f(_,(1;2))):#true.",
        "{p(f(1,1));p(f(2,2))}.q:-not p(f(1,1)).q:-not p(f(2,2)).",
    ),
    (
        "{p(f(1,1));p(f(2,2))}.q:-not not p(f(_,(1;2))):#true.",
        "{p(f(1,1));p(f(2,2))}.q:-not not p(f(1,1)).q:-not not p(f(2,2)).",
    ),
    ("q(N):-N=#count{};not p(N,_):d.", "q(0)."),
    (
        "{d;p(0,1)}.q(N):-N=#count{};not p(N,_):d.",
        "{d;p(0,1)}.q(0):-not d.q(0):-not p(0,1).",
    ),
    (
        "{p(f(1,1));p(f(2,2))}.q:-not p(f(_,X..X+1)):X=1.",
        "{p(f(1,1));p(f(2,2))}.q:-not p(f(1,1)).q:-not p(f(2,2)).",
    ),
    (
        "{p(f(1,1));p(f(2,2))}.q:-not not p(f(_,X..X+1)):X=1.",
        "{p(f(1,1));p(f(2,2))}.q:-not not p(f(1,1)).q:-not not p(f(2,2)).",
    ),
    (
        "{p(f(1,1));p(f(2,2))}.q:-not p((f(_,1);f(_,2))):#true.",
        "{p(f(1,1));p(f(2,2))}.q:-not p(f(1,1)).q:-not p(f(2,2)).",
    ),
    ("q:-not p(f(_,2..1)):#true.", ""),
    ("q:-not not p(f(_,2..1)):#true.", ""),
    (
        "{p(f(1));p(f(2))}.d(9).q(X):-not p(f(_)),X=9,d(X).",
        "{p(f(1));p(f(2))}.d(9).q(9):-not p(f(1)),not p(f(2)).",
    ),
    (
        "{p(f(1));p(f(2));p(g(1))}.q:-not p(f(_)):#true.",
        "{p(f(1));p(f(2));p(g(1))}.q:-not p(f(1)),not p(f(2)).",
    ),
    (
        "{p(f(1));p(f(2))}.q:-not not p(f(_)):#true.",
        "{p(f(1));p(f(2))}.q:-not not p(f(1)).q:-not not p(f(2)).",
    ),
    (
        "{p((1,));p((2,));p(())}.q:-not p((_,)):#true.",
        "{p((1,));p((2,));p(())}.q:-not p((1,)),not p((2,)).",
    ),
    (
        "{p(f(1,1));p(f(2,2))}.q:-not p(f(_,X+1)):X=0.",
        "{p(f(1,1));p(f(2,2))}.q:-not p(f(1,1)).",
    ),
    (
        "{p(f(1,2));p(f(2,1))}.q:-not p(f(_,_)):#true.",
        "{p(f(1,2));p(f(2,1))}.q:-not p(f(1,2)),not p(f(2,1)).",
    ),
    (
        "{p(f(1));p(f(2));p(g(1))}.q:-not p(f(_)).",
        "{p(f(1));p(f(2));p(g(1))}.q:-not p(f(1)),not p(f(2)).",
    ),
    (
        "{p(f(1));p(f(2))}.q:-not not p(f(_)).",
        "{p(f(1));p(f(2))}.q:-not not p(f(1)).q:-not not p(f(2)).",
    ),
    (
        "{p(f(1));p(f(2))}.d(9).q(X):-not p(f(_)),d(X).",
        "{p(f(1));p(f(2))}.d(9).q(9):-not p(f(1)),not p(f(2)).",
    ),
    ("q:-not p(_):#true.", "q."),
    ("q:-not not p(_):#true.", ""),
    (
        "{p(1..2)}.q:-not p(_):#true.",
        "{p(1..2)}.q:-not p(1),not p(2).",
    ),
    (
        "{p(1..2)}.q:-not not p(_):#true.",
        "{p(1..2)}.q:-not not p(1).q:-not not p(2).",
    ),
    (
        "{p(1,2);p(2,1)}.q:-not p(_,_):#true.",
        "{p(1,2);p(2,1)}.q:-not p(1,2),not p(2,1).",
    ),
    (
        "{p(1,1);p(2,1)}.q:-not p(_,X+1):X=0.",
        "{p(1,1);p(2,1)}.q:-not p(1,1),not p(2,1).",
    ),
    (
        "{p(1,1);p(2,2)}.q:-not p(_,(1;2)):#true.",
        "{p(1,1);p(2,2)}.q:-not p(1,1).q:-not p(2,2).",
    ),
    (
        "{p(1,1);p(2,2)}.q:-not p(_,X..X+1):X=1.",
        "{p(1,1);p(2,2)}.q:-not p(1,1).q:-not p(2,2).",
    ),
    (
        "{p(1,1);p(2,2)}.q:-not p(_,X):X=1..2.",
        "{p(1,1);p(2,2)}.q:-not p(1,1),not p(2,2).",
    ),
    ("{p(1)}.q:-not p(_):#false.", "{p(1)}.q."),
    ("{p(1)}.q:-not not p(_):#false.", "{p(1)}.q."),
    ("q:-not p(_,X):X=2..1.", "q."),
    ("q:-not p(_,2..1):#true.", ""),
    ("q:-not not p(_,2..1):#true.", ""),
    ("p(1):-q.q:-not p(_):#true.", "p(1):-q.q:-not p(1)."),
    ("p(1):-q.q:-not not p(_):#true.", "p(1):-q.q:-not not p(1)."),
    (
        "{p(1,1);p(2,2)}.d(1..2).q(X):-d(X);not p(_,X):#true.",
        "{p(1,1);p(2,2)}.d(1..2).q(1):-not p(1,1).q(2):-not p(2,2).",
    ),
    (
        "{p(-1);p(-2)}.q:-p(-(X..X+1)):X=1.",
        "{p(-1);p(-2)}.q:-p(-1).q:-p(-2).",
    ),
    (
        "{p(0);p(1)}.q:-p(|(X..X+1)|):X=-1.",
        "{p(0);p(1)}.q:-p(0).q:-p(1).",
    ),
    (
        "{p(f(1));p(f(2))}.q:-p(--f(X..X+1)):X=1.",
        "{p(f(1));p(f(2))}.q:-p(f(1)).q:-p(f(2)).",
    ),
    (
        "{p((1,));p((2,))}.q:-p((X..X+1,)):X=1.",
        "{p((1,));p((2,))}.q:-p((1,)).q:-p((2,)).",
    ),
    (
        "{p(f(1,-1));p(f(1,-2));p(f(3,-1));p(f(3,-2))}.q:-p(f(2*(X..X+1)-1,-(X..X+1))):X=1.",
        "{p(f(1,-1));p(f(1,-2));p(f(3,-1));p(f(3,-2))}.q:-p(f(1,-1)).q:-p(f(1,-2)).q:-p(f(3,-1)).q:-p(f(3,-2)).",
    ),
    (
        "{p((1,()));p((2,()))}.q:-p((X..X+1,())):X=1.",
        "{p((1,()));p((2,()))}.q:-p((1,())).q:-p((2,())).",
    ),
    ("{p(1..2)}.q:-p(X):#true.", "{p(1..2)}.q:-p(1).q:-p(2)."),
    ("{p(1..2)}.q:-p(X):X=1..2.", "{p(1..2)}.q:-p(1),p(2)."),
    (
        "{p(1..3)}.q:-p(X;X+1):X=1..2.",
        "{p(1..3)}.q:-p(1),p(2).q:-p(1),p(3).q:-p(2).",
    ),
    (
        "{p(1..2)}.q:-not p(1..2):#true.",
        "{p(1..2)}.q:-not p(1).q:-not p(2).",
    ),
    (
        "{p(1..2)}.q:-not not p(1..2):#true.",
        "{p(1..2)}.q:-not not p(1).q:-not not p(2).",
    ),
    (
        "{p(1..3)}.q:-p(X-1;2*(X..X+1)-3):X=2.",
        "{p(1..3)}.q:-p(1).q:-p(3).",
    ),
    ("{p(1..2)}.q:-p(2..1):#true.", "{p(1..2)}."),
    ("{p(1..2)}.q:-not p(2..1):#true.", "{p(1..2)}."),
    ("{p(1..2)}.q:-not not p(2..1):#true.", "{p(1..2)}."),
    ("{p(1..2)}.q:-p(X):X=2..1.", "{p(1..2)}.q."),
    (
        "p(1).s(2).q:-p(X):#true;s(X):#true.",
        "p(1).s(2).q:-p(1),s(2).",
    ),
    ("q:-p(X):#false.", "q."),
    ("{p(1..2)}.q:-p(_):#true.", "{p(1..2)}.q:-p(1).q:-p(2)."),
    (
        "{p(1..2,1..2)}.q:-p((1;2),(1;2)):#true.",
        "{p(1..2,1..2)}.q:-p(1,1).q:-p(1,2).q:-p(2,1).q:-p(2,2).",
    ),
    (
        "{p(1);p(1,2)}.q:-p(1;1,2):#true.",
        "{p(1);p(1,2)}.q:-p(1).q:-p(1,2).",
    ),
    (
        "{-p(1..2)}.q:-not -p(1;2):#true.",
        "{-p(1..2)}.q:-not -p(1).q:-not -p(2).",
    ),
    (
        "{p(f(1));p(f(2))}.q:-p(f(X..X+1)):X=1.",
        "{p(f(1));p(f(2))}.q:-p(f(1)).q:-p(f(2)).",
    ),
    (
        "{p(-f(1));p(-f(2))}.q:-p(-f(X..X+1)):X=1.",
        "{p(-f(1));p(-f(2))}.q:-p(-f(1)).q:-p(-f(2)).",
    ),
    (
        "{p(1);p(2)}.q:-p(2*(1..2)-2):#true.",
        "{p(1);p(2)}.q:-p(0).q:-p(2).",
    ),
    ("{p(1..2)}.q:-p(X;X;X+1):X=1.", "{p(1..2)}.q:-p(1).q:-p(2)."),
    (
        "{p(1..2)}.q(a;b):-p(1;2):#true.",
        "{p(1..2)}.q(a):-p(1).q(a):-p(2).q(b):-p(1).q(b):-p(2).",
    ),
];

#[test]
fn conjunction_case_matches_retained_models() {
    let case = corpus_case();
    let source = case.source();
    let actual = native(&formula(source));
    assert_eq!(actual.len(), 4);
    assert_eq!(actual, corpus_models());
}

#[test]
fn finite_substitutions_preserve_models() {
    for &(source, expanded) in CASES {
        let actual = formula(source);
        assert_eq!(native(&actual), native(&formula(expanded)), "{source}");
        assert_eq!(native(&actual), exhaustive(&actual), "{source}");
    }
}

fn original_rule(program: &AdmittedFormula, head: &str) -> usize {
    let atom = program
        .atoms()
        .iter()
        .position(|atom| canonical(atom) == head)
        .unwrap();
    let matches: Vec<_> = program
        .theory()
        .roots()
        .iter()
        .copied()
        .filter(|root| {
            let zetesis_ferraris::Node::Implies(_, consequent) = program.theory().nodes()[*root]
            else {
                return false;
            };
            program.theory().nodes()[consequent] == zetesis_ferraris::Node::Atom(atom)
        })
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "one original implication; producer guards are separate"
    );
    matches[0]
}

#[test]
fn consequent_rows_preserve_frozen_formulas() {
    let mut pairs = 0;
    for (polarity, sign) in ["", "not ", "not not "].into_iter().enumerate() {
        for (condition_polarity, condition_sign) in ["", "not ", "not not "].into_iter().enumerate()
        {
            let source =
                format!("{{p(1..3);c(1..2)}}.q:-{sign}p(X;X+1):X=1..2,{condition_sign}c(X).");
            let program = formula(&source);
            let mut rows = Formula::truth();
            for x in [1, 2] {
                let consequent = Formula::Or(
                    Box::new(Formula::atom(format!("p({x})")).sign(polarity)),
                    Box::new(Formula::atom(format!("p({})", x + 1)).sign(polarity)),
                );
                rows = Formula::and(
                    rows,
                    Formula::implies(
                        Formula::atom(format!("c({x})")).sign(condition_polarity),
                        consequent,
                    ),
                );
            }
            let specified = Formula::implies(rows, Formula::atom("q"));
            pairs += compare_root(&program, &specified);
        }
    }
    println!("frozen_pairs={pairs}");
    assert_eq!(pairs, 9 * 4096);
}

fn compare_root(program: &AdmittedFormula, specified: &Formula) -> usize {
    let root = original_rule(program, "q");
    assert!(program.atoms().len() <= 6);
    let mut pairs = 0;
    for outer in 0..1 << program.atoms().len() {
        let original = values(program.theory(), outer, None);
        let outer_world = world(program.atoms(), outer);
        assert_eq!(original[root], specified.original(&outer_world));
        for inner in 0..1 << program.atoms().len() {
            assert_eq!(
                values(program.theory(), inner, Some(&original))[root],
                specified.frozen(&outer_world, &world(program.atoms(), inner))
            );
            pairs += 1;
        }
    }
    pairs
}

#[test]
fn anonymous_witnesses_are_negated_after_projection() {
    for (polarity, sign) in [(1, "not "), (2, "not not ")] {
        for condition_sign in ["", "not ", "not not "] {
            let source =
                format!("{{p(1,1);p(2,1);p(1,2);c}}.q:-{sign}p(_,(1;2)):{condition_sign}c.");
            let program = formula(&source);
            let witnesses = Formula::Or(
                Box::new(Formula::atom("p(1,1)")),
                Box::new(Formula::atom("p(2,1)")),
            );
            let alternatives = Formula::Or(
                Box::new(witnesses.sign(polarity)),
                Box::new(Formula::atom("p(1,2)").sign(polarity)),
            );
            let condition = Formula::atom("c").sign(condition_sign.matches("not").count());
            let specified = Formula::implies(
                Formula::implies(condition, alternatives),
                Formula::atom("q"),
            );
            compare_root(&program, &specified);
        }
    }
}

#[test]
fn empty_witnesses_preserve_signed_false() {
    for (polarity, sign) in [(1, "not "), (2, "not not ")] {
        let source = format!("{{c}}.q:-{sign}p(_):c.");
        compare_root(
            &formula(&source),
            &Formula::implies(
                Formula::implies(Formula::atom("c"), Formula::False.sign(polarity)),
                Formula::atom("q"),
            ),
        );
    }
}

#[test]
fn structured_witnesses_preserve_frozen_projection() {
    for (polarity, sign) in [(1, "not "), (2, "not not ")] {
        for term in ["X..X+1", "(X;X+1)", "(1;2)"] {
            let source =
                format!("{{p(f(1,1));p(f(2,1));p(f(1,2));c}}.q:-{sign}p(f(_,{term})):X=1,c.");
            let witnesses = Formula::Or(
                Box::new(Formula::atom("p(f(1,1))")),
                Box::new(Formula::atom("p(f(2,1))")),
            );
            let alternatives = Formula::Or(
                Box::new(witnesses.sign(polarity)),
                Box::new(Formula::atom("p(f(1,2))").sign(polarity)),
            );
            compare_root(
                &formula(&source),
                &Formula::implies(
                    Formula::implies(Formula::atom("c"), alternatives),
                    Formula::atom("q"),
                ),
            );
        }
    }
}

#[test]
fn projection_inputs_require_outer_support() {
    for source in [
        "q:-not p(f(_,X)):#true.",
        "q:-not not p(f(_,X)):#false.",
        "q:-not p(f(_,X)).",
        "q(X):-not p(f(_,X)):#true.",
        "q:-not p(_+1):#true.",
        "q:-not p(f(_+1)):#true.",
        "q:-not p(-f(_)):#true.",
        "q:-not not p(-f(_)):#true.",
        "q:-not p(--f(_)):#true.",
        "q:-not p(h(-f(_))):#true.",
        "q:-not -p(_):#true.",
        "q:-not not -p(_):#true.",
        "q:-not -p(f(_)):#true.",
        "q:-not not -p(f(_)):#true.",
    ] {
        assert!(
            matches!(
                limited(
                    source,
                    ExpansionLimits::default(),
                    &FormulaLimits::default()
                ),
                Err(FormulaFailure::UnsafeVariable { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn absent_witnesses_do_not_hide_value_errors() {
    for source in ["q:-not p(f(_,1/X)):X=0.", "q:-not not p(f(_,1/X)):X=0."] {
        assert!(matches!(
            limited(
                source,
                ExpansionLimits::default(),
                &FormulaLimits::default()
            ),
            Err(FormulaFailure::Expansion(
                zetesis_themelios::ExpansionFailure::Evaluation { .. }
            ))
        ));
    }
}

#[test]
fn empty_condition_rows_defer_projection_values() {
    assert_eq!(
        native(&formula("q:-not p(f(_,1/X)):X=2..1.")),
        native(&formula("q."))
    );
}

#[test]
fn positive_witnesses_preserve_frozen_disjunction() {
    let program = formula("{p(1..2)}.q:-p(X):#true.");
    let specified = Formula::implies(
        Formula::Or(
            Box::new(Formula::atom("p(1)")),
            Box::new(Formula::atom("p(2)")),
        ),
        Formula::atom("q"),
    );
    assert_eq!(compare_root(&program, &specified), 64);
}

#[test]
fn empty_alternatives_preserve_frozen_false() {
    for sign in ["", "not ", "not not "] {
        let program = formula(&format!("{{p(1)}}.q:-{sign}p(2..1):#true."));
        let specified = Formula::implies(Formula::False, Formula::atom("q"));
        assert_eq!(compare_root(&program, &specified), 16);
    }
}

#[test]
fn witnesses_cannot_repair_condition_safety() {
    for source in [
        "q:-p(X):X>1.",
        "q:-p(X):X=X.",
        "q:-p(X):not s(X).",
        "q:-p(X):X>1,Y=2..1.",
    ] {
        assert!(
            matches!(
                limited(
                    source,
                    ExpansionLimits::default(),
                    &FormulaLimits::default()
                ),
                Err(FormulaFailure::UnsafeVariable { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn witnesses_cannot_bind_outer_names() {
    for source in [
        "q(X):-p(X):#true.",
        "q(X):-p(X):#false.",
        "q(X):-p(X;2):Y=2..1.",
    ] {
        assert!(
            matches!(
                limited(
                    source,
                    ExpansionLimits::default(),
                    &FormulaLimits::default()
                ),
                Err(FormulaFailure::UnsafeVariable { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn negative_consequents_require_bound_inputs() {
    for source in [
        "q:-not p(X):#true.",
        "q:-not not p(X):#true.",
        "q:-not p(X;2):Y=2..1.",
        "q:-not p(X+(2..1)):Y=2..1.",
    ] {
        assert!(
            matches!(
                limited(
                    source,
                    ExpansionLimits::default(),
                    &FormulaLimits::default()
                ),
                Err(FormulaFailure::UnsafeVariable { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn empty_conditions_defer_value_errors() {
    assert_eq!(
        native(&formula("q:-p((1..2)/X):X=1..0.")),
        BTreeSet::from([BTreeSet::from(["q".to_owned()])])
    );
}

#[test]
fn entered_conditions_retain_value_errors() {
    assert!(matches!(
        limited(
            "q:-p((1..2)/X):X=0.",
            ExpansionLimits::default(),
            &FormulaLimits::default()
        ),
        Err(FormulaFailure::Expansion(
            zetesis_themelios::ExpansionFailure::Evaluation { .. }
        ))
    ));
}

#[test]
fn preparation_identifies_dependency_projection() {
    let source = "{p(1..2)}.q:-not p(1;2):#true.";
    let prepared = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepared.analysis_basis(),
        AnalysisBasis::DependencyProjection
    );
    assert_eq!(
        prepared.ground().unwrap().analysis_basis(),
        AnalysisBasis::DependencyProjection
    );
    assert_eq!(
        formula("p(1).q:-p(X):#true.").analysis_basis(),
        AnalysisBasis::NormalizedProgram
    );
}

#[test]
#[ignore = "requires clingo: complete sources match clingo"]
fn complete_sources_match_clingo() {
    let case = corpus_case();
    let mut sources: Vec<_> = CASES.iter().map(|(source, _)| *source).collect();
    sources.push(case.source());
    let mut total = 0;
    for source in sources {
        let output = reference::external(source, true);
        assert_eq!(output["Models"]["More"], "no");
        let expected: Models = output["Call"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
            .map(|witness| {
                witness["Value"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|atom| atom.as_str().unwrap().to_owned())
                    .collect()
            })
            .collect();
        assert_eq!(
            usize::try_from(output["Models"]["Number"].as_u64().unwrap()).unwrap(),
            expected.len()
        );
        assert_eq!(native(&formula(source)), expected, "{source}");
        total += expected.len();
    }
    println!("complete_models={total}");
}

fn first_cap(mut admit: impl FnMut(usize) -> Result<AdmittedFormula, FormulaFailure>) -> usize {
    let mut low = 0;
    let mut high = 1_000_000;
    assert!(admit(high).is_ok());
    while low < high {
        let middle = low + (high - low) / 2;
        if admit(middle).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    assert!(low > 0);
    assert!(admit(low).is_ok());
    assert!(admit(low - 1).is_err());
    low
}
const BOUNDED: &str = "{p(1..3)}.q:-p(X-1;2*(X..X+1)-3):X=2.";
const PROJECTED: &str = "{p(f(1,1));p(f(2,1));p(g(1,1))}.q:-not p(f(_,X..X+1)):X=1.";

#[test]
fn witness_projection_obeys_the_work_ceiling() {
    let cap = first_cap(|cap| {
        limited(
            PROJECTED,
            ExpansionLimits::default(),
            &FormulaLimits {
                max_work: u64::try_from(cap).unwrap(),
                ..FormulaLimits::default()
            },
        )
    });
    assert!(cap > 0);
    assert!(matches!(
        limited(
            PROJECTED,
            ExpansionLimits::default(),
            &FormulaLimits {
                max_work: u64::try_from(cap - 1).unwrap(),
                ..FormulaLimits::default()
            }
        ),
        Err(FormulaFailure::Limit {
            resource: zetesis_themelios::FormulaResource::Work,
            ..
        })
    ));
    assert_eq!(
        native(
            &limited(
                PROJECTED,
                ExpansionLimits::default(),
                &FormulaLimits {
                    max_work: u64::try_from(cap).unwrap(),
                    ..FormulaLimits::default()
                }
            )
            .unwrap()
        ),
        native(&formula(PROJECTED))
    );
}

#[test]
fn witness_projection_obeys_the_substitution_ceiling() {
    let cap = first_cap(|cap| {
        limited(
            PROJECTED,
            ExpansionLimits::default(),
            &FormulaLimits {
                max_substitutions: u64::try_from(cap).unwrap(),
                ..FormulaLimits::default()
            },
        )
    });
    assert!(cap > 0);
    assert!(matches!(
        limited(
            PROJECTED,
            ExpansionLimits::default(),
            &FormulaLimits {
                max_substitutions: u64::try_from(cap - 1).unwrap(),
                ..FormulaLimits::default()
            }
        ),
        Err(FormulaFailure::Limit {
            resource: zetesis_themelios::FormulaResource::Substitutions,
            ..
        })
    ));
    assert_eq!(
        native(
            &limited(
                PROJECTED,
                ExpansionLimits::default(),
                &FormulaLimits {
                    max_substitutions: u64::try_from(cap).unwrap(),
                    ..FormulaLimits::default()
                }
            )
            .unwrap()
        ),
        native(&formula(PROJECTED))
    );
}

#[test]
fn witness_projection_obeys_the_byte_ceiling() {
    let cap = first_cap(|cap| {
        limited(
            PROJECTED,
            ExpansionLimits {
                max_scalar_bytes: cap,
                ..ExpansionLimits::default()
            },
            &FormulaLimits::default(),
        )
    });
    assert!(cap > 0);
    assert!(matches!(
        limited(
            PROJECTED,
            ExpansionLimits {
                max_scalar_bytes: cap - 1,
                ..ExpansionLimits::default()
            },
            &FormulaLimits::default()
        ),
        Err(FormulaFailure::Expansion(
            zetesis_themelios::ExpansionFailure::Limit {
                resource: zetesis_themelios::ExpansionResource::ScalarBytes,
                ..
            }
        ))
    ));
    assert_eq!(
        native(
            &limited(
                PROJECTED,
                ExpansionLimits {
                    max_scalar_bytes: cap,
                    ..ExpansionLimits::default()
                },
                &FormulaLimits::default()
            )
            .unwrap()
        ),
        native(&formula(PROJECTED))
    );
}

#[test]
fn anonymous_inspections_obey_the_term_work_ceiling() {
    let source = format!("q:-not p({}_{}):#true.", "f(".repeat(16), ")".repeat(16));
    let cap = first_cap(|cap| {
        limited(
            &source,
            ExpansionLimits {
                max_term_work: cap,
                ..ExpansionLimits::default()
            },
            &FormulaLimits::default(),
        )
    });
    assert!(cap > 0);
    assert!(matches!(
        limited(
            &source,
            ExpansionLimits {
                max_term_work: cap - 1,
                ..ExpansionLimits::default()
            },
            &FormulaLimits::default()
        ),
        Err(FormulaFailure::Expansion(
            zetesis_themelios::ExpansionFailure::Limit {
                resource: zetesis_themelios::ExpansionResource::TermWork,
                ..
            }
        ))
    ));
    assert_eq!(
        native(
            &limited(
                &source,
                ExpansionLimits {
                    max_term_work: cap,
                    ..ExpansionLimits::default()
                },
                &FormulaLimits::default()
            )
            .unwrap()
        ),
        native(&formula("q."))
    );
}

#[test]
fn private_witness_slots_obey_the_variable_ceiling() {
    let source = "q:-not p(f(_)):#true.";
    let mut options = AdmissionOptions::default();
    options.core_limits.max_variables_per_template = 0;
    assert!(matches!(
        admit_formula(
            source.into(),
            options,
            ExpansionLimits::default(),
            FormulaLimits::default()
        ),
        Err(FormulaFailure::Limit {
            resource: zetesis_themelios::FormulaResource::Variables,
            observed: 1,
            ..
        })
    ));
    options.core_limits.max_variables_per_template = 1;
    assert_eq!(
        native(
            &admit_formula(
                source.into(),
                options,
                ExpansionLimits::default(),
                FormulaLimits::default()
            )
            .unwrap()
        ),
        native(&formula("q."))
    );
}

#[test]
fn local_substitution_limit_is_inclusive() {
    let run = |cap| {
        limited(
            BOUNDED,
            ExpansionLimits::default(),
            &FormulaLimits {
                max_substitutions: cap as u64,
                ..Default::default()
            },
        )
    };
    let cap = first_cap(run);
    assert!(
        matches!(run(cap - 1), Err(FormulaFailure::Limit { resource: zetesis_themelios::FormulaResource::Substitutions, observed, limit, .. }) if observed == limit + 1)
    );
    assert_eq!(native(&run(cap).unwrap()), native(&formula(BOUNDED)));
}

#[test]
fn consequent_value_limit_is_inclusive() {
    let run = |cap| {
        limited(
            BOUNDED,
            ExpansionLimits {
                max_values: cap,
                ..Default::default()
            },
            &FormulaLimits::default(),
        )
    };
    let cap = first_cap(run);
    assert!(matches!(
        run(cap - 1),
        Err(FormulaFailure::Expansion(
            zetesis_themelios::ExpansionFailure::Limit {
                resource: zetesis_themelios::ExpansionResource::Values,
                ..
            }
        ))
    ));
}

#[test]
fn consequent_byte_limit_is_inclusive() {
    let run = |cap| {
        limited(
            BOUNDED,
            ExpansionLimits {
                max_scalar_bytes: cap,
                ..Default::default()
            },
            &FormulaLimits::default(),
        )
    };
    let cap = first_cap(run);
    assert!(matches!(
        run(cap - 1),
        Err(FormulaFailure::Expansion(
            zetesis_themelios::ExpansionFailure::Limit {
                resource: zetesis_themelios::ExpansionResource::ScalarBytes,
                ..
            }
        ))
    ));
}

#[test]
fn consequent_term_work_limit_is_inclusive() {
    let run = |cap| {
        limited(
            BOUNDED,
            ExpansionLimits {
                max_term_work: cap,
                ..Default::default()
            },
            &FormulaLimits::default(),
        )
    };
    let cap = first_cap(run);
    assert!(matches!(
        run(cap - 1),
        Err(FormulaFailure::Expansion(
            zetesis_themelios::ExpansionFailure::Limit {
                resource: zetesis_themelios::ExpansionResource::TermWork,
                ..
            }
        ))
    ));
}

#[test]
fn projection_node_limit_is_inclusive() {
    let run = |cap| {
        limited(
            BOUNDED,
            ExpansionLimits::default(),
            &FormulaLimits {
                max_analysis_nodes: cap,
                ..Default::default()
            },
        )
    };
    let cap = first_cap(run);
    assert!(matches!(
        run(cap - 1),
        Err(FormulaFailure::Limit {
            resource: zetesis_themelios::FormulaResource::AnalysisNodes,
            ..
        })
    ));
}

#[test]
fn consequent_products_are_preflighted() {
    let arguments = ["(1;2)"; 20].join(",");
    let source = format!("q:-p({arguments}):#true.");
    assert!(
        matches!(limited(&source, ExpansionLimits { max_values: 1_000, ..Default::default() }, &FormulaLimits::default()),
        Err(FormulaFailure::Expansion(zetesis_themelios::ExpansionFailure::Limit { resource: zetesis_themelios::ExpansionResource::Values, observed, .. })) if observed >= 1 << 20)
    );
}

#[test]
fn dependency_projection_preserves_edge_modes() {
    use zetesis_themelios::logical::{
        analyze::DependencyKind,
        symbol::{Name, Sign, Signature},
    };
    for (text, mode) in [
        ("", DependencyKind::Positive),
        ("not ", DependencyKind::Negative),
        ("not not ", DependencyKind::Negative),
    ] {
        let source = format!("q:-{text}-p(1;1,2):d(1),not c.");
        let program = formula(&source);
        assert_eq!(
            program.analysis_basis(),
            AnalysisBasis::DependencyProjection
        );
        let signature = |name, arity, sign| Signature {
            name: Name::new(name).unwrap(),
            arity,
            sign,
        };
        let q = signature("q", 0, Sign::Positive);
        let edges: BTreeSet<_> = program
            .source_analysis()
            .dependencies()
            .edges_from(&q)
            .map(|(kind, signature)| (kind, signature.clone()))
            .collect();
        assert_eq!(
            edges,
            BTreeSet::from([
                (mode, signature("p", 1, Sign::Negative)),
                (mode, signature("p", 2, Sign::Negative)),
                (DependencyKind::Positive, signature("d", 1, Sign::Positive)),
                (DependencyKind::Negative, signature("c", 0, Sign::Positive)),
            ])
        );
        assert_eq!(
            program.source_analysis(),
            &zetesis_themelios::analysis::Analysis::of(program.analyzed_program())
        );
    }
}

#[test]
fn projected_alternatives_retain_parsed_atom_origins() {
    use zetesis_themelios::logical::{
        program::{BodyElement, LiteralInner, Statement},
        provenance::Origin,
    };
    let source = "q:-not -p(1;1,2):d(1).";
    let program = formula(source);
    let mut alternatives = 0;
    for statement in program.analyzed_program().statements() {
        assert!(
            statement
                .provenance()
                .origins()
                .any(|origin| matches!(origin, Origin::Parsed(_)))
        );
        let Statement::Rule(rule) = statement.get() else {
            panic!("rule")
        };
        for element in rule.body().get().elements() {
            let BodyElement::Conditional(conditional) = element.get() else {
                panic!("conditional")
            };
            let LiteralInner::Atom(atom) = &conditional.literal.inner else {
                panic!("atom")
            };
            let locations: Vec<_> = atom
                .provenance()
                .origins()
                .filter_map(|origin| match origin {
                    Origin::Parsed(location) => Some(location),
                    _ => None,
                })
                .collect();
            assert_eq!(locations.len(), 1);
            assert_eq!(locations[0].source, program.source().id());
            alternatives += 1;
        }
    }
    assert_eq!(alternatives, 2);
}

#[test]
fn prepared_bundle_retains_analysis_basis() {
    use zetesis_themelios::{
        BundleAdmissionOptions, BundleLimits, SourceBundle, prepare_bundle_formula,
    };
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input.lp");
    std::fs::write(&path, "{p(1..2)}.q:-p(1;2):#true.").unwrap();
    let bundle = SourceBundle::load(&path, BundleLimits::default()).unwrap();
    let prepared = prepare_bundle_formula(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepared.analysis_basis(),
        AnalysisBasis::DependencyProjection
    );
    assert_eq!(
        prepared.ground().unwrap().analysis_basis(),
        AnalysisBasis::DependencyProjection
    );
}

#[test]
fn outer_pools_preserve_local_alternatives() {
    let actual = formula("{p(1..2)}.q(a;b):-p(1;2):#true.");
    assert_eq!(
        native(&actual),
        native(&formula(
            "{p(1..2)}.q(a):-p(1).q(a):-p(2).q(b):-p(1).q(b):-p(2)."
        ))
    );
}

#[test]
fn private_interval_slots_obey_variable_limit() {
    let mut options = AdmissionOptions::default();
    options.core_limits.max_variables_per_template = 1;
    let result = admit_formula(
        "q:-p(2*(X..X+1)):X=1.".into(),
        options,
        ExpansionLimits::default(),
        FormulaLimits::default(),
    );
    assert!(matches!(
        result,
        Err(FormulaFailure::Limit {
            resource: zetesis_themelios::FormulaResource::Variables,
            limit: 1,
            observed: 2,
            ..
        })
    ));
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config { cases: 64, rng_seed: proptest::test_runner::RngSeed::Fixed(20_260_907), ..Default::default() })]
    #[test]
    fn interval_alternatives_preserve_models(lower in -2_i32..3, width in 0_i32..3, coefficient in -2_i32..3) {
        let upper = lower + width;
        let atoms: BTreeSet<_> = (lower..=upper).map(|x| format!("p({})", coefficient * x + 1)).collect();
        let choice = format!("{{{}}}.", atoms.iter().cloned().collect::<Vec<_>>().join(";"));
        let source = format!("{choice}q:-p({coefficient}*(X..X+{width})+1):X={lower}.");
        let mut expanded = choice;
        for atom in &atoms { write!(expanded, "q:-{atom}.").unwrap(); }
        proptest::prop_assert_eq!(native(&formula(&source)), native(&formula(&expanded)));
    }
}

#[test]
fn conjunction_matches_bounded_completion() {
    use std::num::NonZeroUsize;
    use zetesis_sat::{
        BatchLimits, BatchVerdict, Cancellation, CompletionExecutor, Limits, StableModels,
    };
    let case = corpus_case();
    let admitted = formula(case.source());
    let expected = corpus_models();
    assert_eq!(expected.len(), 4);
    for workers in [1, 2] {
        let mut executor = CompletionExecutor::new(NonZeroUsize::new(workers).unwrap()).unwrap();
        let mut search = StableModels::new(
            admitted.theory(),
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        let mut models = Models::new();
        while !search.exhausted() {
            let batch = search
                .next_batch_with_completion(
                    BatchLimits {
                        max_candidates: NonZeroUsize::new(4).unwrap(),
                        max_pending_bytes: 1 << 20,
                    },
                    &mut executor,
                    |_, candidates| {
                        Ok::<_, std::convert::Infallible>(vec![
                            BatchVerdict::Residual;
                            candidates.len()
                        ])
                    },
                )
                .unwrap();
            for model in batch {
                assert!(
                    models.insert(
                        model
                            .atoms()
                            .map(|index| canonical(admitted.atoms().at(index).unwrap()))
                            .collect()
                    )
                );
            }
        }
        assert_eq!(models, expected);
    }
}

#[test]
fn projected_copies_share_the_node_allowance() {
    let cap_for = |source: &str| {
        first_cap(|cap| {
            limited(
                source,
                ExpansionLimits::default(),
                &FormulaLimits {
                    max_analysis_nodes: cap,
                    ..Default::default()
                },
            )
        })
    };
    let one = "q:-p(1;2):#true.";
    let two = "q:-p(1;2):#true.r:-p(1;2):#true.";
    let cap = cap_for(one);
    assert!(cap_for(two) > cap);
    assert!(matches!(
        limited(
            two,
            ExpansionLimits::default(),
            &FormulaLimits {
                max_analysis_nodes: cap,
                ..Default::default()
            }
        ),
        Err(FormulaFailure::Limit {
            resource: zetesis_themelios::FormulaResource::AnalysisNodes,
            ..
        })
    ));
}
