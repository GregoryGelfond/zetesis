//! Possible-head continuations omit no original formula witness.
use super::*;
use crate::formula::Preparation;
use zetesis_core::Atom;

fn prepare(source: &str, reuse: bool) -> Preparation {
    let mut preparation = testing::prepare(source);
    if !reuse {
        for rule in &mut preparation.program.rules {
            if let Some(plan) = &mut rule.bindings {
                // The reference changes only the producer-continuation policy.
                // Generator readiness, carrier reuse and formula keys agree.
                plan.continuation_inputs = None;
            }
        }
    }
    preparation
}

fn completion(source: &str, reuse: bool) -> (Vec<Atom>, u64) {
    let Preparation {
        catalog,
        accounting,
        program,
        mut budget,
        limits,
        location,
        ..
    } = prepare(source, reuse);
    let mut counters = Counters::resume(accounting, crate::grounding_observer::Work::default());
    let start = counters.accounting.work;
    let catalog = build(
        catalog,
        &program,
        None,
        &limits,
        &mut budget,
        &mut counters,
        location,
    )
    .unwrap();
    let work = counters.accounting.work - start;
    let snapshot = catalog.snapshot(&limits, &mut counters, location).unwrap();
    let mut atoms = Vec::new();
    for (_, source) in snapshot.relations.source_atoms() {
        atoms.extend(
            source
                .iter()
                .map(|atom| atom.to_atom(zetesis_core::ValueLimits::default()).unwrap()),
        );
    }
    atoms.sort();
    (atoms, work)
}

fn outputs(atoms: &[Atom], predicate: &str) -> Vec<Vec<i32>> {
    let mut result: Vec<_> = atoms
        .iter()
        .filter(|atom| atom.predicate().name() == predicate)
        .map(|atom| {
            atom.values()
                .iter()
                .map(|value| {
                    let zetesis_core::Value::Number(value) = value else {
                        panic!("numeric output fixture")
                    };
                    *value
                })
                .collect()
        })
        .collect();
    result.sort();
    result
}

fn same_formula(source: &str) -> crate::formula::Compiled {
    // Hold formula factorization fixed while the reference changes only support
    // continuation reuse. Both routes still emit every original witness here.
    let ((shared, reference), _) = crate::formula_factor::testing::scoped(false, || {
        (
            crate::formula_ground::ground(prepare(source, true), None, None).unwrap(),
            crate::formula_ground::ground(prepare(source, false), None, None).unwrap(),
        )
    });
    assert_eq!(shared.atoms.atoms(), reference.atoms.atoms(), "{source}");
    // Identity of the complete graph, roots and origins is stronger here than
    // sampling interpretations: the support-only optimization must not elide
    // any original positive witness or its frozen-reduct formula.
    assert_eq!(
        (shared.theory.nodes(), shared.theory.operands()),
        (reference.theory.nodes(), reference.theory.operands()),
        "{source}"
    );
    assert_eq!(shared.theory.roots(), reference.theory.roots(), "{source}");
    assert_eq!(shared.origins, reference.origins, "{source}");
    assert_eq!(shared.warnings, reference.warnings, "{source}");
    shared
}

#[test]
fn anonymous_witnesses_share_support_continuations() {
    let source = "d(1,a).d(1,b).d(2,a). {p}. q(X,N) :- d(X,_), N=#sum{X:p}.";
    let (shared, work) = completion(source, true);
    let (reference, reference_work) = completion(source, false);
    assert_eq!(shared, reference);
    assert_eq!(
        outputs(&shared, "q"),
        [vec![1, 0], vec![1, 1], vec![2, 0], vec![2, 2]]
    );
    assert!(work < reference_work);
    same_formula(source);
}

#[test]
fn changed_continuation_inputs_refresh_support() {
    for source in [
        // The head reads Y even though the aggregate does not.
        "d(1,a).d(1,b). {p}. q(Y,N) :- d(X,Y), N=#count{1:p}.",
        // A head scalar consumes a relational input outside the aggregate.
        "d(1,4,a).d(1,4,b).d(1,5,a). {p}. q(X,N+Z) :- d(X,Z,_), N=#count{1:p}.",
        // A body check reads Y after a proposal; false and true rows differ.
        "d(1,0,a).d(1,0,b).d(1,1,a). {p}. q(N) :- d(X,Y,_), N=#count{1:p}, N=Y.",
        // A nonbinding aggregate bound remains an outer read.
        "d(1,0,a).d(1,0,b).d(1,1,a). {p}. q(X,N) :- d(X,Y,_), N=#count{1:p}, #count{1:p}!=Y.",
        // Tuple-only and nested/local condition reads preserve correlation.
        "d(0,a).d(0,b).d(1,a).r(0).r(1). {p}. q(N) :- d(X,_), N=#count{X:p;Z:r(Z),Z=X}.",
        // Negative truth is retained for every original witness.
        "d(1,a).d(1,b).d(2,a). {p;r(1)}. q(N) :- d(X,_), N=#count{1:p}, not r(X).",
        "d(1,a).d(1,b).d(2,a). {p;r(1,1)}. q(N) :- d(X,_), N=#count{1:p}, not r(X,_).",
    ] {
        assert_eq!(
            completion(source, true).0,
            completion(source, false).0,
            "{source}"
        );
        same_formula(source);
    }
}

