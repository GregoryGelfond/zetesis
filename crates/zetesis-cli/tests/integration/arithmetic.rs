//! Original-answer arithmetic preserves complete families and publishes its work.

mod root_facts;

use std::collections::BTreeSet;

use serde_json::{Value as Json, json};
use zetesis_cli::{Completion, Grounder, Oracle, Report};
use zetesis_stable::{CardinalityStatistics, ParityStatistics, PreparationStatus, PreparationWork};
use zetesis_test_support::document::spelled;

use crate::support::{options::serial, runs::detailed};

const GROUNDERS: [&[&str]; 3] = [&[], &["--grounder", "eager"], &["--grounder", "lazy"]];

fn solve(source: &str, arguments: &[&str]) -> (Report, Json) {
    let extra: Vec<_> = ["--json", "--stats"]
        .into_iter()
        .chain(arguments.iter().copied())
        .collect();
    let options = serial(&extra);
    assert_eq!(options.oracle, Oracle::Auto);
    if arguments.is_empty() {
        assert_eq!(options.grounder, Grounder::Auto);
    }
    let (report, output, _) = detailed(source, &options);
    let document = serde_json::from_str(&output).unwrap();
    (report, document)
}

fn complete(report: &Report, document: &Json, models: usize) {
    assert_eq!(report.completion, Completion::Exhausted);
    assert!(report.interruption.is_none());
    assert_eq!(report.models, models);
    assert_eq!(document["models"].as_array().unwrap().len(), models);
    let outcome = &document["outcome"];
    assert_eq!(outcome["completion"], "exhausted");
    assert_eq!(outcome["coverage"], "exhausted");
    assert!(outcome["interruption"].is_null());
    assert_eq!(outcome["published_models"], models);
    assert_eq!(outcome["verified_models"], models);
    assert_eq!(
        outcome["status"],
        if models == 0 {
            "unsatisfiable"
        } else {
            "satisfiable"
        }
    );
}

fn phases(document: &Json, work: PreparationWork, total: u64) {
    // Each phase is disjoint logical work, not elapsed time. Check the public
    // field correspondence as well as the independent sum of all seven fields.
    let expected = json!({
        "recognition": work.recognition,
        "provenance": work.provenance,
        "reduction": work.reduction,
        "discovery": work.discovery,
        "candidate_checks": work.candidate_checks,
        "index": work.index,
        "root_seed": work.root_seed,
    });
    assert_eq!(document["phases"], expected);
    let sum: u64 = document["phases"]
        .as_object()
        .unwrap()
        .values()
        .map(|value| value.as_u64().unwrap())
        .sum();
    assert_eq!(sum, total);
    assert_eq!(work.total(), total);
    assert_eq!(document["preparation_work"], total);
}

fn receipts(document: &Json, parity: ParityStatistics, cardinality: CardinalityStatistics) {
    let search = &document["statistics"]["search"];
    let published = &search["parity"];
    assert_eq!(published["status"], parity.status.label());
    assert_eq!(published["rows"], parity.rows);
    assert_eq!(published["source_roots"], parity.source_roots);
    assert_eq!(published["rank"], parity.rank);
    assert_eq!(published["units"], parity.units);
    assert_eq!(published["region_passes"], parity.region_passes);
    assert_eq!(published["forced"], parity.forced);
    assert_eq!(published["refuted"], parity.refuted);
    assert_eq!(published["root_refuted"], parity.root_refuted);
    assert_eq!(published["provenance_rejected"], parity.provenance_rejected);
    phases(published, parity.phases, parity.preparation_work);
    let published = &search["cardinality"];
    assert_eq!(published["status"], cardinality.status.label());
    assert_eq!(published["rows"], cardinality.rows);
    assert_eq!(published["lower"], cardinality.lower);
    assert_eq!(published["upper"], cardinality.upper);
    assert_eq!(published["guarded"], cardinality.guarded);
    assert_eq!(published["region_passes"], cardinality.region_passes);
    assert_eq!(published["refuted"], cardinality.refuted);
    assert_eq!(published["root_refuted"], cardinality.root_refuted);
    assert_eq!(
        published["provenance_rejected"],
        cardinality.provenance_rejected
    );
    phases(published, cardinality.phases, cardinality.preparation_work);
}

fn family(document: &Json) -> BTreeSet<BTreeSet<String>> {
    document["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| {
            spelled(document, record)
                .into_iter()
                .map(|atom| {
                    assert_eq!(atom["sign"], "positive");
                    assert_eq!(atom["arguments"], json!([]));
                    atom["predicate"].as_str().unwrap().to_owned()
                })
                .collect()
        })
        .collect()
}

