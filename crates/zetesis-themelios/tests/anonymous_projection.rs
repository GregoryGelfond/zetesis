//! Anonymous default-negated arguments project complete relation identities.

use std::collections::BTreeSet;

use zetesis_core::Atom;
use zetesis_cpu::Cancellation;
use zetesis_sat::{Limits, StableModels};
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaFailure, FormulaLimits, admit_formula,
};

fn models(source: &str) -> BTreeSet<BTreeSet<Atom>> {
    let input = admit_formula(
        source.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"));
    let mut search = StableModels::new(input.theory(), Limits::default(), Cancellation::default())
        .expect("admitted theory");
    let result = search
        .by_ref()
        .map(|model| {
            model
                .expect("complete reduct check")
                .atoms()
                .map(|index| input.atoms()[index].clone())
                .collect()
        })
        .collect();
    assert!(search.exhausted());
    result
}

fn expected(sources: &[&str]) -> BTreeSet<BTreeSet<Atom>> {
    sources.iter().flat_map(|source| models(source)).collect()
}

#[test]
fn empty_and_nonempty_projections_keep_not_and_not_not_distinct() {
    assert_eq!(models("p:-not q(_)."), models("p."));
    assert_eq!(models("p:-not not q(_)."), models(""));
    assert_eq!(
        models("{q(1);q(2)}.p:-not q(_)."),
        expected(&["p.", "q(1).", "q(2).", "q(1;2)."])
    );
    assert_eq!(
        models("{q(1);q(2)}.p:-not not q(_)."),
        expected(&["", "q(1).p.", "q(2).p.", "q(1;2).p."])
    );
}

#[test]
fn named_arguments_remain_bound_and_anonymous_positions_are_independent() {
    assert_eq!(
        models("d(1..2).q(1,a,b).p(X):-d(X),not q(X,_,_)."),
        models("d(1..2).q(1,a,b).p(2).")
    );
    assert_eq!(
        models("d(1..2).q(1,1,a).q(1,2,b).p(X):-d(X),not q(X,X,_)."),
        models("d(1..2).q(1,1,a).q(1,2,b).p(2).")
    );
    assert_eq!(
        models("q(\"a b\",1).p:-not q(\"a b\",_)."),
        models("q(\"a b\",1).")
    );
    assert!(matches!(
        admit_formula(
            "p(X):-not q(X,_).".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default()
        ),
        Err(FormulaFailure::UnsafeVariable { .. })
    ));
}

#[test]
fn recursive_projection_and_local_eligibility_remain_in_the_reduct() {
    assert!(models("p:-not q(_).q(1):-p.").is_empty());
    assert_eq!(
        models("p:-not not q(_).q(1):-p."),
        expected(&["", "p.q(1)."])
    );
    assert_eq!(
        models("{q(1);q(2)}.n(N):-N=#count{1:not q(_)}."),
        expected(&["n(1).", "q(1).n(0).", "q(2).n(0).", "q(1;2).n(0)."])
    );
    assert_eq!(
        models("{q(1);q(2)}.{p:not q(_)}."),
        expected(&["", "p.", "q(1).", "q(2).", "q(1;2)."])
    );
}
