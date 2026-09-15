//! Source pools retain whole-rule products, local groups and bounded evidence.
#[path = "support/finite_bindings.rs"]
mod reference;
use reference::{Models, atom_text, exhaustive, holds, native, values};
use std::collections::BTreeSet;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, admit_formula,
};

fn input(source: &str) -> AdmittedFormula {
    limited(
        source,
        ExpansionLimits::default(),
        &FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
}
fn limited(
    source: &str,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        expansion,
        *limits,
    )
}

// Independently handwritten expansions. The Cartesian family remains a family
// of rules; element expansion retains one choice and its original bounds.
const BOOLEAN_OCCURRENCE_POOL: &str = "{p(1);p(2)}.1{#true:p(1;2)}1.";

const CASES: &[(&str, &str)] = &[
    ("{d(1);d(2)}.q:-X=(1;2):d(X).", "{d(1);d(2)}.q."),
    ("{d(1);d(2)}.q:-X=(1;3):d(X).", "{d(1);d(2)}.q:-not d(2)."),
    ("{d(1);d(2)}.q:-not X=(1;2):d(X).", "{d(1);d(2)}.q."),
    ("{d(1);d(2)}.q:-f(X)=f(1..2):d(X).", "{d(1);d(2)}.q."),
    ("{q}.q:-1<(1..2)<2:#true.", "{q}."),
    ("p(X):-X=(1..2)..3.", "p(X):-X=1..3.p(X):-X=2..3."),
    ("p(X):-X=0..(1..2).", "p(X):-X=0..1.p(X):-X=0..2."),
    (
        "p:-not not 2=(1..2)..3.",
        "p:-not not 2=1..3.p:-not not 2=2..3.",
    ),
    (
        "p:-not not 0=(1..2)..3.",
        "p:-not not 0=1..3.p:-not not 0=2..3.",
    ),
    (
        "{p(f(1));p(f(2))}.q:-2{p(f(1..2))}2.",
        "{p(f(1));p(f(2))}.q:-2{p(f(1));p(f(2))}2.",
    ),
    (
        "{p(f(1));p(f(2))}.q:-1{p(f(X)):X=1..2}1.",
        "{p(f(1));p(f(2))}.q:-1{p(f(1));p(f(2))}1.",
    ),
    (
        "{p(f(1));p(f(2))}.q:-1{p(f(1;1;2))}1.",
        "{p(f(1));p(f(2))}.q:-1{p(f(1));p(f(2))}1.",
    ),
    ("(0;1){p}1.", "0{p}1.1{p}1."),
    (
        "{p}.q:-#count{1:p}=(0;1).",
        "{p}.q:-#count{1:p}=0.q:-#count{1:p}=1.",
    ),
    (
        "{p}.q:-#count{1:p}=(0..1)+0.",
        "{p}.q:-#count{1:p}=0.q:-#count{1:p}=1.",
    ),
    ("(0..1){p}1.", "0{p}1.1{p}1."),
    ("1#count{f(1..2):p}1.", "1#count{f(1):p;f(2):p}1."),
    (BOOLEAN_OCCURRENCE_POOL, "{p(1);p(2)}.:-not p(1),not p(2)."),
    (
        "{p(1);p(2)}.1{#true:p(X),X=1..2}1.",
        "{p(1);p(2)}.:-not p(1),not p(2).",
    ),
    (
        "{p(1);p(2)}.1{#true:p(X)}1.",
        "{p(1);p(2)}.:-not p(1),not p(2).",
    ),
    (
        "{p(1);p(2)}.1{#true:p(1);#true:p(2)}1.",
        "{p(1);p(2)}.:-not p(1),not p(2).:-p(1),p(2).",
    ),
    (
        "{p}.q:-#count{f(1..2):p}=2.",
        "{p}.q:-#count{f(1):p;f(2):p}=2.",
    ),
    (
        "1#count{(1;2):p(1;2)}1.",
        "1#count{1:p(1);1:p(2);2:p(1);2:p(2)}1.",
    ),
    ("p(X):-q(1;2),X=1.", "p(X):-q(1),X=1.p(X):-q(2),X=1."),
    ("{p(X):X=(1;2)}.", "{p(X):X=1;p(X):X=2}."),
    ("p:-#count{X:X=(1;2)}>0.", "p:-#count{X:X=1;X:X=2}>0."),
    ("p(1+(1;2));q.", "p(2);q.p(3);q."),
    ("p(f(1)).q:-p(f(1..2)).", "p(f(1)).q:-p(f(1)).q:-p(f(2))."),
    (
        "{p(f(1));p(f(2))}.q:-not p(f(1..2)).",
        "{p(f(1));p(f(2))}.q:-not p(f(1)).q:-not p(f(2)).",
    ),
    ("p(f(X)):-X=f(1..2).", "p(f(f(1))).p(f(f(2)))."),
    ("{p(1);p(2)}.q:-p(1;2).", "{p(1);p(2)}.q:-p(1).q:-p(2)."),
    (
        "{p(1);p(2)}.q:-not p(1;2).",
        "{p(1);p(2)}.q:-not p(1).q:-not p(2).",
    ),
    (
        "{p(1);p(2)}.q:-not not p(1;2).",
        "{p(1);p(2)}.q:-not not p(1).q:-not not p(2).",
    ),
    (
        "{p(f(1));p(f(2))}.q:-p(f((1;2))).",
        "{p(f(1));p(f(2))}.q:-p(f(1)).q:-p(f(2)).",
    ),
    (
        "d(1).q:-d(X),not p(f(X;2)).",
        "d(1).q:-d(X),not p(f(X)).q:-d(X),not p(f(2)).",
    ),
    ("{p(1);p(2)}.{q:p(1;2)}.", "{p(1);p(2)}.{q:p(1);q:p(2)}."),
    (
        "{p(1);p(2)}.q:-p(1):p(1;2).",
        "{p(1);p(2)}.q:-p(1):p(1);p(1):p(2).",
    ),
    (
        "{p(1);p(2)}.q:-2=#count{(1;2):p(1;2)}.",
        "{p(1);p(2)}.q:-2=#count{1:p(1);2:p(1);1:p(2);2:p(2)}.",
    ),
    (
        "p(X):-X=(1;2),(0;1)<X<3.",
        "p(X):-X=1,0<X<3.p(X):-X=1,1<X<3.p(X):-X=2,0<X<3.p(X):-X=2,1<X<3.",
    ),
    ("p(f(1..2)).", "p(f(1)).p(f(2))."),
    ("d(1).p(f(X..X+1)):-d(X).", "d(1).p(f(1)).p(f(2))."),
    ("d(1).p((X..X+1)+1)|q:-d(X).", "d(1).p(2)|q.p(3)|q."),
    ("p(f(1..2));q.", "p(f(1));q.p(f(2));q."),
    ("1{p(f(1..2))}1.", "1{p(f(1));p(f(2))}1."),
    (
        "p(f((1;2)),(3;4));q.",
        "p(f(1),3);q.p(f(1),4);q.p(f(2),3);q.p(f(2),4);q.",
    ),
    ("a(X):-X=(1;2;4).", "a(1).a(2).a(4)."),
    ("a(X):-(1;2)=X.", "a(1).a(2)."),
    ("p(X,Y):-X=(1;2),Y=(X;X+1).", "p(1,1).p(1,2).p(2,2).p(2,3)."),
    ("p(X,Y):-Y=(X;X+1),X=(1;2).", "p(1,1).p(1,2).p(2,2).p(2,3)."),
    ("d(0..3).p(X):-d(X),X=(1;2).", "d(0..3).p(1).p(2)."),
    ("p(X,Y):-X=(1;2),Y=(3;4),X+Y!=5.", "p(1,3).p(2,4)."),
    ("p(X):-X=(1;1;2).", "p(1).p(2)."),
    ("p(X,Y):-X=(1;2),Y=(1;2).", "p(1,1).p(1,2).p(2,1).p(2,2)."),
    ("d(0).p(1;2):-d(0).", "d(0).p(1).p(2)."),
    ("p(1;2);q.", "p(1);q.p(2);q."),
    ("a(1;2) | b.\n", "a(1) | b.a(2) | b."),
    ("a(1;2)|b.", "a(1)|b.a(2)|b."),
    ("not a(1;2)|b.", "not a(1)|b.not a(2)|b."),
    ("1 {p((1..2;2..3))} 1.", "1{p(1);p(2);p(3)}1."),
    ("1 {p(1..2;2..3)} 1.", "1{p(1);p(2);p(3)}1."),
    ("p(f(1;2));q.", "p(f(1));q.p(f(2));q."),
    ("p(1;2);q(3;4).", "p(1);q(3).p(1);q(4).p(2);q(3).p(2);q(4)."),
    ("p((1;2),(3;4));q.", "p(1,3);q.p(1,4);q.p(2,3);q.p(2,4);q."),
    ("p(1;2,3);q.", "p(1);q.p(2,3);q."),
    ("{p(1;2)}.", "{p(1);p(2)}."),
    ("1{p(1;2);p(2)}1.", "1{p(1);p(2)}1."),
    ("1{p(X;X+1)}1:-X=(1;2).", "1{p(1);p(2)}1.1{p(2);p(3)}1."),
    ("p(2..1;3);q.", "p(3);q."),
    ("1{p(2..1;3..2)}1.", "1{}1."),
    ("p(X):-X=(2..1;3).", "p(3)."),
    ("-p(1;2);q.", "-p(1);q.-p(2);q."),
    (
        "not p(1;2);q.{p(1);p(2)}.",
        "not p(1);q.not p(2);q.{p(1);p(2)}.",
    ),
    (
        "not not p(1;2);q.{p(1);p(2)}.",
        "not not p(1);q.not not p(2);q.{p(1);p(2)}.",
    ),
    ("p(X):-X=(-f(1);(2,)).", "p(-f(1)).p((2,))."),
    (
        "p(X):-X=(1;2).n(N):-N=#count{X:p(X)}.",
        "p(1).p(2).n(N):-N=#count{X:p(X)}.",
    ),
    (
        "{d(1);d(2)}.1{p(X;X+1):d(X)}1.",
        "{d(1);d(2)}.1{p(1):d(1);p(2):d(1);p(2):d(2);p(3):d(2)}1.",
    ),
    ("{p(1;2):p(1)}.", "{p(1):p(1);p(2):p(1)}."),
    (
        "p(X):-X=(z;a).m(M):-M=#min{X:p(X)}.",
        "p(z).p(a).m(M):-M=#min{X:p(X)}.",
    ),
    (
        "m(K,M):-K=(z;a),M=#min{K}.",
        "m(z,M):-M=#min{z}.m(a,M):-M=#min{a}.",
    ),
    (
        "p(X):-X=(f(1);\"s\").n(N):-N=#max{X:p(X)}.",
        "p(f(1)).p(\"s\").n(N):-N=#max{X:p(X)}.",
    ),
    (
        "p(1;2):#true;q:#true.",
        "p(1):#true;q:#true.p(2):#true;q:#true.",
    ),
    (
        "1#count{X:p(K,X):X=1..2}1:-K=(1;2).",
        "1{p(1,1);p(1,2)}1.1{p(2,1);p(2,2)}1.",
    ),
    ("#count{K:p}:-K=(1;2).", "{p}.{p}."),
];