fn same_diagnostic(shared: FormulaFailure, reference: FormulaFailure) {
    let (
        FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation {
            error: left,
            location: left_site,
        }),
        FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation {
            error: right,
            location: right_site,
        }),
    ) = (shared, reference)
    else {
        panic!("expected typed source arithmetic failures")
    };
    assert_eq!(left, right);
    assert_eq!(left_site, right_site);
}

#[test]
fn support_continuations_preserve_arithmetic_evidence() {
    for source in [
        // A repeated failing continuation precedes a defined continuation.
        "d(1,a).d(1,b).d(2,a). {p}. q(X,C,N) :- d(X,_), C=1/(X-1), N=#count{1:p}.",
        // Undefined aggregate element arithmetic remains a family diagnostic.
        "d(1,a).d(1,b).d(2,a). {p}. q(X,N) :- d(X,_), N=#count{1:p,1/(X-1)>0}.",
        // A scalar false row excludes its independent arithmetic error.
        "d(1,a).d(1,b).d(2,a). {p}. q(X,C,N) :- d(X,_), C=1/(X-1), N=#count{1:p}, X!=1.",
    ] {
        let shared = crate::formula_ground::ground(prepare(source, true), None, None);
        let reference = crate::formula_ground::ground(prepare(source, false), None, None);
        match (shared, reference) {
            (Ok(shared), Ok(reference)) => {
                assert_eq!(shared.atoms.atoms(), reference.atoms.atoms());
                assert_eq!(
                    (shared.theory.nodes(), shared.theory.operands()),
                    (reference.theory.nodes(), reference.theory.operands())
                );
                assert_eq!(shared.theory.roots(), reference.theory.roots());
                assert_eq!(shared.origins, reference.origins);
                assert_eq!(shared.warnings, reference.warnings);
            }
            (Err(shared), Err(reference)) => same_diagnostic(shared, reference),
            (shared, reference) => {
                panic!("diagnostic mismatch for {source}: {shared:?}, {reference:?}")
            }
        }
    }
}

#[test]
fn failed_continuations_keep_their_diagnostic() {
    let source = "d(1,a).d(1,b). {p}. q(C,N) :- d(X,_), C=1/(X-1), N=#count{1:p}.";
    let shared = crate::formula_ground::ground(prepare(source, true), None, None).unwrap_err();
    let reference = crate::formula_ground::ground(prepare(source, false), None, None).unwrap_err();
    same_diagnostic(shared, reference);
}

#[test]
fn empty_continuations_publish_no_support() {
    let source = "d(a).d(b). {p}. q(N,R) :- d(_), N=#count{1:p}, R=2..1.";
    let (shared, _) = completion(source, true);
    assert_eq!(shared, completion(source, false).0);
    assert!(outputs(&shared, "q").is_empty());
    same_formula(source);
}

#[test]
fn new_support_rounds_refresh_continuations() {
    let source = "d(a).d(b).p(1).p(2):-n(1). n(N):-d(_),N=#count{X:p(X)}.";
    let (shared, _) = completion(source, true);
    assert_eq!(shared, completion(source, false).0);
    assert_eq!(outputs(&shared, "n"), [vec![0], vec![1], vec![2]]);
    same_formula(source);
}

#[test]
fn rich_heads_keep_complete_support_traversal() {
    let preparation = prepare("d(a).d(b). {p}. q(N);r(N) :- d(_),N=#count{1:p}.", true);
    let plan = preparation
        .program
        .rules
        .iter()
        .find_map(|rule| rule.bindings.as_ref())
        .unwrap();
    assert!(plan.continuation_inputs.is_none());
    same_formula("d(a).d(b). {p}. q(N);r(N) :- d(_),N=#count{1:p}.");
}
