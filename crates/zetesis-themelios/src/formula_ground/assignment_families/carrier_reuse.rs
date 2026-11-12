//! Shared carriers preserve emission and arbitrary original/frozen truth.
//! Slice1 uses only actual carrier events; occurrence replay remains absent.
use super::*;
use crate::formula_ground::families;

fn compile_pair(source: &str) -> (Compiled, Compiled, families::testing::Counts) {
    let (shared, used) = families::testing::scoped(true, || {
        let result = ground(testing::prepare(source), None, None);
        (result, families::testing::counts())
    });
    let (ordinary, unused) = families::testing::scoped(false, || {
        let result = ground(testing::prepare(source), None, None);
        (result, families::testing::counts())
    });
    assert!(
        used.produced > 0,
        "must retain a completed family: {source}"
    );
    assert!(
        used.carrier_borrowed > 0,
        "must borrow the shared proposal: {source}"
    );
    assert_eq!(unused.produced, 0);
    assert_eq!(unused.reused, 0);
    assert_eq!(unused.carrier_borrowed, 0);
    assert_eq!(unused.consumed, 0);
    (shared.unwrap(), ordinary.unwrap(), used)
}

fn same_emission(shared: &Compiled, ordinary: &Compiled, source: &str) {
    assert_eq!(shared.atoms.atoms(), ordinary.atoms.atoms(), "{source}");
    assert_eq!(shared.origins, ordinary.origins, "{source}");
    assert_eq!(
        shared.objective_origins, ordinary.objective_origins,
        "{source}"
    );
    assert_eq!(
        shared.objective_declarations, ordinary.objective_declarations,
        "{source}"
    );
    assert_eq!(shared.warnings, ordinary.warnings, "{source}");
    assert_eq!(shared.theory.roots(), ordinary.theory.roots(), "{source}");
    assert_eq!(
        shared.theory.view().len(),
        ordinary.theory.view().len(),
        "{source}"
    );
    for node in 0..shared.theory.view().len() {
        // NodeView includes complete ordered Boolean operands and native
        // threshold/extremum payloads, not merely the raw row identifier.
        assert_eq!(
            shared.theory.view().node(node).unwrap(),
            ordinary.theory.view().node(node).unwrap(),
            "logical node {node}: {source}",
        );
    }
}

pub(super) fn same_truth(source: &str) -> families::testing::Counts {
    let (shared, ordinary, used) = compile_pair(source);
    same_emission(&shared, &ordinary, source);
    assert!(
        shared.theory.atom_count() <= 7,
        "keep exhaustive M/J finite: {source}"
    );
    let width = 1 << shared.theory.atom_count();
    let limits = Limits::default();
    let cancellation = Cancellation::default();
    for candidate in 0..width {
        let left = interpretation(&shared.theory, candidate);
        let right = interpretation(&ordinary.theory, candidate);
        assert_eq!(
            models(&shared.theory, &left, limits, &cancellation).unwrap(),
            models(&ordinary.theory, &right, limits, &cancellation).unwrap(),
            "original M={candidate}: {source}",
        );
        for tested in 0..width {
            assert_eq!(
                models_reduct(
                    &shared.theory,
                    &left,
                    &interpretation(&shared.theory, tested),
                    limits,
                    &cancellation,
                )
                .unwrap(),
                models_reduct(
                    &ordinary.theory,
                    &right,
                    &interpretation(&ordinary.theory, tested),
                    limits,
                    &cancellation,
                )
                .unwrap(),
                "frozen M={candidate}, J={tested}: {source}",
            );
        }
    }
    used
}

#[test]
fn shared_carrier_preserves_irrelevant_outer_bindings() {
    let source = "d(0;1). {a}. ok(X,N) :- d(X), N=#count{1:a}.";
    let used = same_truth(source);
    assert_eq!(
        used.produced, 1,
        "one source aggregate and one empty input key"
    );
    // This source resets one Cursor's existing carrier. The separate
    // independent-empty-frames test requires an actual global registry hit.
}

#[test]
fn shared_carrier_separates_changed_read_inputs() {
    let source = "d(0;1). {a}. ok :- d(X), N=#count{X:a;0:a}.";
    let used = same_truth(source);
    assert_eq!(used.produced, 2, "the tuple reads both distinct X values");
}

#[test]
fn shared_carrier_keeps_local_ranges_and_structured_reads() {
    for source in [
        "d(0;1). {a}. ok :- d(X), N=#count{Y:Y=0..X,a}, N=1.",
        "d(0;1). {a}. ok :- d(X), N=#count{f(X+1):a}, N=1.",
        "d(f(0);f(1)). {a}. ok :- d(f(X)), N=#count{Y:d(f(Y)),Y=X,a}, N=1.",
        "{a}. ok :- A=#count{1:a}, B=#sum{A:a;2,A:not a}, B=A.",
    ] {
        same_truth(source);
    }
}

#[test]
fn shared_carrier_keeps_signed_required_and_typed_values() {
    for source in [
        "{a;b}. ok :- N=#sum{-3:a;2:b;2:a}, N=-1.",
        "{a}. ok :- N=#sum{2;2:a;3:a;name:a}, N=2.",
        "{a}. ok :- N=#sum+{-2:a;0:a;2:a}, N=2.",
        "{a}. ok :- N=#min{4:a}, N=4.",
        "{a}. ok :- N=#max{f(a):a}, N=f(a).",
    ] {
        same_truth(source);
    }
}

#[test]
fn fatal_source_failure_keeps_exact_diagnostics_without_publication() {
    let source = "d(1). {a}. ok :- d(X), N=#count{1:a,1/(1-X)>0}.";
    let (shared, events) = families::testing::scoped(true, || {
        let result = ground(testing::prepare(source), None, None);
        (result.unwrap_err(), families::testing::counts())
    });
    let ordinary = families::testing::scoped(false, || {
        ground(testing::prepare(source), None, None).unwrap_err()
    });
    assert!(matches!(
        shared,
        FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation { .. })
    ));
    assert_eq!(shared.to_string(), ordinary.to_string());
    assert_eq!(shared.diagnostics(), ordinary.diagnostics());
    assert_eq!(events.produced, 0);
    assert_eq!(events.reused, 0);
    assert_eq!(events.carrier_borrowed, 0);
    assert_eq!(events.consumed, 0);
}