#[test]
fn complete_models_and_every_frozen_pair_match_handwritten_expansions() {
    for &(source, expanded) in CASES {
        let original = input(source);
        let reference = input(expanded);
        assert_eq!(native(&original), native(&reference), "{source}");
        assert_eq!(native(&original), exhaustive(&original), "{source}");
        let left: Vec<_> = original.atoms().iter().map(atom_text).collect();
        let right: Vec<_> = reference.atoms().iter().map(atom_text).collect();
        assert_eq!(
            left.iter().collect::<BTreeSet<_>>(),
            right.iter().collect(),
            "{source}"
        );
        assert!(left.len() <= 6);
        let remap = |mask: usize| {
            left.iter().enumerate().fold(0, |result, (index, atom)| {
                result
                    | if mask & (1 << index) == 0 {
                        0
                    } else {
                        1 << right.iter().position(|other| other == atom).unwrap()
                    }
            })
        };
        for outer in 0..1 << left.len() {
            let frozen = values(original.theory(), outer, None);
            let other = values(reference.theory(), remap(outer), None);
            assert_eq!(
                holds(original.theory(), &frozen),
                holds(reference.theory(), &other),
                "{source}"
            );
            for inner in 0..1 << left.len() {
                assert_eq!(
                    holds(
                        original.theory(),
                        &values(original.theory(), inner, Some(&frozen))
                    ),
                    holds(
                        reference.theory(),
                        &values(reference.theory(), remap(inner), Some(&other))
                    ),
                    "{source}: M={outer} J={inner}",
                );
            }
        }
        assert_eq!(original.source().text(), source);
    }
}

