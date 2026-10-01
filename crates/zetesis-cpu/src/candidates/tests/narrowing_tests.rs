//! The narrowing keeps its closures' preparation and workspace with the
//! iterator, so that every closure after the first runs on what the first
//! prepared and reserved.

use super::*;
use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Template, Term, Value};

/// node(1..n). in(X) :- node(X), not out(X). out(X) :- node(X), not in(X).
fn independent(nodes: i32) -> Program {
    let pattern = |name: &str, terms: Vec<Term>| {
        AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms).unwrap()
    };
    let mut templates: Vec<Template> = (1..=nodes)
        .map(|node| {
            Template::new(
                Some(pattern("node", vec![Term::Constant(Value::Number(node))])),
                vec![],
                vec![],
                vec![],
                vec![],
            )
        })
        .collect();
    let x = Term::Variable(0);
    for (head, gate) in [("in", "out"), ("out", "in")] {
        templates.push(Template::new(
            Some(pattern(head, vec![x.clone()])),
            vec![pattern("node", vec![x.clone()])],
            vec![],
            vec![pattern(gate, vec![x.clone()])],
            vec![],
        ));
    }
    Program::new(templates, AdmissionLimits::default()).unwrap()
}

#[test]
fn the_narrowing_keeps_the_capacity_its_closures_reserved() {
    // Fifteen regions, each narrowed by two closures on the one workspace,
    // which retains the capacity the closures reserved instead of freeing it
    // after each.
    let program = independent(3);
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    );
    candidates.bounded(Limits::default());
    assert_eq!(candidates.by_ref().map(Result::unwrap).count(), 8);
    assert_eq!(candidates.statistics().regions, 15);
    let NarrowingState::Applied(closures) = &candidates.narrowing else {
        panic!("the bounds are applied");
    };
    let empty = ClosureWorkspace::default().retained_bytes().unwrap();
    assert!(closures.workspace.retained_bytes().unwrap() > empty);
}

#[test]
fn a_stopped_preparation_is_the_narrowings_stop() {
    // No unit of work prepares a program with gate atoms: the narrowing
    // reports the preparation's stop and offers the whole symbolic carrier.
    let program = independent(2);
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    );
    candidates.bounded(Limits {
        max_work: 0,
        ..Limits::default()
    });
    assert_eq!(candidates.by_ref().map(Result::unwrap).count(), 16);
    assert_eq!(
        candidates.statistics().narrowing_stop,
        Some(Stop::WorkLimit)
    );
    assert!(matches!(candidates.narrowing, NarrowingState::Unavailable));
    assert!(candidates.prepared_queries().is_none());
}

/// The fixture's default allowance is a checked finite upper bound, not a
/// guessed unit count. Every probe reconstructs the same workspace history.
fn least_work(mut completes: impl FnMut(u64) -> Result<(), Stop>) -> u64 {
    let mut upper = Limits::default().max_work;
    completes(upper).expect("the finite fixture completes within default work");
    let mut lower = 0;
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        match completes(middle) {
            Ok(()) => upper = middle,
            Err(Stop::WorkLimit) => lower = middle + 1,
            Err(other) => panic!("unexpected phase refusal: {other:?}"),
        }
    }
    lower
}

/// Measure root transfer with both completed closures retained by the caller.
fn root_transfer_work(
    program: &Program,
    prepared: &PreparedQueries,
    cube: &Cube,
    cancellation: &Cancellation,
) -> u64 {
    let mut workspace = ClosureWorkspace::default();
    let lower = definite_closure(
        prepared,
        &mut workspace,
        cube.into(),
        Limits::default(),
        cancellation,
    )
    .unwrap();
    let upper = possible_closure(
        prepared,
        &mut workspace,
        cube.into(),
        Limits::default(),
        cancellation,
    )
    .unwrap();
    assert!(!lower.constraint_violated && !upper.constraint_violated);
    let mut transferred = Cube::all_open();
    let mut transfer = Work::source(cancellation, Limits::default().max_work);
    assert_eq!(
        transfer_root(
            program,
            &mut transferred,
            &lower.atoms,
            &upper.atoms,
            Limits::default().max_closure_bytes,
            &mut 0,
            &mut transfer
        ),
        Ok(Pass::Changed)
    );
    transfer.source_statistics(0).work
}

