//! Distinct attempted and accepted parser counters keep their native failures.

use super::{Error, Issue, Limits, Resource, admit_spelling, record_attempt, values};

#[test]
fn occurrence_refusal_retains_the_representable_attempt() {
    for resource in [
        Resource::Atoms,
        Resource::ValueNodes,
        Resource::SpellingBytes,
    ] {
        let mut used = 2;
        record_attempt(&mut used, 1, 3, resource, "native count").unwrap();
        assert_eq!(used, 3);
        record_attempt(&mut used, 0, 3, resource, "native count").unwrap();
        assert!(matches!(
            record_attempt(&mut used, 2, 3, resource, "native count"),
            Err(Error::Limit { resource: refused, limit: 3, attempted: 5 }) if refused == resource
        ));
        assert_eq!(used, 5);
    }
}

#[test]
fn occurrence_overflow_leaves_the_original_prefix_and_detail() {
    let mut used = usize::MAX;
    assert!(matches!(
        record_attempt(&mut used, 1, 0, Resource::Atoms, "native atom count"),
        Err(Error::Invalid {
            issue: Issue::CountOverflow,
            detail: "native atom count"
        })
    ));
    assert_eq!(used, usize::MAX);
}

#[test]
fn spelling_admission_publishes_only_an_accepted_total() {
    let mut used = 2;
    admit_spelling(&mut used, 1, 3).unwrap();
    admit_spelling(&mut used, 0, 3).unwrap();
    assert!(matches!(
        admit_spelling(&mut used, 1, 3),
        Err(Error::Limit {
            resource: Resource::SpellingBytes,
            limit: 3,
            attempted: 4
        })
    ));
    assert_eq!(used, 3);
    used = usize::MAX;
    assert!(matches!(
        admit_spelling(&mut used, 1, 0),
        Err(Error::Invalid {
            issue: Issue::CountOverflow,
            detail: "display spelling bytes"
        })
    ));
    assert_eq!(used, usize::MAX);
}

#[test]
fn node_count_refusal_precedes_value_shape_decoding() {
    let raw = serde_json::json!([{"kind":"unrecognized"}]);
    let mut used = 1;
    let limits = Limits {
        max_value_nodes: 1,
        ..Limits::default()
    };
    assert!(matches!(
        values::value(&raw, limits, &mut used),
        Err(Error::Limit {
            resource: Resource::ValueNodes,
            limit: 1,
            attempted: 2
        })
    ));
    assert_eq!(used, 2);
    used = usize::MAX;
    assert!(matches!(
        values::value(&raw, limits, &mut used),
        Err(Error::Invalid {
            issue: Issue::CountOverflow,
            detail: "native value node count"
        })
    ));
    assert_eq!(used, usize::MAX);
}
