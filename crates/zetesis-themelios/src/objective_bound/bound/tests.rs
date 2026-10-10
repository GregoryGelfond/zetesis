use super::*;
use zetesis_ferraris::{ContributionRow, Node};

fn work(cancellation: &Cancellation) -> Work<'_> {
    Work {
        cancellation,
        limits: ObjectivePlanLimits::default(),
        template: None,
        statistics: ObjectiveBoundStatistics::default(),
    }
}

#[test]
fn eligibility_copy_retains_shared_numeric_rows() {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new(vec![Node::atom(0), Node::atom(1)], vec![]).unwrap());
    nodes
        .push_threshold_family(&[0, 0, 1], &[2, 3, 4], &[2, 5, 7], 100, 100)
        .unwrap();
    let source = nodes.into_parts();
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    let copied = copy(&source, &mut work).unwrap();
    assert_eq!(copied.parts().rows(), source.rows());
    assert_eq!(copied.parts().thresholds(), source.thresholds());
    assert_eq!(copied.parts().conditions(), source.conditions());
    assert_eq!(copied.parts().weights(), source.weights());
    assert_eq!(work.statistics.nodes, 5);
    // Two node passes, both numeric columns, one row and three descriptors.
    assert_eq!(work.statistics.work, 2 * 5 + 2 * 3 + 1 + 3);
}

#[test]
fn eligibility_copy_bounds_unused_numeric_rows() {
    let source = FormulaParts::from_raw(
        vec![],
        vec![],
        vec![],
        vec![],
        vec![
            ContributionRow {
                start: 0,
                length: 0
            };
            3
        ],
        vec![],
    )
    .unwrap();
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    work.limits.max_nodes = 2;
    assert_eq!(
        copy(&source, &mut work).unwrap_err().kind,
        Kind::Limit(ObjectiveBoundResource::Nodes)
    );
    assert_eq!(work.statistics.work, 0);
}

#[test]
fn numeric_append_charges_columns_and_descriptors() {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new(vec![Node::atom(0), Node::atom(1)], vec![]).unwrap());
    let cancellation = Cancellation::default();
    let mut measured = work(&cancellation);
    measured
        .node(
            &mut nodes,
            NodeView::Threshold(zetesis_ferraris::Threshold {
                conditions: &[0, 0, 1],
                weights: &[2, 3, 4],
                bound: 5,
            }),
        )
        .unwrap();
    // Node, child checks, both columns, row and threshold descriptor.
    assert_eq!(measured.statistics.work, 1 + 3 + 6 + 2);
}
