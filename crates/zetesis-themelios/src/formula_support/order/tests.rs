use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use themelios_program::program::{DefaultNegation, Relation};
use zetesis_core::{AtomPattern, Predicate, Sign, Term};

use super::arrange;
use crate::ExpansionLimits;
use crate::expansion::Budget;
use crate::formula_ir::{Expression, LiteralIr, Operation};
use crate::formula_support::{PatternOccurrence, PositivePattern};

fn location() -> Location {
    Location {
        source: SourceId::new(37),
        span: Span::empty(ByteOffset::new(12)),
    }
}

fn atom(name: &str, variables: &[usize]) -> LiteralIr {
    let pattern = AtomPattern::new(
        Predicate::with_sign(name, variables.len(), Sign::Positive).unwrap(),
        variables
            .iter()
            .map(|&variable| Term::Variable(variable))
            .collect(),
    )
    .unwrap();
    LiteralIr::Atom(DefaultNegation::None, pattern)
}

fn less(left: usize, right: usize) -> LiteralIr {
    let variable = |variable| Expression {
        nodes: vec![Operation::Variable(variable)],
    };
    LiteralIr::Compare(variable(left), Relation::Lt, variable(right))
}

/// The predicate names of `literals` in the order `arrange` chooses, with
/// `sizes` giving each positive occurrence's relation size in source order.
fn order(literals: &[LiteralIr], sizes: &[usize], variables: usize) -> Vec<String> {
    let mut occurrences: Vec<_> = literals
        .iter()
        .enumerate()
        .filter_map(|(source, literal)| match literal {
            LiteralIr::Atom(_, pattern) => Some(PatternOccurrence {
                pattern: PositivePattern::Flat(pattern),
                source,
            }),
            _ => None,
        })
        .collect();
    let positions: Vec<usize> = occurrences.iter().map(|o| o.source).collect();
    let mut bound = vec![false; variables];
    let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
    arrange(
        &mut occurrences,
        literals,
        &mut bound,
        |occurrence| {
            sizes[positions
                .iter()
                .position(|&p| p == occurrence.source)
                .unwrap()]
        },
        &mut budget,
        location(),
    )
    .unwrap();
    occurrences
        .iter()
        .map(|occurrence| occurrence.atom().predicate().name().to_owned())
        .collect()
}

#[test]
fn a_comparison_is_decided_before_an_unrelated_occurrence_of_the_same_size() {
    // d(W), d(X), d(Y) in canonical order with X < Y: d(X) and d(Y) each
    // bind a variable the comparison waits on and d(W) none, so d(X) comes
    // first by canonical order among them, d(Y) then decides the comparison
    // and d(W) joins the surviving rows.
    let literals = [
        atom("dw", &[0]),
        atom("dx", &[1]),
        atom("dy", &[2]),
        less(1, 2),
    ];
    assert_eq!(order(&literals, &[40, 40, 40], 3), ["dx", "dy", "dw"]);
}

#[test]
fn a_smaller_relation_precedes_a_comparison_decider() {
    let literals = [atom("big", &[0]), atom("small", &[1]), less(0, 0)];
    assert_eq!(order(&literals, &[100, 10], 2), ["small", "big"]);
}

#[test]
fn a_bound_occurrence_is_a_test_and_precedes_every_generator() {
    // a(X) binds X; c(X) is then a test although its relation is the largest.
    let literals = [atom("a", &[0]), atom("b", &[0, 1]), atom("c", &[0])];
    assert_eq!(order(&literals, &[10, 500, 1000], 2), ["a", "c", "b"]);
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config { cases: 256, rng_seed: proptest::test_runner::RngSeed::Fixed(20_260_917), ..Default::default() })]
    /// Without comparisons or shared variables the order is the stable sort
    /// by relation size.
    #[test]
    fn without_comparisons_disjoint_occurrences_sort_stably_by_size(sizes in proptest::collection::vec(0_usize..4, 0..8)) {
        let literals: Vec<_> = sizes
            .iter()
            .enumerate()
            .map(|(index, _)| atom(&format!("p{index}"), &[index]))
            .collect();
        let mut expected: Vec<(usize, usize)> = sizes.iter().copied().enumerate().map(|(index, size)| (size, index)).collect();
        expected.sort_unstable();
        let expected: Vec<String> = expected.iter().map(|(_, index)| format!("p{index}")).collect();
        proptest::prop_assert_eq!(order(&literals, &sizes, sizes.len()), expected);
    }
}
