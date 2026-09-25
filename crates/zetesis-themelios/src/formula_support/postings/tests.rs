//! Selectivity observations retain the original formula grounding route.

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use zetesis_core::{Atom, AtomPattern, Predicate, Term, Value, ValueLimits, ValueNode};

use super::{Limits, Report, Stop, begin_support, record};
use crate::formula_support::{Counters, SupportCatalog};
use crate::{
    AdmissionOptions, BundleAdmissionOptions, BundleLimits, ExpansionLimits, FormulaLimits,
    GroundingObserver, GroundingOutcome, GroundingPhase, GroundingWork, SourceBundle,
    admit_bundle_formula_with_grounding_observer, admit_formula,
};

fn relation(rows: Vec<Vec<Value>>) -> SupportCatalog {
    let mut support = SupportCatalog::default();
    let mut counters = Counters::default();
    for row in rows {
        let atom = Atom::new(Predicate::new("row", row.len()).unwrap(), row).unwrap();
        support = support
            .insert(atom, &FormulaLimits::default(), &mut counters, location())
            .unwrap();
    }
    support
}

fn location() -> Location {
    Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    }
}

fn probe(catalog: &SupportCatalog, values: &[Option<Value>]) -> (Option<Vec<usize>>, u64) {
    let support = catalog
        .snapshot(
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
    let pattern = AtomPattern::new(
        Predicate::new("row", values.len()).unwrap(),
        (0..values.len()).map(Term::Variable).collect(),
    )
    .unwrap();
    let mut counters = Counters::default();
    let rows = support
        .probe(
            &pattern,
            values,
            &FormulaLimits::default(),
            &mut counters,
            location(),
        )
        .unwrap();
    (rows.map(<[usize]>::to_vec), counters.work)
}

fn numbers(values: &[i32]) -> Vec<Value> {
    values.iter().map(|&value| Value::Number(value)).collect()
}

fn independent() -> SupportCatalog {
    relation(
        (0..4)
            .flat_map(|left| (0..4).map(move |right| numbers(&[left, right])))
            .collect(),
    )
}

#[test]
fn independent_columns_remove_surplus_rows() {
    let support = independent();
    let ((), report) = record(Limits::default(), || {
        begin_support();
        probe(&support, &[Some(Value::Number(2)), Some(Value::Number(3))]);
    });
    assert_eq!(report.stop, None);
    assert_eq!(report.totals.shortest_rows, 4);
    assert_eq!(report.totals.intersection_rows, 1);
    assert_eq!(report.totals.reduced_probes, 1);
}

#[test]
fn correlated_columns_have_no_selectivity_gain() {
    let support = relation(
        (0..16)
            .map(|row| numbers(&[row % 4, row % 4, row]))
            .collect(),
    );
    let ((), report) = record(Limits::default(), || {
        probe(
            &support,
            &[Some(Value::Number(2)), Some(Value::Number(2)), None],
        );
    });
    assert_eq!(report.stop, None);
    assert_eq!(report.totals.shortest_rows, 4);
    assert_eq!(report.totals.intersection_rows, 4);
    assert!(report.totals.posting_comparisons > 0);
}

#[test]
fn unavailable_bounds_leave_every_row_eligible() {
    let support = independent();
    let (_, report) = record(Limits::default(), || probe(&support, &[None, None]));
    assert_eq!(report.stop, None);
    assert_eq!(report.totals.intersection_rows, 16);
}

#[test]
fn missing_keys_produce_an_empty_intersection() {
    let support = independent();
    let (_, report) = record(Limits::default(), || {
        probe(&support, &[Some(Value::Number(7)), None])
    });
    assert_eq!(report.stop, None);
    assert_eq!(report.totals.intersection_rows, 0);
}

#[test]
fn exhaustive_small_queries_preserve_full_rows() {
    let support = independent();
    let ((), report) = record(Limits::default(), || {
        for left in [None, Some(-1), Some(0), Some(1), Some(2), Some(3), Some(4)] {
            for right in [None, Some(-1), Some(0), Some(1), Some(2), Some(3), Some(4)] {
                probe(
                    &support,
                    &[left.map(Value::Number), right.map(Value::Number)],
                );
            }
        }
    });
    assert_eq!(report.stop, None);
    assert_eq!(report.totals.probes, 49);
}

#[test]
fn sparse_postings_preserve_tuple_correlations() {
    let support = relation(
        [[0, 1, 2], [0, 3, 2], [1, 3, 2], [2, 0, 1], [2, 1, 0]]
            .iter()
            .map(|row| numbers(row))
            .collect(),
    );
    let ((), report) = record(Limits::default(), || {
        for left in [None, Some(0), Some(1), Some(2), Some(3)] {
            for middle in [None, Some(0), Some(1), Some(2), Some(3)] {
                for right in [None, Some(0), Some(1), Some(2), Some(3)] {
                    probe(
                        &support,
                        &[left, middle, right].map(|value| value.map(Value::Number)),
                    );
                }
            }
        }
    });
    assert_eq!(report.stop, None);
    assert_eq!(report.totals.probes, 125);
}

#[test]
fn structured_bounds_preserve_whole_value_identity() {
    let tuple = Value::from_nodes(
        vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(1)],
        ValueLimits::default(),
    )
    .unwrap();
    let support = relation(vec![
        vec![tuple.clone(), Value::Number(1)],
        vec![Value::Number(1), Value::Number(1)],
        vec![Value::String("(1,)".into()), Value::Number(1)],
        vec![tuple.clone(), Value::Number(2)],
    ]);
    let (_, report) = record(Limits::default(), || {
        probe(&support, &[Some(tuple), Some(Value::Number(1))])
    });
    assert_eq!(report.stop, None);
    assert_eq!(report.totals.shortest_rows, 2);
    assert_eq!(report.totals.intersection_rows, 1);
}

