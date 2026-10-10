use super::*;
use crate::formula_count_plan::{CountPlanLimits, CountPlanResource, CountPlanStatistics};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::AdmissionLimits;

fn work() -> Work {
    Work {
        limits: CountPlanLimits::default(),
        cancellation: Cancellation::default(),
        statistics: CountPlanStatistics::default(),
        location: ProgramSite::program(),
    }
}

fn original() -> Theory {
    let mut nodes = FormulaNodes::default();
    let mut transaction = nodes.transaction();
    transaction.push(NodeView::Atom(0), 100, 100).unwrap();
    transaction.push(NodeView::Atom(1), 100, 100).unwrap();
    transaction
        .push_threshold_family(&[0, 0, 1], &[2, 3, 4], &[2, 5, 7], 100, 100)
        .unwrap();
    transaction.commit();
    Theory::new(2, nodes.into_parts(), vec![], AdmissionLimits::default()).unwrap()
}

#[test]
fn prefix_copy_charges_shared_rows_once() {
    let original = original();
    let mut work = work();
    let copied = prefix::copy(&original, 4, &mut work).unwrap();
    assert_eq!(copied.parts().rows().len(), 1);
    assert_eq!(copied.parts().thresholds().len(), 2);
    assert_eq!(copied.parts().conditions(), &[0, 0, 1]);
    for index in 0..4 {
        assert_eq!(copied.view().node(index), original.view().node(index));
    }
    let source = original.parts();
    let maps = source.nodes().len() * size_of::<Option<usize>>()
        + (source.rows().len() + source.thresholds().len())
            * (size_of::<bool>() + size_of::<Option<usize>>());
    assert_eq!(
        u128::from(work.statistics.storage_bytes),
        copied.parts().payload_bytes() + maps as u128
    );
}

#[test]
fn prefix_copy_refuses_unfunded_numeric_payload() {
    let original = original();
    let mut measured = work();
    prefix::copy(&original, 4, &mut measured).unwrap();
    let mut bounded = work();
    bounded.limits.max_bytes = measured.statistics.storage_bytes - 1;
    assert_eq!(
        prefix::copy(&original, 4, &mut bounded).unwrap_err(),
        Fault::Limit(CountPlanResource::Bytes)
    );
}

#[test]
fn count_payload_is_independent_of_numeric_bound() {
    let mut amounts = Vec::new();
    for bound in [2, 50] {
        let mut nodes = FormulaNodes::default();
        let mut work = work();
        work.limits.max_bytes = 10_000;
        let consequence = Consequence {
            location: ProgramSite::program(),
            body: 0,
            members: (0..100).collect(),
            lower: bound,
            origins: vec![],
        };
        let root = lower(&mut nodes, &consequence, &mut work).unwrap();
        let NodeView::Threshold(threshold) = nodes.view().node(root).unwrap() else {
            panic!("interior count bound must use the native row");
        };
        assert_eq!(threshold.conditions.len(), 100);
        assert_eq!(threshold.bound, bound as i128);
        amounts.push(work.statistics.storage_bytes);
    }
    assert_eq!(amounts[0], amounts[1]);
}