#[test]
fn odd_parity_refutes_before_candidate_enumeration() {
    // Each edge requires opposite truth values. The three equations are
    // individually satisfiable, but their XOR is 0 = 1. No root unit suffices.
    let source = "{a;b;c}.
        :- a,b. :- not a,not b.
        :- b,c. :- not b,not c.
        :- c,a. :- not c,not a.";
    for arguments in GROUNDERS {
        let (report, document) = solve(source, arguments);
        complete(&report, &document, 0);
        assert_eq!(report.checked, 0);
        let statistics = report.countermodel_statistics.unwrap();
        let regions = statistics.regions.unwrap();
        assert_eq!(regions.parity.status, PreparationStatus::Prepared);
        assert_eq!(regions.parity.rows, 3);
        assert_eq!(regions.parity.source_roots, 6);
        assert!(regions.parity.root_refuted);
        assert!(regions.parity.preparation_work > 0);
        assert!(regions.parity.phases.provenance > 0);
        assert!(regions.parity.phases.reduction > 0);
        assert_eq!(regions.cardinality.status, PreparationStatus::NotPrepared);
        assert_eq!(regions.counts.regions, 0);
        assert_eq!(regions.parity.region_passes, 0);
        assert_eq!(regions.parity.forced, 0);
        assert_eq!(regions.parity.refuted, 0);
        assert_eq!(regions.cardinality.region_passes, 0);
        assert_eq!(regions.cardinality.refuted, 0);
        assert_eq!(statistics.candidates, 0);
        assert_eq!(statistics.countermodel_queries, 0);
        let search = &document["statistics"]["search"];
        assert_eq!(search["candidate_regions"]["visited"], 0);
        assert_eq!(search["candidates"], 0);
        assert_eq!(search["countermodel_queries"], 0);
        receipts(&document, regions.parity, regions.cardinality);
        if arguments == ["--grounder", "lazy"] {
            // Positive pairs are materialized during capture. The other three
            // constraints enter through the retained share; its receipt counts
            // only that second path. All six are checked as parity sources above.
            let hybrid = report.hybrid_execution.unwrap();
            assert_eq!(hybrid.streamed.retained_templates, 3);
            assert_eq!(hybrid.streamed.retained_instances, 3);
            assert_eq!(hybrid.streamed.templates, 0);
            assert_eq!(hybrid.streamed.instances, 0);
            assert_eq!(search["scope"], "retained_core");
        } else {
            assert!(report.hybrid_execution.is_none());
            assert_eq!(search["scope"], "original_theory");
        }
    }
}

#[test]
fn even_parity_preserves_both_complete_answers() {
    let source = "{a;b;c;d}.
        :- a,b. :- not a,not b.
        :- b,c. :- not b,not c.
        :- c,d. :- not c,not d.
        :- d,a. :- not d,not a.";
    let expected: BTreeSet<BTreeSet<String>> = [["a", "c"], ["b", "d"]]
        .into_iter()
        .map(|atoms| atoms.into_iter().map(str::to_owned).collect())
        .collect();
    for arguments in GROUNDERS {
        let (report, document) = solve(source, arguments);
        complete(&report, &document, 2);
        assert_eq!(family(&document), expected);
        let regions = report.countermodel_statistics.unwrap().regions.unwrap();
        assert_eq!(regions.parity.status, PreparationStatus::Prepared);
        assert_eq!(regions.parity.rows, 4);
        assert_eq!(regions.parity.rank, 3);
        assert!(!regions.parity.root_refuted);
        assert!(!regions.cardinality.root_refuted);
        receipts(&document, regions.parity, regions.cardinality);
    }
}

#[test]
fn a_guarded_count_preserves_inactive_guard_answers() {
    let source = "{a;b;c;g}. :- g, #count {1:a;2:b;3:c} >= 2.";
    // All eight selections are valid when g is false. With g true, only the
    // empty selection and the three singletons remain: twelve full answers.
    let expected: BTreeSet<BTreeSet<String>> = (0_u8..16)
        .filter(|&mask| mask & 8 == 0 || (mask & 7).count_ones() <= 1)
        .map(|mask| {
            ["a", "b", "c", "g"]
                .into_iter()
                .enumerate()
                .filter(|&(index, _)| mask & (1 << index) != 0)
                .map(|(_, atom)| atom.to_owned())
                .collect()
        })
        .collect();
    assert_eq!(expected.len(), 12);
    for arguments in GROUNDERS {
        let (report, document) = solve(source, arguments);
        complete(&report, &document, 12);
        assert_eq!(family(&document), expected);
        let regions = report.countermodel_statistics.unwrap().regions.unwrap();
        assert_eq!(regions.cardinality.status, PreparationStatus::Prepared);
        assert!(regions.cardinality.guarded > 0);
        assert!(regions.cardinality.upper > 0);
        assert!(regions.cardinality.phases.provenance > 0);
        assert!(!regions.parity.root_refuted);
        assert!(!regions.cardinality.root_refuted);
        receipts(&document, regions.parity, regions.cardinality);
    }
}

#[test]
fn lowered_pigeonhole_sources_refute_before_search() {
    let examples = zetesis_test_support::repository::examples();
    for (path, lower, upper) in [
        ("scalability/pigeonhole-clauses/pigeonhole_clauses.lp", 5, 4),
        ("scalability/pigeonhole.lp", 8, 15),
    ] {
        let source = std::fs::read_to_string(examples.join(path)).unwrap();
        for arguments in GROUNDERS {
            let (report, document) = solve(&source, arguments);
            complete(&report, &document, 0);
            let statistics = report.countermodel_statistics.unwrap();
            let regions = statistics.regions.unwrap();
            assert_eq!(regions.cardinality.status, PreparationStatus::Prepared);
            assert_eq!(regions.cardinality.lower, lower, "{path}, {arguments:?}");
            assert_eq!(regions.cardinality.upper, upper, "{path}, {arguments:?}");
            assert!(regions.cardinality.root_refuted, "{path}, {arguments:?}");
            assert!(regions.cardinality.phases.provenance > 0);
            assert!(regions.cardinality.phases.candidate_checks > 0);
            assert_eq!(regions.counts.regions, 0);
            assert_eq!(regions.parity.region_passes, 0);
            assert_eq!(regions.parity.forced, 0);
            assert_eq!(regions.parity.refuted, 0);
            assert_eq!(regions.cardinality.region_passes, 0);
            assert_eq!(regions.cardinality.refuted, 0);
            assert_eq!(statistics.candidates, 0);
            assert_eq!(statistics.countermodel_queries, 0);
            receipts(&document, regions.parity, regions.cardinality);
        }
    }
}