#[test]
fn original_manual_formulas_distinguish_cartesian_rules_from_a_flat_head() {
    // This formula is handwritten without compiling a second source. Additional
    // support constraints are tautologies here because both rules have true bodies.
    let p = input("p(1;2);q.");
    let names: Vec<_> = p.atoms().iter().map(atom_text).collect();
    let bit =
        |mask: usize, name: &str| mask & (1 << names.iter().position(|n| n == name).unwrap()) != 0;
    for outer in 0..8 {
        let first = bit(outer, "p(1)") || bit(outer, "q");
        let second = bit(outer, "p(2)") || bit(outer, "q");
        let frozen = values(p.theory(), outer, None);
        assert_eq!(holds(p.theory(), &frozen), first && second);
        for inner in 0..8 {
            let atom = |name| bit(outer, name) && bit(inner, name);
            let expected =
                first && second && (atom("p(1)") || atom("q")) && (atom("p(2)") || atom("q"));
            assert_eq!(
                holds(p.theory(), &values(p.theory(), inner, Some(&frozen))),
                expected
            );
        }
    }
    let expected: Models = [vec!["p(1)", "p(2)"], vec!["q"]]
        .into_iter()
        .map(|row| row.into_iter().map(str::to_owned).collect())
        .collect();
    assert_eq!(native(&p), expected);
    assert_ne!(native(&p), native(&input("p(1);p(2);q.")));
    assert_ne!(
        native(&input("1{p(1;2)}1.")),
        native(&input("1{p(1)}1.1{p(2)}1."))
    );
}

