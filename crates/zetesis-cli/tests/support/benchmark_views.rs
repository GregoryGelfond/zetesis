//! Human views retain the distinctions in typed workload identities.
use zetesis_validation::{examples, performance::matrix};

#[test]
fn amended_workloads_have_distinct_human_labels() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/kr-domains");
    let corpus = examples::load(&root, examples::Limits::default()).unwrap();
    let entry = "standalone/n-queens/variant-01.lp";
    let workloads: Vec<_> = [10, 11]
        .into_iter()
        .map(|replacement| {
            matrix::Workload::amended(
                &corpus,
                entry,
                &[matrix::ConstantAmendment {
                    source_path: entry,
                    name: "n",
                    expected: 8,
                    replacement,
                }],
                matrix::WorkloadLimits::default(),
            )
            .unwrap()
        })
        .collect();
    let cases = vec![entry.to_owned(); 2];
    let summary = matrix::Summary {
        schema: 1,
        format: "zetesis_benchmark_summary",
        passed: false,
        accounted: true,
        cases: &cases,
        workloads: Some(&workloads),
        profiles: &[],
        before: &[],
        cells: (0..2)
            .map(|case| matrix::CellSummary {
                case,
                producer: matrix::Producer::Reference,
                decisions: vec![matrix::DecisionCount {
                    decision: matrix::Decision::NotAttempted,
                    positions: 1,
                }],
                timing: None,
                peak_rss_bytes: None,
            })
            .collect(),
    };
    let mut output = Vec::new();
    super::corpus(
        &summary,
        false,
        zetesis_presentation::Layout::default(),
        &mut output,
    )
    .unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("1: n-queens/variant-01 8→10"), "{text}");
    assert!(text.contains("2: n-queens/variant-01 8→11"), "{text}");
}
