//! Generative scalar instructions preserve scoped bindings and whole-model identity.

use std::collections::BTreeSet;

use zetesis_core::{Atom, Predicate, Value};
use zetesis_cpu::Control;
use zetesis_sat::{Limits, StableModels};
use zetesis_themelios::{
    AdmissionOptions, ExpansionFailure, ExpansionLimits, FormulaFailure, FormulaLimits,
    FormulaResource, admit_formula,
};

fn models(source: &str) -> BTreeSet<BTreeSet<Atom>> {
    let admitted = admit_formula(
        source.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"));
    let mut search = StableModels::new(admitted.theory(), Limits::default(), Control::default())
        .expect("theory admitted");
    let result = search
        .by_ref()
        .map(|result| {
            result
                .expect("complete reduct queries")
                .atoms()
                .map(|index| admitted.atoms()[index].clone())
                .collect()
        })
        .collect();
    assert!(search.exhausted(), "{source}");
    result
}

#[test]
fn scalar_dependencies_preserve_reordered_bindings() {
    for source in [
        "p(X,Y,Z):-X=2,Y=X+1,Z=Y*2.",
        "p(X,Y,Z):-Z=Y*2,Y=X+1,X=2.",
        "p(X,Y,Z):-Y*2=Z,X+1=Y,2=X.",
    ] {
        assert_eq!(models(source), models("p(2,3,6)."));
    }
}

#[test]
fn bound_scalar_equalities_filter_existing_values() {
    assert_eq!(models("p(X):-X=1,X=2."), models(""));
    assert_eq!(models("p(X):-X=1,1=X."), models("p(1)."));
}

#[test]
fn nonnumeric_fact_ranges_match_bound_ranges() {
    for (lower, upper) in [("a", "b"), ("1", "a"), ("a", "1"), ("#inf", "#sup")] {
        let facts = format!("n({lower},{upper}).p({lower}..{upper}).");
        let bound = format!("n({lower},{upper}).p(X):-n(L,U),X=L..U.");
        assert_eq!(models(&facts), models(&bound), "{facts}");
    }
}

#[test]
fn affine_equalities_generate_exact_integer_bindings() {
    for (source, number) in [
        ("p(X):-X+1=2.", 1),
        ("p(X):-2=X+1.", 1),
        ("p(X):-X+1=3.", 2),
    ] {
        let atom = Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(number)]).unwrap();
        let expected = BTreeSet::from([BTreeSet::from([atom])]);
        assert_eq!(models(source), expected, "{source}");
    }
}

#[test]
fn unanchored_scalar_dependencies_remain_unsafe() {
    for source in ["p(X):-X=Y,Y=X.", "p(X):-X=1..X."] {
        assert!(
            matches!(
                admit_formula(
                    source.to_owned(),
                    AdmissionOptions::default(),
                    ExpansionLimits::default(),
                    FormulaLimits::default()
                ),
                Err(FormulaFailure::UnsafeVariable { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn separate_interval_cursors_restore_bindings_and_bound_targets_are_filters() {
    assert_eq!(
        models("p(X,Y):-Y=X..X+1,X=1..2."),
        models("p(1,1).p(1,2).p(2,2).p(2,3).")
    );
    assert_eq!(
        models("p(X,Y):-1..2=X,3..4=Y,X+Y!=5."),
        models("p(1,3).p(2,4).")
    );
    assert_eq!(
        models("d(0..3).p(X):-d(X),X=1..2."),
        models("d(0..3).p(1..2).")
    );
    assert_eq!(models("p(X):-X=2..1."), models(""));
    assert_eq!(models("d(a).p(X):-d(X),X=1..2."), models("d(a)."));
    assert_eq!(models("d(a;2).p(0..X):-d(X)."), models("d(a;2).p(0..2)."));
    assert_eq!(models("d(a).p(X):-d(X),Y=0..X."), models("d(a)."));
    for function in ["min", "max"] {
        let producer = format!("n(N):-N=#{function}{{}}.");
        assert_eq!(
            models(&format!("{producer}p(0..N):-n(N).")),
            models(&producer)
        );
    }
}

#[test]
fn generated_normal_heads_extend_the_carrier_and_preserve_each_interval_product() {
    assert_eq!(models("d(2).p(X+1):-d(X)."), models("d(2).p(3)."));
    assert_eq!(
        models("d(2).p(X..X+1,X..X+1):-d(X)."),
        models("d(2).p(2,2).p(2,3).p(3,2).p(3,3).")
    );
    assert_eq!(
        models("start(2).duration(2).end(E):-start(S),duration(D),E=S+D.:-end(E),E>3."),
        BTreeSet::new()
    );
    assert_eq!(models("p(0).p(X+1):-p(X),X<3."), models("p(0..3)."));
}

#[test]
fn local_bindings_do_not_escape_choice_and_aggregate_scopes() {
    assert_eq!(models("{p(X):X=1..2}."), models("{p(1);p(2)}."));
    assert_eq!(models("n(N):-N=#count{X:X=1..2}."), models("n(2)."));
    assert_eq!(
        models("d(1).n(D,N):-d(D),N=#count{X:X=D..D+1}."),
        models("d(1).n(1,2).")
    );
    assert!(matches!(
        admit_formula(
            "p(X):-#count{Y:Y=1..2}>0.".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default()
        ),
        Err(FormulaFailure::UnsafeVariable { .. })
    ));
}

#[test]
fn value_ranges_recursive_growth_and_arithmetic_have_typed_refusals() {
    for (source, limits, resource) in [
        (
            "p(X):-X=1..3.",
            FormulaLimits {
                max_assignment_values: 2,
                ..FormulaLimits::default()
            },
            FormulaResource::AssignmentValues,
        ),
        (
            "p(0).p(X+1):-p(X).",
            FormulaLimits {
                max_support_rounds: 3,
                ..FormulaLimits::default()
            },
            FormulaResource::SupportRounds,
        ),
        (
            "p(X):-X=1..2147483647.",
            FormulaLimits::default(),
            FormulaResource::AssignmentValues,
        ),
    ] {
        let failure = admit_formula(
            source.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits,
        )
        .expect_err("no partial carrier may be admitted");
        assert!(
            matches!(failure, FormulaFailure::Limit { resource:actual, .. } if actual == resource)
        );
        assert!(!failure.diagnostics().is_empty());
    }
    for source in ["d(0).p(Y):-d(X),Y=1/X.", "d(2147483647).p(X+1):-d(X)."] {
        assert!(matches!(
            admit_formula(
                source.to_owned(),
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                FormulaLimits::default()
            ),
            Err(FormulaFailure::Expansion(
                ExpansionFailure::Evaluation { .. }
            ))
        ));
    }
}