#[test]
fn templates_charge_duplicate_occurrences_before_deduplication() {
    let source = "p(1;1;2);q.";
    let error = limited(
        source,
        ExpansionLimits {
            max_templates: 2,
            ..ExpansionLimits::default()
        },
        &FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::Templates,
            limit: 2,
            observed: 3,
            ..
        })
    ));
    assert!(
        limited(
            source,
            ExpansionLimits {
                max_templates: 3,
                ..ExpansionLimits::default()
            },
            &FormulaLimits::default()
        )
        .is_ok()
    );
}

fn first_success(mut accepts: impl FnMut(usize) -> bool) -> usize {
    let mut high = 1;
    while !accepts(high) {
        high *= 2;
        assert!(high <= 1 << 24);
    }
    let mut low = 0;
    while low + 1 < high {
        let middle = usize::midpoint(low, high);
        if accepts(middle) {
            high = middle;
        } else {
            low = middle;
        }
    }
    high
}

#[test]
fn independent_limits_are_inclusive_and_failure_retains_source_location() {
    let source = "p((1;2),(3;4));q.";
    for resource in [
        ExpansionResource::TermWork,
        ExpansionResource::Values,
        ExpansionResource::ScalarBytes,
        ExpansionResource::Origins,
    ] {
        let configured = |limit| {
            let mut bounds = ExpansionLimits::default();
            match resource {
                ExpansionResource::TermWork => bounds.max_term_work = limit,
                ExpansionResource::Values => bounds.max_values = limit,
                ExpansionResource::ScalarBytes => bounds.max_scalar_bytes = limit,
                ExpansionResource::Origins => bounds.max_origin_locations = limit,
                _ => unreachable!(),
            }
            bounds
        };
        let exact = first_success(|limit| {
            limited(source, configured(limit), &FormulaLimits::default()).is_ok()
        });
        let error = limited(source, configured(exact - 1), &FormulaLimits::default()).unwrap_err();
        assert!(
            matches!(error, FormulaFailure::Expansion(ExpansionFailure::Limit { resource: actual, location, .. })
            if actual == resource && !location.span.is_empty()),
            "{error}"
        );
        assert_eq!(
            native(&limited(source, configured(exact), &FormulaLimits::default()).unwrap()),
            native(&input(source))
        );
    }
    for resource in [
        FormulaResource::AnalysisNodes,
        FormulaResource::Substitutions,
        FormulaResource::Work,
    ] {
        let configured = |limit: usize| {
            let mut bounds = FormulaLimits::default();
            match resource {
                FormulaResource::AnalysisNodes => bounds.max_analysis_nodes = limit,
                FormulaResource::Substitutions => bounds.max_substitutions = limit as u64,
                FormulaResource::Work => bounds.max_work = limit as u64,
                _ => unreachable!(),
            }
            bounds
        };
        let exact = first_success(|limit| {
            limited(source, ExpansionLimits::default(), &configured(limit)).is_ok()
        });
        let error =
            limited(source, ExpansionLimits::default(), &configured(exact - 1)).unwrap_err();
        assert!(
            matches!(error, FormulaFailure::Limit { resource: actual, .. } if actual == resource),
            "{error}"
        );
    }
}