#[test]
fn narrowing_gives_each_phase_an_independent_work_allowance() {
    let program = independent(2);
    let cancellation = Cancellation::default();
    let prepared =
        PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
    let cube = Cube::all_open();
    let lower_work = least_work(|max_work| {
        definite_closure(
            &prepared,
            &mut ClosureWorkspace::default(),
            (&cube).into(),
            Limits {
                max_work,
                ..Limits::default()
            },
            &cancellation,
        )
        .map(|_| ())
    });
    let upper_work = least_work(|max_work| {
        let mut workspace = ClosureWorkspace::default();
        // Keep the lower Model live exactly as Closures::enclose does; the upper
        // reuses that successful closure's retained identity and buffer capacity.
        let lower = definite_closure(
            &prepared,
            &mut workspace,
            (&cube).into(),
            Limits::default(),
            &cancellation,
        )
        .unwrap();
        assert!(!lower.constraint_violated);
        possible_closure(
            &prepared,
            &mut workspace,
            (&cube).into(),
            Limits {
                max_work,
                ..Limits::default()
            },
            &cancellation,
        )
        .map(|_| ())
    });
    let transfer_work = root_transfer_work(&program, &prepared, &cube, &cancellation);
    let preparation_work = prepared.statistics().work;
    let maximum = [preparation_work, lower_work, upper_work, transfer_work]
        .into_iter()
        .max()
        .unwrap();
    assert!(lower_work > 0 && upper_work > 0 && transfer_work > 0 && preparation_work > 0);
    assert!(
        preparation_work + lower_work + upper_work + transfer_work > maximum,
        "a pooled allowance would not establish independent phase budgets"
    );

    let limits = Limits {
        max_work: maximum,
        ..Limits::default()
    };
    let mut closures = Closures::new(&program, limits, cancellation.clone()).unwrap();
    let mut actual = Cube::all_open();
    assert_eq!(closures.narrow(&mut actual), Ok(Pass::Changed));
    let expected: CarrierSet = program
        .indexed_gate_atoms()
        .map(|atom| atom.unwrap().carrier())
        .collect();
    assert!(actual.must.is_empty());
    assert_eq!(actual.may.as_ref(), Some(&expected));
    assert_eq!(closures.transfer_work, transfer_work);

    let mut refused = Cube::all_open();
    let result = Closures::new(
        &program,
        Limits {
            max_work: maximum - 1,
            ..limits
        },
        cancellation,
    )
    .and_then(|mut closures| closures.narrow(&mut refused));
    assert_eq!(result, Err(Stop::WorkLimit));
    assert!(refused.must.is_empty());
    assert!(refused.may.is_none());
}

#[test]
fn preparation_is_borrowed_only_after_narrowing() {
    let program = independent(2);
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    );
    candidates.bounded(Limits::default());
    assert!(candidates.prepared_queries().is_none());
    let first = candidates.next_selection().unwrap().unwrap();
    let prepared = Arc::clone(candidates.prepared_queries().unwrap());
    let NarrowingState::Applied(closures) = &candidates.narrowing else {
        panic!("completed narrowing retains its queries")
    };
    assert!(Arc::ptr_eq(&prepared, &closures.prepared));
    assert_eq!(candidates.by_ref().map(Result::unwrap).count(), 3);
    assert!(Arc::ptr_eq(
        candidates.prepared_queries().unwrap(),
        &prepared
    ));
    drop(candidates);
    let check = prepared
        .check_view(
            first.view(),
            &mut ClosureWorkspace::default(),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(check.accepted());
}

#[test]
fn an_empty_gate_carrier_retains_no_queries() {
    let program = Program::new(vec![], AdmissionLimits::default()).unwrap();
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    );
    candidates.bounded(Limits::default());
    assert!(candidates.next_selection().unwrap().is_ok());
    assert!(candidates.prepared_queries().is_none());
    assert!(candidates.next_selection().is_none());
}