#[test]
fn count_overflow_retains_the_previous_total() {
    let mut total = usize::MAX;
    assert_eq!(super::add(&mut total, 1), Err(Stop::CountOverflow));
    assert_eq!(total, usize::MAX);
}

#[test]
fn repeated_queries_belong_to_one_support_build() {
    let support = independent();
    let ((), report) = record(Limits::default(), || {
        begin_support();
        probe(&support, &[None, None]);
        probe(&support, &[None, None]);
        begin_support();
        probe(&support, &[None, None]);
    });
    assert_eq!(report.totals.repeated_probes, 1);
    assert_eq!(report.keys, 2);
}

#[test]
fn diagnostic_limits_leave_the_probe_unchanged() {
    let support = independent();
    let values = [Some(Value::Number(1)), None];
    let plain = probe(&support, &values);
    for (limits, stop) in [
        (
            Limits {
                probes: 0,
                ..Limits::default()
            },
            Stop::Probes,
        ),
        (
            Limits {
                rows: 0,
                ..Limits::default()
            },
            Stop::Rows,
        ),
        (
            Limits {
                columns: 0,
                ..Limits::default()
            },
            Stop::Columns,
        ),
        (
            Limits {
                keys: 0,
                ..Limits::default()
            },
            Stop::Keys,
        ),
        (
            Limits {
                key_bytes: 0,
                ..Limits::default()
            },
            Stop::KeyBytes,
        ),
        (
            Limits {
                work: 0,
                ..Limits::default()
            },
            Stop::Work,
        ),
    ] {
        let (observed, report) = record(limits, || probe(&support, &values));
        assert_eq!(observed, plain);
        assert_eq!(report.stop, Some(stop));
        assert_eq!(report.totals.probes, 0);
    }
}

#[test]
fn unwind_clears_observation_identity() {
    assert!(
        catch_unwind(AssertUnwindSafe(|| record(Limits::default(), || {
            begin_support();
            panic!("controlled observation unwind");
        })))
        .is_err()
    );
    let ((), report) = record(Limits::default(), || {});
    assert_eq!(report.support_builds, 0);
}

