//! Full-vocabulary original and arbitrary frozen truth control factorization.
use super::testing::{self as route, Counts, Point};
use crate::formula::Compiled;
use crate::formula_ground::{ground, ground_retained};
use crate::formula_support::testing;
use crate::{ExpansionFailure, FormulaFailure, FormulaResource};
use zetesis_core::{Atom, ValueLimits};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Limits, models, models_reduct};

fn compile(source: &str, factored: bool) -> (Compiled, Counts) {
    let (result, counts) = route::scoped(factored, || ground(testing::prepare(source), None, None));
    (result.unwrap(), counts)
}

fn atoms(compiled: &Compiled) -> Vec<Atom> {
    compiled
        .atoms
        .atoms()
        .iter()
        .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
        .collect()
}

fn interpretation(compiled: &Compiled, vocabulary: &[Atom], bits: usize) -> Interpretation {
    Interpretation::new(
        &compiled.theory,
        atoms(compiled)
            .iter()
            .enumerate()
            .filter_map(|(index, atom)| {
                (bits & (1 << vocabulary.binary_search(atom).unwrap()) != 0).then_some(index)
            }),
    )
    .unwrap()
}

fn same_truth(source: &str) -> Counts {
    let (shared, counts) = compile(source, true);
    let (reference, _) = compile(source, false);
    let mut vocabulary = atoms(&shared);
    vocabulary.sort();
    let mut original = atoms(&reference);
    original.sort();
    assert_eq!(vocabulary, original, "{source}");
    assert!(
        vocabulary.len() <= 8,
        "bounded complete truth table: {source}"
    );
    assert_eq!(shared.warnings, reference.warnings, "{source}");
    let origins = |compiled: &Compiled| {
        let mut sites: Vec<_> = compiled.origins.iter().flatten().copied().collect();
        sites.sort();
        sites.dedup();
        sites
    };
    assert_eq!(origins(&shared), origins(&reference), "{source}");
    let interpretations: Vec<_> = (0..1 << vocabulary.len())
        .map(|bits| {
            (
                interpretation(&shared, &vocabulary, bits),
                interpretation(&reference, &vocabulary, bits),
            )
        })
        .collect();
    let limits = Limits::default();
    let cancellation = Cancellation::default();
    for (candidate, (left, right)) in interpretations.iter().enumerate() {
        assert_eq!(
            models(&shared.theory, left, limits, &cancellation).unwrap(),
            models(&reference.theory, right, limits, &cancellation).unwrap(),
            "original {candidate}: {source}",
        );
        for (tested, (left_tested, right_tested)) in interpretations.iter().enumerate() {
            assert_eq!(
                models_reduct(&shared.theory, left, left_tested, limits, &cancellation).unwrap(),
                models_reduct(
                    &reference.theory,
                    right,
                    right_tested,
                    limits,
                    &cancellation
                )
                .unwrap(),
                "frozen {candidate}, tested {tested}: {source}",
            );
        }
    }
    counts
}

#[test]
fn witness_factoring_preserves_formula_semantics() {
    for source in [
        "{p(1);p(2);r(1);r(2);a}. q(N) :- p(X), r(X), N=#count{1:a}.",
        "{p(1);p(2)}. a :- q(1). q(N) :- p(_), N=#count{1:a}.",
        "{p(1);p(2);a;b}. q(N) :- p(_), N=#count{1:a}, not not b.",
        "d(0;a). {p}. q(N) :- d(X), N=#count{X:p;0:p}.",
        "d(0;1). {p}. q(N) :- d(X), N=#count{Z:d(Z),Z=X,not p}.",
        "d(0;1). {p}. q(N) :- d(X), N=#count{1:p}, #count{1:p}!=X.",
        "d(0,a).d(0,b). {p}. q(Y,N) :- d(_,Y), N=#count{1:p}.",
        "d(0,a).d(0,b).d(1,a). {p}. q(N+Y) :- d(Y,_), N=#count{1:p}.",
    ] {
        assert!(same_truth(source).runs > 0, "checked route: {source}");
    }
}

#[test]
fn equal_witnesses_expand_one_continuation() {
    let source = "d(1..12). {p(1..3)}. q(A,B) :- d(_), A=#count{X:p(X)}, B=#count{X:p(X)}.";
    let (shared, counts) = route::scoped(true, || {
        ground_retained(testing::prepare(source), None, false).unwrap()
    });
    let (reference, _) = route::scoped(false, || {
        ground_retained(testing::prepare(source), None, false).unwrap()
    });
    assert_eq!(
        counts,
        Counts {
            runs: 1,
            witnesses: 12,
            continuations: 16
        }
    );
    assert!(shared.accounting.work < reference.accounting.work);
    let mut bounded = testing::prepare(source);
    bounded.limits.max_work = shared.accounting.work;
    route::scoped(true, || ground_retained(bounded, None, false))
        .0
        .unwrap();
    let mut bounded = testing::prepare(source);
    bounded.limits.max_work = shared.accounting.work;
    assert!(matches!(
        route::scoped(false, || ground_retained(bounded, None, false)).0,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            ..
        })
    ));
}

