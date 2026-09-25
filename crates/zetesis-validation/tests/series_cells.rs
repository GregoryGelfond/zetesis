//! The audit series measures a fixed, distinct, capture-bounded cell set.
use std::collections::BTreeSet;
use std::path::Path;
use zetesis_validation::{
    examples,
    performance::{
        matrix::{Workload, WorkloadLimits},
        series,
    },
};

fn corpus() -> examples::Corpus {
    examples::load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/correctness"),
        examples::Limits::default(),
    )
    .unwrap()
}

#[test]
fn the_series_names_its_cells_in_a_fixed_order() {
    let workloads = series::workloads(&corpus(), WorkloadLimits::default()).unwrap();
    let entries: Vec<_> = workloads.iter().map(Workload::entry).collect();
    assert_eq!(
        entries,
        [
            "generated/independent-choice-12.lp",
            "generated/independent-choice-16.lp",
            "generated/independent-negation-8.lp",
            "generated/independent-negation-10.lp",
            "generated/independent-negation-aggregate-16.lp",
            "generated/disjunction-12.lp",
            "generated/ties-50.lp",
            "generated/transitive-path-100.lp",
            "generated/transitive-path-200.lp",
            "generated/transitive-dense-40.lp",
            "generated/chain-1000.lp",
            "generated/chain-2000.lp",
            "generated/chain-arithmetic-1000.lp",
            "generated/stratified-16.lp",
            "generated/producer-chain-700.lp",
            "generated/latin-square-5.lp",
            "generated/planning-14.lp",
            "standalone/n-queens/variant-01.lp",
            "standalone/n-queens/variant-01.lp",
            "standalone/n-queens/variant-04.lp",
            "standalone/send-money/send-money.lp",
            "scenarios/task-allocation/variant-04/05-larger-mix.lp",
        ]
    );
    assert_eq!(series::CELLS, workloads.len());
}

#[test]
fn series_cells_have_distinct_identities_and_the_expected_provenance() {
    let workloads = series::workloads(&corpus(), WorkloadLimits::default()).unwrap();
    let identities: BTreeSet<_> = workloads.iter().map(Workload::identity).collect();
    assert_eq!(identities.len(), workloads.len());
    let generated = workloads.iter().filter(|w| w.is_generated()).count();
    let amended = workloads.iter().filter(|w| w.is_amended()).count();
    assert_eq!((generated, amended), (17, 3));
    // Unchanged corpus entries and generated programs carry a contract; an
    // amended queens board is established by its reference family alone.
    assert!(
        workloads
            .iter()
            .all(|w| w.contract().is_some() != w.is_amended())
    );
    let queens: Vec<_> = workloads
        .iter()
        .filter(|w| w.is_amended())
        .map(|w| serde_json::to_value(w).unwrap()["sources"][0]["edits"][0]["after"].clone())
        .collect();
    assert_eq!(queens, ["10", "11", "11"]);
}

#[test]
fn series_limits_raise_only_the_ceilings_the_cells_need() {
    use zetesis_validation::{answers::native_json, performance::Limits};
    let base = Limits::default();
    let limits = series::limits(base);
    assert_eq!(limits.process.max_output_bytes, series::CAPTURE_BYTES);
    assert!(limits.max_total_capture_bytes >= 40 * series::CAPTURE_BYTES);
    assert!(limits.max_report_bytes >= limits.max_total_capture_bytes);
    assert_eq!(limits.process.timeout, base.process.timeout);
    let answers = series::native_answers(native_json::Limits::default());
    assert_eq!(answers.report.max_input_bytes, series::CAPTURE_BYTES);
    // A caller's larger ceiling is kept.
    let mut wide = Limits::default();
    wide.process.max_output_bytes = 64 * 1024 * 1024;
    assert_eq!(
        series::limits(wide).process.max_output_bytes,
        64 * 1024 * 1024
    );
}

#[test]
fn series_sources_fit_a_small_source_ceiling() {
    // The library enforces the source ceiling; that every cell's complete
    // native output stays under the capture ceiling is a property the sizes
    // were measured for, recorded in the observations, not one it checks.
    let limits = WorkloadLimits {
        source_bytes: 65_536,
        ..WorkloadLimits::default()
    };
    assert!(series::workloads(&corpus(), limits).is_ok());
}