#[test]
fn observation_preserves_the_compiled_subject() {
    for source in [
        "p(1,2). p(2,1). q(X) :- p(X,X).",
        "p(f(1),2). p(f(2),1). q(X) :- p(f(X),X).",
        "p(1,2). p(2,1). q(X) :- p(X,2), not r(X). {r(1)}.",
        "p(1,2). p(2,1). q(X) :- p(X,Y), X+Y=3.",
    ] {
        let compile = || {
            admit_formula(
                source.into(),
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                FormulaLimits::default(),
            )
            .unwrap()
        };
        let (observed, report) = record(Limits::default(), compile);
        let plain = compile();
        assert_eq!(report.stop, None);
        assert_eq!(report.support_builds, 1);
        assert!(report.totals.probes > 0);
        assert_eq!(observed.atoms(), plain.atoms());
        assert_eq!(observed.theory().nodes(), plain.theory().nodes());
        assert_eq!(observed.theory().roots(), plain.theory().roots());
        assert_eq!(observed.formula_origins(), plain.formula_origins());
        assert_eq!(
            observed.objectives().templates(),
            plain.objectives().templates()
        );
    }
}

#[derive(Default)]
struct Observer(Cell<GroundingWork>);

impl GroundingObserver for Observer {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        _phase: GroundingPhase,
        _location: Option<Location>,
        _outcome: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.0.set(self.0.get().checked_sum(work));
    }
}

fn report_json(
    path: &str,
    report: &Report,
    work: &GroundingWork,
    admission_completed: bool,
) -> serde_json::Value {
    let limits = Limits::default();
    serde_json::json!({
        "case": path,
        "route": "eager-formula-possible-support",
        "admission_completed": admission_completed,
        "complete": report.stop.is_none(),
        "stop": report.stop.map(|stop| format!("{stop:?}")),
        "support_builds": report.support_builds,
        "observed_existing_relation_probes": report.totals.probes,
        "bound_probes": report.totals.bound,
        "multiple_bound_columns": report.totals.multiple,
        "shortest_posting_rows": report.totals.shortest_rows,
        "intersection_rows": report.totals.intersection_rows,
        "reduced_probes": report.totals.reduced_probes,
        "repeated_snapshot_queries": report.totals.repeated_probes,
        "repeated_bound_snapshot_queries": report.totals.repeated_bound_probes,
        "posting_comparisons": report.totals.posting_comparisons,
        "diagnostic_work": report.work,
        "retained_query_keys": report.keys,
        "retained_key_payload_bytes": report.key_bytes,
        "actual_join_probes": work.join_probes,
        "actual_join_rows": work.join_rows,
        "actual_support_rounds": work.support_rounds,
        "actual_binding_snapshots": work.binding_snapshots,
        "limits": {
            "probes": limits.probes,
            "rows_per_relation": limits.rows,
            "columns": limits.columns,
            "retained_keys": limits.keys,
            "key_payload_bytes": limits.key_bytes,
            "work": limits.work,
        },
    })
}

#[test]
#[ignore = "explicit bounded corpus diagnostic; these observations are not timings"]
fn corpus_postings_preserve_full_row_equalities() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/correctness");
    let paths = [
        "standalone/n-queens/variant-01.lp",
        "standalone/n-queens/variant-02.lp",
        "standalone/n-queens/variant-03.lp",
        "standalone/n-queens/variant-04.lp",
        "standalone/n-queens/variant-05.lp",
        "standalone/n-queens/variant-06.lp",
        "standalone/send-money/send-money.lp",
        "scenarios/shortest-path/variant-01/07-cycles.lp",
        "scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp",
        "scenarios/shortest-path/variant-03/05-budget-unsat.lp",
        "scenarios/task-allocation/variant-01/05-larger-mix.lp",
        "scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp",
        "scenarios/task-allocation/variant-04/05-larger-mix.lp",
    ];
    for path in paths {
        let bundle = SourceBundle::load(root.join(path), BundleLimits::default()).unwrap();
        let observer = Observer::default();
        let (result, report) = record(Limits::default(), || {
            admit_bundle_formula_with_grounding_observer(
                bundle,
                BundleAdmissionOptions::default(),
                ExpansionLimits::default(),
                FormulaLimits::default(),
                Some(&observer),
            )
        });
        println!(
            "{}",
            report_json(path, &report, &observer.0.get(), result.is_ok())
        );
        result.unwrap_or_else(|error| panic!("{path}: {error}"));
        assert_eq!(report.stop, None, "{path}: incomplete diagnostic");
        assert_eq!(
            report.support_builds, 1,
            "one support builder owns row identity"
        );
    }
}
