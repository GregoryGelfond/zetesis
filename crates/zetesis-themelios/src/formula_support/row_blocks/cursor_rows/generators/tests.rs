use super::*;
use crate::formula_support::{
    Computation, Context, GroundingWork, Ir, Join, SourceExecution, Support, build_with_executor,
    delta, testing, with_source_workers,
};
use crate::grounding_observer::Work;
use crate::{FormulaResource, GroundingWork as Observed};
use std::num::NonZeroUsize;
use zetesis_algebra::{Cancellation, Stop};
use zetesis_core::{Atom, ValueLimits};

const LOOKUPS: &str =
    "p(1;2;3).q(1,1,a).q(1,2,a).q(2,2,a).q(3,3,b).{r(X)}:-p(X),q(X,X,a).s(X):-p(X),q(X,X,a).";

struct Attempt {
    result: Result<Vec<Atom>, FormulaFailure>,
    tasks: usize,
    observed: Observed,
}

fn support(source: &str, workers: Option<usize>, bytes: usize) -> Attempt {
    let prepared = testing::prepare(source);
    let limits = FormulaLimits {
        max_support_bytes: bytes,
        ..prepared.limits
    };
    let mut budget = prepared.budget;
    let mut counters = Counters::resume(prepared.accounting, Work::worker(true));
    let action = || {
        let execution = SourceExecution::new();
        build_with_executor(
            prepared.catalog,
            &prepared.program,
            None,
            &mut budget,
            GroundingWork::new(&limits, &mut counters, prepared.location),
            execution.executor(),
        )
    };
    let (result, tasks) = if let Some(workers) = workers {
        with_source_workers(NonZeroUsize::new(workers).unwrap(), action)
    } else {
        (action(), 0)
    };
    let observed = counters.observed.snapshot();
    let result = result.map(|catalog| {
        let snapshot = catalog
            .snapshot(&limits, &mut counters, prepared.location)
            .unwrap();
        let mut atoms = Vec::new();
        for (_, rows) in snapshot.relations.source_atoms() {
            atoms.extend(
                rows.iter()
                    .map(|atom| atom.to_atom(ValueLimits::default()).unwrap()),
            );
        }
        atoms.sort();
        atoms
    });
    Attempt {
        result,
        tasks,
        observed,
    }
}

#[test]
fn one_generator_support_preserves_the_original_storage_contract() {
    let source = "d(1..100).p(X):-d(X).";
    let expected = support(source, None, 100_000).result.unwrap();
    assert_eq!(expected.len(), 200);
    for workers in [1, 14] {
        let actual = support(source, Some(workers), 100_000);
        assert_eq!(actual.tasks, 0);
        assert_eq!(actual.result.unwrap(), expected);
        assert_eq!(actual.observed.support_atoms, Some(200));
        assert_eq!(actual.observed.support_index_entries, Some(100));
    }
}

#[test]
fn bound_lookups_and_constant_prefixes_keep_serial_source_witnesses() {
    for source in [
        LOOKUPS,
        "ready.d(1..6).q(1..6).out(X):-ready,d(X),q(X).",
        "a.b.out:-a,b.",
        "d(1;2).p(X,X):-d(X),d(X).",
    ] {
        let expected = support(source, None, FormulaLimits::default().max_support_bytes)
            .result
            .unwrap();
        for workers in [1, 14] {
            let actual = support(
                source,
                Some(workers),
                FormulaLimits::default().max_support_bytes,
            );
            assert_eq!(actual.tasks, 0, "one generator: {source}");
            assert_eq!(actual.result.unwrap(), expected);
        }
    }
}

#[test]
fn expanding_join_generators_keep_actual_support_dispatch_and_complete_heads() {
    for source in [
        "d(1..4).e(1..3).out(X,Y):-d(X),e(Y).",
        "edge(1,2).edge(1,3).edge(2,4).edge(3,4).out(X,Z):-edge(X,Y),edge(Y,Z).",
        "gate(k).d(1..4).e(1..3).out(X,Y):-gate(k),d(X),e(Y).",
    ] {
        let expected = support(source, None, FormulaLimits::default().max_support_bytes)
            .result
            .unwrap();
        for workers in [1, 14] {
            let actual = support(
                source,
                Some(workers),
                FormulaLimits::default().max_support_bytes,
            );
            assert!(actual.tasks > 0, "expanding join: {source}");
            assert_eq!(actual.result.unwrap(), expected);
        }
    }
}

#[test]
fn structural_classification_preserves_cursor_owners_on_refusal_and_cancellation() {
    use crate::formula_support::row_blocks::tests::{JOIN, prepared_source};
    let (program, mut catalog) = prepared_source(JOIN);
    let rule = program.rules_without_facts().last().unwrap();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut budget = testing::budget();
    let (relations, mut append) = catalog
        .split(&limits, &mut counters, rule.location)
        .unwrap();
    let support = Support::indexed(&relations, &limits, &counters, rule.location).unwrap();
    let computation = Computation::new(&mut append, &support);
    let cursor = Join::variant_rule(
        Ir::Source(rule),
        delta::Variant::Full,
        &support,
        None,
        &mut budget,
        Context::new(&computation, &limits, &mut counters, rule.location),
    )
    .unwrap();
    let positions = cursor.positions.clone();
    let storage = computation.query_support().live_bytes();
    let before = counters.accounting.work;
    assert!(expands(&cursor.plan, &limits, &mut counters, rule.location).unwrap());
    let complete = counters.accounting.work - before;
    assert!(complete > 0);
    for allowance in 0..complete {
        let before = counters.accounting.work;
        let bounded = FormulaLimits {
            max_work: before + allowance,
            ..limits
        };
        assert!(matches!(
            expands(&cursor.plan, &bounded, &mut counters, rule.location),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ));
        assert_eq!(counters.accounting.work - before, allowance);
        assert_eq!(cursor.positions, positions);
        assert_eq!(computation.query_support().live_bytes(), storage);
    }
    let cancellation = Cancellation::default();
    cancellation.cancel();
    counters.attach_test_control(&cancellation);
    let before = counters.accounting.work;
    assert!(matches!(
        expands(&cursor.plan, &limits, &mut counters, rule.location),
        Err(FormulaFailure::Interrupted {
            reason: Stop::Cancelled,
            ..
        })
    ));
    assert_eq!(counters.accounting.work, before);
    assert_eq!(cursor.positions, positions);
    assert!(cursor.probes.iter().all(Option::is_none));
    assert_eq!(computation.query_support().live_bytes(), storage);
}