#[test]
fn a_true_conditional_guard_does_not_hide_later_undefined_values() {
    for source in ["d(0).q:-1=(1;1/X):d(X).", "d(0).q:-1=(1..2)/X:d(X)."] {
        let error = limited(
            source,
            ExpansionLimits::default(),
            &FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, FormulaFailure::Expansion(ExpansionFailure::Evaluation { location, .. })
            if !location.span.is_empty()),
            "{source}: {error}"
        );
    }
}

#[test]
fn conditional_value_alternatives_cannot_bind_source_names() {
    for source in ["q:-X=(1;2):#true.", "q:-X=1..2:#true."] {
        let error = limited(
            source,
            ExpansionLimits::default(),
            &FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{source}: {error}"
        );
    }
}

#[test]
fn local_value_owners_respect_each_expansion_ceiling() {
    for source in [
        "{d(1);d(2)}.q:-f(X)=f(1..2):d(X).",
        "{p(f(1));p(f(2))}.q:-2{p(f(1..2))}2.",
    ] {
        let expected = native(&input(source));
        for resource in [
            ExpansionResource::TermWork,
            ExpansionResource::Values,
            ExpansionResource::ScalarBytes,
        ] {
            let configured = |limit| {
                let mut bounds = ExpansionLimits::default();
                match resource {
                    ExpansionResource::TermWork => bounds.max_term_work = limit,
                    ExpansionResource::Values => bounds.max_values = limit,
                    ExpansionResource::ScalarBytes => bounds.max_scalar_bytes = limit,
                    _ => unreachable!(),
                }
                bounds
            };
            let exact = first_success(|limit| {
                limited(source, configured(limit), &FormulaLimits::default()).is_ok()
            });
            let error =
                limited(source, configured(exact - 1), &FormulaLimits::default()).unwrap_err();
            assert!(
                matches!(error, FormulaFailure::Expansion(ExpansionFailure::Limit { resource: actual, location, .. })
                if actual == resource && !location.span.is_empty()),
                "{source}: {error}"
            );
            let accepted = limited(source, configured(exact), &FormulaLimits::default()).unwrap();
            assert_eq!(native(&accepted), expected, "{source}: {resource:?}");
        }
    }
}

