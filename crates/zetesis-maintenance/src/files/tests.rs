//! Bounded reading retains native error priority and its actual traffic prefix.

use super::{Error, Limits, Tree};

#[test]
fn file_refusal_precedes_total_refusal_without_consuming_budget() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("input"), b"ab").unwrap();
    let mut tree = Tree::new(
        directory.path(),
        Limits {
            file_bytes: 1,
            total_bytes: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    assert!(matches!(
        tree.read("input"),
        Err(Error::Limit {
            resource: "file bytes",
            limit: 1,
        })
    ));
    assert_eq!(tree.remaining, 1);
}

#[test]
fn repeated_reads_keep_the_accepted_traffic_prefix() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("pair"), b"ab").unwrap();
    std::fs::write(directory.path().join("one"), b"c").unwrap();
    let mut tree = Tree::new(
        directory.path(),
        Limits {
            file_bytes: 2,
            total_bytes: 3,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(tree.read("pair").unwrap(), b"ab");
    assert_eq!(tree.remaining, 1);
    assert!(matches!(
        tree.read("pair"),
        Err(Error::Limit {
            resource: "total read bytes",
            limit: 3,
        })
    ));
    assert_eq!(tree.remaining, 1);
    assert_eq!(tree.read("one").unwrap(), b"c");
    assert_eq!(tree.remaining, 0);
}