#[test]
fn interleaved_keys_keep_separate_continuations() {
    let source = "d(1,0).d(2,1).d(3,0).d(4,0). {a}. q(K,N) :- d(_,K), N=#count{1:a}.";
    let (_, counts) = compile(source, true);
    assert_eq!(
        counts,
        Counts {
            runs: 3,
            witnesses: 4,
            continuations: 6
        }
    );
    let source = "d(1,0).d(2,1).d(3,0). {a}. q(N) :- d(_,K), N=#sum{K:a}.";
    assert_eq!(same_truth(source).runs, 3);
}

#[test]
fn rejected_families_publish_no_witnesses() {
    for source in [
        "d(1;2). {a}. q(N) :- d(_), N=#count{1:a}, N=3.",
        "d(1;2). {a}. q(N,X) :- d(_), N=#count{1:a}, X=2..N.",
    ] {
        let counts = same_truth(source);
        assert_eq!(counts.runs, 1);
        assert_eq!(counts.witnesses, 0);
        assert_eq!(counts.continuations, 0);
    }
}

fn same_error(source: &str) {
    let (shared, _) = route::scoped(true, || ground(testing::prepare(source), None, None));
    let (reference, _) = route::scoped(false, || ground(testing::prepare(source), None, None));
    match (shared.unwrap_err(), reference.unwrap_err()) {
        (
            FormulaFailure::Expansion(ExpansionFailure::Evaluation { error, location }),
            FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                error: expected,
                location: site,
            }),
        ) => {
            assert_eq!(error, expected);
            assert_eq!(location, site);
        }
        errors => panic!("expected identical typed arithmetic failures: {errors:?}"),
    }
}

#[test]
fn witness_factoring_preserves_arithmetic_evidence() {
    for source in [
        "d(0,a).d(0,b). {p}. q(N) :- d(X,_), N=#count{1:p,1/X>0}.",
        "d(0,a).d(0,b). {p}. q(N+1/X) :- d(X,_), N=#count{1:p}.",
    ] {
        same_error(source);
    }
    let source = "d(0,a).d(0,b).d(1,a). {p}. q(N) :- d(X,_), N=#count{1:p}, N/X>0.";
    assert!(same_truth(source).runs > 0);
}

#[test]
fn partial_prefixes_keep_complete_traversal() {
    for source in [
        "d(1;2). {p}. q(N) :- d(X), 1/X>0, N=#count{1:p}.",
        "d(f(1);f(2)). {p}. q(N) :- d(f(_)), N=#count{1:p}.",
        "d(1;2). {p}. {q(N)} :- d(_), N=#count{1:p}.",
    ] {
        assert_eq!(same_truth(source).runs, 0);
    }
}

#[test]
fn interrupted_factoring_never_publishes_a_theory() {
    let source = "d(1;2;3). {p}. q(N) :- d(_), N=#count{1:p}.";
    for point in [Point::Witness, Point::Continuation] {
        let cancellation = Cancellation::default();
        let mut preparation = testing::prepare(source);
        preparation.budget = preparation
            .budget
            .with_cancellation(Some(cancellation.clone()));
        let (result, counts) = route::controlled(true, Some((point, 2, cancellation)), || {
            ground(preparation, None, None)
        });
        assert!(matches!(result, Err(FormulaFailure::Interrupted { .. })));
        assert_eq!(counts.runs, 1);
    }
}

#[test]
fn lookahead_preserves_active_head_continuations() {
    let source = "d(1,1,a).d(2,1,b).d(3,0,a). {p}. q(1..(N+1)) :- d(_,K,_), N=#count{1:p}, N=K.";
    assert_eq!(
        same_truth(source),
        Counts {
            runs: 2,
            witnesses: 3,
            continuations: 3
        }
    );
}

#[test]
fn false_bodies_keep_selected_witness_vocabulary() {
    let source = "d(1;2). {p}. q(N) :- d(_), N=#count{1:p}, #count{1:p}=-1.";
    assert_eq!(
        same_truth(source),
        Counts {
            runs: 1,
            witnesses: 2,
            continuations: 2
        }
    );
}
