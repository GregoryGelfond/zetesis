//! Full-vocabulary original/frozen truth and actual route assertions.

mod boundaries;
mod provenance;

use super::testing::{self as route, Counts};
use crate::formula::Compiled;
use crate::formula_ground::ground;
use crate::formula_support::testing;
use crate::test_support::compiled_atoms;
use zetesis_algebra::Cancellation;
use zetesis_core::Atom;
use zetesis_ferraris::{Interpretation, Limits, NodeView, models, models_reduct};

fn compile(source: &str, enabled: bool) -> (Compiled, Counts) {
    let (result, counts) = route::scoped(enabled, || ground(testing::prepare(source), None, None));
    (result.unwrap(), counts)
}

fn interpretation(compiled: &Compiled, vocabulary: &[Atom], bits: usize) -> Interpretation {
    Interpretation::new(
        &compiled.theory,
        compiled_atoms(compiled)
            .iter()
            .enumerate()
            .filter_map(|(index, atom)| {
                (bits & (1 << vocabulary.binary_search(atom).unwrap()) != 0).then_some(index)
            }),
    )
    .unwrap()
}

fn same_truth(source: &str) -> Counts {
    let (actual, counts) = compile(source, true);
    let (reference, ordinary) = compile(source, false);
    assert_eq!(ordinary, Counts::default());
    let mut vocabulary = compiled_atoms(&actual);
    vocabulary.sort();
    let mut original = compiled_atoms(&reference);
    original.sort();
    assert_eq!(vocabulary, original, "{source}");
    assert!(vocabulary.len() <= 7, "bounded complete truth population");
    assert_eq!(actual.warnings, reference.warnings);
    assert_eq!(actual.objective_origins, reference.objective_origins);
    assert_eq!(
        actual.objective_declarations,
        reference.objective_declarations
    );
    // Root counts/positions may change. Every original source site stays the
    // source of its group; no unrelated rule's evidence is pooled into it.
    let origins = |compiled: &Compiled| {
        let mut result: Vec<_> = compiled.origins.iter().flatten().copied().collect();
        result.sort();
        result.dedup();
        result
    };
    assert_eq!(origins(&actual), origins(&reference), "{source}");
    let readings: Vec<_> = (0..1 << vocabulary.len())
        .map(|bits| {
            (
                interpretation(&actual, &vocabulary, bits),
                interpretation(&reference, &vocabulary, bits),
            )
        })
        .collect();
    let cancellation = Cancellation::default();
    for (m, (a, r)) in readings.iter().enumerate() {
        assert_eq!(
            models(&actual.theory, a, Limits::default(), &cancellation).unwrap(),
            models(&reference.theory, r, Limits::default(), &cancellation).unwrap(),
            "M={m}: {source}"
        );
        for (j, (aj, rj)) in readings.iter().enumerate() {
            assert_eq!(
                models_reduct(&actual.theory, a, aj, Limits::default(), &cancellation).unwrap(),
                models_reduct(&reference.theory, r, rj, Limits::default(), &cancellation).unwrap(),
                "M={m}, J={j}: {source}"
            );
        }
    }
    counts
}

#[test]
fn grouped_source_preserves_original_and_arbitrary_frozen_readings() {
    for source in [
        "{p(1);p(2);p(3)}. :- p(X),p(Y),X!=Y.",
        "{p(1..4)}. :- p(X),p(Y),X!=Y.",
        "{p(1,1);p(1,2);p(1,3);p(2,1);p(2,2)}. :- p(G,X),p(G,Y),X!=Y.",
        "{p(1);p(2);h}. g :- h. :- p(X),p(Y),g,X!=Y.",
        "d(1;2). {g}. :- d(X),d(Y),g,X!=Y.",
        "{-p(1);-p(2);p(1)}. :- -p(X),-p(Y),X!=Y.",
        "{p(1);p(a);p(f(1))}. :- p(X),p(Y),X!=Y.",
        "{p(0,a);p(0,b);p(1,a);p(1,b)}. :- p(K,X),p(K,Y),X!=Y.",
        "{p(1);p(2)}. :- p(X),p(X),p(Y),p(Y),X!=Y.",
    ] {
        let counts = same_truth(source);
        assert_eq!(counts.accepted, 1, "actual grouped source: {source}");
        assert!(counts.first_rows > 1 && counts.second_rows > 1 && counts.groups > 0);
    }
}

#[test]
fn singleton_or_empty_fibers_do_not_name_or_publish() {
    for source in [
        ":- p(X),p(Y),X!=Y.",
        "p(1). :- p(X),p(Y),g,X!=Y. {g}.",
        "p(0,a). p(1,b). :- p(K,X),p(K,Y),X!=Y.",
    ] {
        let counts = same_truth(source);
        assert_eq!(counts.accepted, 1);
        assert_eq!(counts.second_rows, 0);
        assert_eq!(counts.groups, 0);
    }
}

#[test]
fn equal_formula_ids_keep_distinct_binding_occurrences() {
    let source = "d(1;2). {g}. :- d(X),d(Y),g,X!=Y.";
    let (actual, counts) = compile(source, true);
    assert_eq!(counts.groups, 1);
    assert!(
        (0..actual.theory.view().len()).any(|node| {
            matches!(actual.theory.view().node(node).unwrap(), NodeView::Threshold(row)
            if row.bound == 2 && row.weights == [1, 1]
                && row.conditions.len() == 2 && row.conditions[0] == row.conditions[1])
        }),
        "two fixed-true lanes must survive as two native occurrences"
    );
    same_truth(source);
}

#[test]
fn unsupported_correlation_or_literal_grammar_keeps_the_ordinary_route() {
    for source in [
        "{p(1);p(2)}. :- p(X),p(Y),X!=Y,X<Y.",
        "{p(1);p(2);q}. :- p(X),p(Y),not q,X!=Y.",
        "{p(f(1));p(f(2))}. :- p(f(X)),p(f(Y)),X!=Y.",
        "{p(1);q(2)}. :- p(X),q(Y),X!=Y.",
        "{p(1,2);p(2,1)}. :- p(X,Y),X!=Y.",
        "{p(1);p(2)}. edge(1,2). :- p(X),p(Y),edge(X,Y),X!=Y.",
    ] {
        assert_eq!(same_truth(source), Counts::default(), "{source}");
    }
}