#[test]
fn unsafe_or_undefined_alternatives_remain_located_refusals() {
    for source in [
        "p:-not X=(1;2).",
        "p:-X<(1;2).",
        "p(X;1):-#true.",
        "{p(2..1;X)}.",
        "p(1/0;2);q.",
    ] {
        let error = limited(
            source,
            ExpansionLimits::default(),
            &FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(!error.diagnostics().is_empty(), "{source}");
    }
}

#[test]
#[ignore = "requires an independently installed clingo"]
fn pool_cases_match_declared_clingo_families() {
    for &(source, _) in CASES {
        let raw = reference::external(source, true);
        assert_eq!(raw["Models"]["More"], "no");
        let models: Models = raw["Call"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
            .map(|model| {
                model["Value"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|atom| atom.as_str().unwrap().to_owned())
                    .collect()
            })
            .collect();
        let native = native(&input(source));
        if source == BOOLEAN_OCCURRENCE_POOL {
            // The adopted Boolean-choice extension retains one written key
            // across pool alternatives. Clingo splits this pool into two
            // contributions, unlike its one-element variable/interval forms.
            // Check both complete families explicitly; neither is a parity claim.
            assert_eq!(
                native,
                Models::from([
                    BTreeSet::from(["p(1)".into()]),
                    BTreeSet::from(["p(2)".into()]),
                    BTreeSet::from(["p(1)".into(), "p(2)".into()]),
                ]),
                "adopted one-occurrence semantics: {source}"
            );
            assert_eq!(
                models,
                Models::from([
                    BTreeSet::from(["p(1)".into()]),
                    BTreeSet::from(["p(2)".into()]),
                ]),
                "clingo pool expansion: {source}"
            );
        } else {
            assert_eq!(native, models, "{source}");
        }
    }
}

#[test]
fn include_origins_survive_duplicate_rules_and_choice_groups() {
    use zetesis_themelios::{
        BundleAdmissionOptions, BundleLimits, SourceBundle, admit_bundle_formula,
    };
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let directory = Directory(
        std::env::temp_dir().join(format!("zetesis-pool-origins-{}", std::process::id())),
    );
    std::fs::create_dir(&directory.0).unwrap();
    for rule in ["p(one;2);q.", "1{p(one;2)}1."] {
        std::fs::write(
            directory.0.join("entry.lp"),
            format!("#include \"data.lp\".\n{rule}"),
        )
        .unwrap();
        std::fs::write(
            directory.0.join("data.lp"),
            format!("#const one=1.\n{rule}"),
        )
        .unwrap();
        let bundle =
            SourceBundle::load(directory.0.join("entry.lp"), BundleLimits::default()).unwrap();
        let admitted = admit_bundle_formula(
            bundle,
            BundleAdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        assert!(!admitted.formula_origins().is_empty());
        for origins in admitted.formula_origins() {
            let sources: BTreeSet<_> = origins.iter().map(|origin| origin.source).collect();
            assert_eq!(sources.len(), 2);
            for origin in origins {
                let original = admitted
                    .bundle()
                    .get(origin.source)
                    .unwrap()
                    .source()
                    .slice(origin.span)
                    .unwrap();
                assert_eq!(original, rule);
            }
        }
    }
}

#[test]
fn oversized_pool_products_refuse_before_publication() {
    let arguments = ["(1;2)"; 140].join(",");
    let source = format!("p({arguments});q.");
    assert!(matches!(
        limited(
            &source,
            ExpansionLimits::default(),
            &FormulaLimits::default()
        ),
        Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::Templates,
            observed: u128::MAX,
            ..
        }))
    ));
}

#[test]
fn pooled_head_values_obey_the_cumulative_limit() {
    assert!(matches!(
        limited(
            "p(X):-X=(1;2).",
            ExpansionLimits::default(),
            &FormulaLimits {
                max_generated_values: 1,
                ..FormulaLimits::default()
            }
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::GeneratedValues,
            ..
        })
    ));
}

#[test]
fn recursive_pool_growth_obeys_the_round_limit() {
    assert!(matches!(
        limited(
            "p(0).p(Y):-p(X),Y=(X+1;X+2).",
            ExpansionLimits::default(),
            &FormulaLimits {
                max_support_rounds: 3,
                ..FormulaLimits::default()
            }
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportRounds,
            ..
        })
    ));
}

#[test]
fn pooled_scalar_equality_preserves_explicit_facts() {
    assert_eq!(
        native(&input("p(X):-X=(1;2).")),
        native(&input("p(1).p(2)."))
    );
}

