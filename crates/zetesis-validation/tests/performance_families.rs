//! Generated family sources are exact, bounded and carry closed-form contracts.
use proptest::prelude::*;
use themelios_syntax::{dialect::Dialect, parse::parse_str};
use zetesis_validation::examples::{Family as Selection, Satisfiability};
use zetesis_validation::performance::families::{Error, Family};

fn fibonacci(index: u32) -> u64 {
    let (mut previous, mut current) = (0u64, 1u64);
    for _ in 0..index {
        (previous, current) = (current, previous + current);
    }
    previous
}

#[test]
fn independent_choice_source_is_the_authored_shape() {
    assert_eq!(
        Family::IndependentChoice.source(3).unwrap(),
        "node(1..3).\nedge(1,2). edge(2,3).\n{ in(X) } :- node(X).\n:- edge(X,Y), in(X), in(Y).\n"
    );
}

#[test]
fn independent_negation_source_spells_the_in_out_idiom() {
    assert_eq!(
        Family::IndependentNegation.source(2).unwrap(),
        "node(1..2).\nedge(1,2).\nin(X) :- node(X), not out(X).\nout(X) :- node(X), not in(X).\n:- edge(X,Y), in(X), in(Y).\n"
    );
    assert_eq!(
        Family::IndependentNegationAggregate.source(2).unwrap(),
        format!(
            "{}:- #count{{X:in(X)}} < 0.\n",
            Family::IndependentNegation.source(2).unwrap()
        )
    );
}

#[test]
fn positive_sources_keep_their_derivation_shapes() {
    assert_eq!(
        Family::Disjunction.source(2).unwrap(),
        "d(1..2).\np(X) | q(X) :- d(X).\n"
    );
    assert_eq!(
        Family::Chain.source(2).unwrap(),
        "e(0,1). e(1,2).\nr(0).\nr(Y) :- r(X), e(X,Y).\n"
    );
    assert_eq!(
        Family::ChainArithmetic.source(2).unwrap(),
        "p(0).\np(X+1) :- p(X), X < 2.\n"
    );
    assert_eq!(
        Family::TransitivePath.source(3).unwrap(),
        "e(1,2). e(2,3).\nreach(X,Y) :- e(X,Y).\nreach(X,Z) :- reach(X,Y), e(Y,Z).\n"
    );
    assert_eq!(
        Family::TransitiveDense.source(2).unwrap(),
        "v(1..2).\ne(X,Y) :- v(X), v(Y), X < Y.\nreach(X,Y) :- e(X,Y).\nreach(X,Z) :- reach(X,Y), e(Y,Z).\n"
    );
    assert_eq!(
        Family::ProducerChain.source(3).unwrap(),
        "p1. p2. p3.\nq1 :- p1, p2.\nq2 :- p2, p3.\n"
    );
    assert_eq!(
        Family::Ties.source(3).unwrap(),
        "n(1..3).\n{ p(X) } :- n(X).\n:- #count{X:p(X)} != 2.\n#minimize{ 1,X : q(X) }.\n"
    );
}

#[test]
fn stratified_source_blocks_every_seventh_node_and_keeps_the_last_reachable() {
    let source = Family::Stratified.source(16).unwrap();
    assert!(source.starts_with("node(1..16).\n"));
    assert!(source.contains("bad(3). bad(10).\n"));
    assert!(!source.contains("bad(17)"));
    assert!(source.contains("e(1,3). e(2,4)."));
    assert!(source.ends_with(
        "blocked(Y) :- bad(X), next(X,Y).\nreach(1).\nreach(Y) :- reach(X), e(X,Y), not blocked(Y).\n:- not reach(16).\n"
    ));
}

#[test]
fn complete_family_counts_follow_their_closed_forms() {
    for size in 1..=40 {
        for family in [
            Family::IndependentChoice,
            Family::IndependentNegation,
            Family::IndependentNegationAggregate,
        ] {
            assert_eq!(
                family.contract(size).unwrap().model_count(),
                Some(fibonacci(size + 2)),
                "{family:?} at {size}"
            );
        }
        assert_eq!(
            Family::Disjunction.contract(size).unwrap().model_count(),
            Some(1u64 << size)
        );
    }
    for size in 2..=40 {
        assert_eq!(
            Family::Ties.contract(size).unwrap().model_count(),
            Some(u64::from(size) * u64::from(size - 1) / 2)
        );
    }
    for family in [
        Family::TransitivePath,
        Family::TransitiveDense,
        Family::Chain,
        Family::ChainArithmetic,
        Family::Stratified,
        Family::ProducerChain,
    ] {
        let size = *family.sizes().start();
        let contract = family.contract(size).unwrap();
        assert_eq!(contract.model_count(), Some(1), "{family:?}");
        assert_eq!(contract.family(), Selection::All);
        assert_eq!(contract.satisfiability(), Satisfiability::Sat);
    }
}

#[test]
fn the_tied_optimisation_family_declares_its_zero_cost() {
    let contract = Family::Ties.contract(4).unwrap();
    assert_eq!(contract.family(), Selection::Optimal);
    assert_eq!(contract.cost(), Some(&[0][..]));
    assert_eq!(contract.model_count(), Some(6));
    assert_eq!(Family::Disjunction.contract(5).unwrap().cost(), None);
}

#[test]
fn sizes_outside_the_admitted_range_are_refused() {
    assert!(matches!(
        Family::Disjunction.source(64),
        Err(Error::Size {
            family: Family::Disjunction,
            size: 64
        })
    ));
    assert!(matches!(
        Family::IndependentChoice.contract(0),
        Err(Error::Size { .. })
    ));
    assert!(matches!(Family::Ties.source(1), Err(Error::Size { .. })));
    assert!(Family::Chain.source(*Family::Chain.sizes().end()).is_ok());
    assert!(
        Family::Chain
            .source(Family::Chain.sizes().end() + 1)
            .is_err()
    );
}

#[test]
fn labels_are_distinct_file_stems() {
    let labels: std::collections::BTreeSet<_> = Family::ALL.iter().map(|f| f.label()).collect();
    assert_eq!(labels.len(), Family::ALL.len());
    for label in labels {
        assert!(
            label
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'-'),
            "{label}"
        );
    }
}

proptest! {
    #[test]
    fn every_admitted_source_parses_without_diagnostics(index in 0..Family::ALL.len(), size in 1u32..=4096) {
        let family = Family::ALL[index];
        let sizes = family.sizes();
        let size = size.clamp(*sizes.start(), *sizes.end());
        let source = family.source(size).unwrap();
        prop_assert_eq!(&source, &family.source(size).unwrap());
        let parsed = parse_str(&source, Dialect::Clingo).unwrap();
        prop_assert!(parsed.diagnostics().is_empty(), "{}", source);
        prop_assert!(family.contract(size).is_ok());
    }
}
