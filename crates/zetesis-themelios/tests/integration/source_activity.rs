//! Rich source activity shares scoped lowering and preserves independent literals.

use zetesis_reference_support::{admit, formula};
use zetesis_themelios::{FormulaFailure, FormulaLimits, FormulaResource};

#[test]
fn constant_aggregate_producers_exclude_negative_domains() {
    for producer in [
        "q:-1=#count{1}.",
        "q:-2=#sum{2}.",
        "q:-2=#min{2}.",
        "q:-2=#max{2}.",
    ] {
        let admitted = formula(&format!("{producer}{{p}}.#project p:not q."));
        assert!(admitted.projection().is_explicit());
        assert!(admitted.projection().atoms().is_empty(), "{producer}");
    }
}

#[test]
fn empty_local_obligations_can_establish_source_facts() {
    for producer in ["q:-r(X):missing(X).", "q:-not r(_)."] {
        let admitted = formula(&format!("{producer}{{p}}.#project p:not q."));
        assert!(admitted.projection().atoms().is_empty(), "{producer}");
    }
}

#[test]
fn contradictory_optional_guards_keep_the_fixed_domain() {
    for program in ["{p;q}.", "{p;r}.q:-1=#count{1:r}."] {
        let admitted = formula(&format!("{program}#project p:q,not q."));
        let domain = admitted.projection().atoms();
        assert_eq!(domain.len(), 1, "{program}");
        assert_eq!(domain.at(0).unwrap().predicate().name(), "p");
        assert!(domain.at(0).unwrap().values().is_empty());
    }
}

#[test]
fn source_activity_keeps_the_original_formula_owner() {
    let source = "{p;r}.q:-1=#count{1:r}.";
    let original = formula(source);
    let observed = formula(&format!("{source}#project p:not q."));
    assert_eq!(original.atoms(), observed.atoms());
    assert_eq!(
        (original.theory().nodes(), original.theory().operands()),
        (observed.theory().nodes(), observed.theory().operands())
    );
    assert_eq!(original.theory().roots(), observed.theory().roots());
    assert_eq!(original.formula_origins(), observed.formula_origins());
}

#[test]
fn rich_activity_does_not_retain_model_query_nodes() {
    let source = "q:-1=#count{1}.#minimize{1:not q}.";
    let mut limits = FormulaLimits::default();
    limits.objective.max_condition_nodes = 0;
    let admitted = admit(source, &limits).unwrap();
    assert!(admitted.objectives().templates().is_empty());
    assert!(admitted.objectives().priorities().is_empty());
}

#[test]
fn rich_activity_work_refusal_is_inclusive() {
    let source = "{p;r}.q:-1=#count{1:r}.#project p:q,not q.";
    let attempt = |max_work| {
        admit(
            source,
            &FormulaLimits {
                max_work,
                ..FormulaLimits::default()
            },
        )
    };
    let (mut lower, mut upper) = (0, 65_536);
    assert!(attempt(upper).is_ok());
    while lower + 1 < upper {
        let middle = lower + (upper - lower) / 2;
        if attempt(middle).is_ok() {
            upper = middle;
        } else {
            lower = middle;
        }
    }
    let accepted = attempt(upper).unwrap();
    assert_eq!(accepted.projection().atoms().len(), 1);
    let failure = attempt(upper - 1).unwrap_err();
    assert!(matches!(failure, FormulaFailure::Limit {
        resource: FormulaResource::Work, limit, observed, ..
    } if limit == u128::from(upper - 1) && observed == u128::from(upper)));
    assert_eq!(attempt(upper).unwrap().projection(), accepted.projection());
}

#[test]
fn a_fact_remains_required_in_its_positive_cycle() {
    let admitted = formula("q.q:-q.{p}.#project p:not q.");
    assert!(admitted.projection().atoms().is_empty());
}

#[test]
fn unsupported_positive_cycles_have_no_source_atoms() {
    let admitted = formula("q:-r.r:-q.{p}.#project p:q.");
    assert!(admitted.projection().atoms().is_empty());
}

#[test]
fn negative_cycles_remain_optional_without_correlation() {
    let admitted = formula("q:-not r.r:-not q.{p}.#project p:q,not q.");
    let domain = admitted.projection().atoms();
    assert_eq!(domain.len(), 1);
    assert_eq!(domain.at(0).unwrap().predicate().name(), "p");
}

#[test]
fn complete_absence_can_establish_required_negation() {
    let admitted = formula("q:-r.r:-q.s:-not q.{p}.#project p:not s.");
    assert!(admitted.projection().atoms().is_empty());
}

#[test]
fn required_cycle_truth_can_eliminate_a_negative_producer() {
    let admitted = formula("q.q:-q.r:-not q.{p}.#project p:r.");
    assert!(admitted.projection().atoms().is_empty());
}