#[test]
fn hidden_optimum_ties_and_cancellation_keep_complete_model_identity() {
    use zetesis_cpu::Control;
    let p = input("1{p(1;2)}1.#minimize{1@1:p(1);1@1:p(2)}.#show.");
    let control = Control::default();
    let mut search =
        zetesis_sat::StableModels::new(p.theory(), zetesis_sat::Limits::default(), control.clone())
            .unwrap();
    control.cancel();
    assert!(search.next().unwrap().is_err());
    assert!(!search.exhausted());
    let mut search = zetesis_sat::StableModels::new(
        p.theory(),
        zetesis_sat::Limits::default(),
        Control::default(),
    )
    .unwrap();
    let mut count = 0;
    let mut full_models = Models::new();
    for model in search.by_ref() {
        let model = model.unwrap();
        let model = zetesis_core::Model::new(model.atoms().map(|atom| p.atoms()[atom].clone()));
        let evaluated = zetesis_objective::evaluate(
            p.objectives(),
            &model,
            zetesis_objective::Limits::default(),
            &Control::default(),
        )
        .unwrap();
        assert_eq!(evaluated.score().costs(), &[(1, 1)]);
        let shown = p
            .metadata()
            .observations()
            .render(
                &model,
                p.metadata().output(),
                zetesis_themelios::observation::Limits::default(),
                &Control::default(),
            )
            .unwrap();
        assert_eq!(shown.text(), "");
        assert!(full_models.insert(model.atoms().iter().map(atom_text).collect()));
        count += 1;
    }
    assert!(search.exhausted());
    assert_eq!(count, 2);
    assert_eq!(
        full_models,
        [
            ["p(1)".to_owned()].into_iter().collect(),
            ["p(2)".to_owned()].into_iter().collect()
        ]
        .into_iter()
        .collect()
    );
    println!(
        "hidden optimum full models={full_models:?}, costs=[(1,1)], shown=empty, exhausted=true"
    );
}

#[test]
fn empty_alternatives_never_erase_required_source_safety() {
    for source in [
        "p(Y):-X=(2..1;3..2).",
        "p(2..1;Y);q.",
        "{p(Y;1):X=2..1}.",
        "p:-X=(2..1;Y).",
    ] {
        let error = limited(
            source,
            ExpansionLimits::default(),
            &FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{source}: {error}"
        );
        assert!(!error.diagnostics().is_empty());
    }
}

#[test]
fn pool_free_rules_have_an_inclusive_charged_scan_and_no_pool_projection() {
    for source in ["p:-q.", "{p}.", "p(X):-X=1..2."] {
        let configured = |limit| ExpansionLimits {
            max_term_work: limit,
            ..ExpansionLimits::default()
        };
        let exact = first_success(|limit| {
            limited(source, configured(limit), &FormulaLimits::default()).is_ok()
        });
        let error = limited(source, configured(exact - 1), &FormulaLimits::default()).unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Limit {
                    resource: ExpansionResource::TermWork,
                    ..
                })
            ),
            "{source}: {error}"
        );
        assert_eq!(
            native(&limited(source, configured(exact), &FormulaLimits::default()).unwrap()),
            native(&input(source))
        );
        println!("pool-free inclusive expansion work: {source} => {exact}");
    }
    // The empty program contains no rule scan or pool projection allocation.
    assert!(
        limited(
            "",
            ExpansionLimits {
                max_term_work: 0,
                ..ExpansionLimits::default()
            },
            &FormulaLimits {
                max_analysis_nodes: 0,
                ..FormulaLimits::default()
            }
        )
        .is_ok()
    );
}

#[test]
fn zero_arity_disjunct_carriers_have_an_independent_preclone_node_ceiling() {
    for count in [16, 64] {
        let head = (0..count)
            .map(|index| format!("z{index}"))
            .collect::<Vec<_>>()
            .join(";");
        let source = format!("p(1;2);{head}.");
        let configured = |limit| FormulaLimits {
            max_analysis_nodes: limit,
            ..FormulaLimits::default()
        };
        let exact = first_success(|limit| {
            limited(&source, ExpansionLimits::default(), &configured(limit)).is_ok()
        });
        // Zero-argument predicates still add structural nodes in both complete
        // source copies; a text-only payload counter cannot replace this ceiling.
        assert!(exact >= 2 * (count + 1));
        let error =
            limited(&source, ExpansionLimits::default(), &configured(exact - 1)).unwrap_err();
        assert!(
            matches!(error, FormulaFailure::Limit {
            resource: FormulaResource::AnalysisNodes, observed, location, ..
        } if observed == exact as u128 && !location.span.is_empty()),
            "{error}"
        );
        assert_eq!(
            limited(&source, ExpansionLimits::default(), &configured(exact))
                .unwrap()
                .atoms()
                .len(),
            count + 2
        );
        println!("zero-arity disjuncts={count}, inclusive projection nodes={exact}");
    }
}
