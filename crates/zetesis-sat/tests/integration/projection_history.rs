//! Public history refusals preserve semantic evidence without claiming coverage.

use std::num::NonZeroUsize;

use crate::support::clause_search::by_clauses;
use zetesis_ferraris::{AdmissionLimits, Node, Theory};
use zetesis_sat::{
    BatchError, BatchLimits, BatchVerdict, Cancellation, Incomplete, Limits, ProjectionLimits,
    ProjectionResource,
};

fn choices() -> Theory {
    // Two independent choices: every one of the four interpretations is stable.
    Theory::new(
        2,
        vec![
            Node::Atom(0),
            Node::False,
            Node::Implies(0, 1),
            Node::Or(0, 2),
            Node::Atom(1),
            Node::Implies(4, 1),
            Node::Or(4, 5),
        ],
        vec![3, 6],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn history_refusals_preserve_the_checked_answer() {
    let input = choices();
    let mut reference = by_clauses(&input, Limits::default(), Cancellation::default()).unwrap();
    let header = usize::try_from(reference.statistics().projections.retained_bytes).unwrap();
    let expected: Vec<_> = reference.next().unwrap().unwrap().atoms().collect();
    for (resource, projections, required, limit) in [
        (
            ProjectionResource::Entries,
            ProjectionLimits {
                max_entries: 0,
                ..ProjectionLimits::default()
            },
            1,
            0,
        ),
        (
            ProjectionResource::Nodes,
            ProjectionLimits {
                max_nodes: 0,
                ..ProjectionLimits::default()
            },
            3,
            0,
        ),
        (
            ProjectionResource::Bytes,
            ProjectionLimits {
                max_bytes: header,
                ..ProjectionLimits::default()
            },
            (header + 3 * 8) as u128,
            header as u128,
        ),
    ] {
        let mut search = by_clauses(
            &input,
            Limits {
                projections,
                ..Limits::default()
            },
            Cancellation::default(),
        )
        .unwrap();
        let model = search.next().unwrap().unwrap();
        assert!(model.theory().same_instance(&input));
        assert_eq!(model.atoms().collect::<Vec<_>>(), expected);
        assert!(
            zetesis_ferraris::check(
                &input,
                &model,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default()
            )
            .unwrap()
            .accepted()
        );
        let before_stop = search.statistics();
        assert_eq!(before_stop.stable_models, 1);
        assert_eq!(before_stop.projections.entries, 0);
        assert_eq!(before_stop.projections.nodes, 0);
        assert!(before_stop.projections.work > 0);
        assert!(before_stop.projections.work <= before_stop.search.work);
        assert_eq!(
            search.next().unwrap().unwrap_err(),
            Incomplete::ProjectionLimit {
                resource,
                required,
                limit
            }
        );
        assert!(!search.exhausted());
        assert!(search.next().is_none());
        assert_eq!(search.statistics(), before_stop);
    }
}

#[test]
fn checker_retry_keeps_the_original_history_stop() {
    let input = Theory::new(0, vec![], vec![], AdmissionLimits::default()).unwrap();
    let mut search = by_clauses(
        &input,
        Limits {
            projections: ProjectionLimits {
                max_entries: 0,
                ..ProjectionLimits::default()
            },
            ..Limits::default()
        },
        Cancellation::default(),
    )
    .unwrap();
    let batch = BatchLimits {
        max_candidates: NonZeroUsize::new(2).unwrap(),
        max_pending_bytes: 1024,
    };
    let stopped = search.next_batch(batch, |_, candidates| {
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].atoms().count(), 0);
        Err::<Vec<BatchVerdict>, _>("checker stopped")
    });
    assert!(matches!(
        stopped,
        Err(BatchError::Checker("checker stopped"))
    ));
    let before = search.statistics();
    assert_eq!(before.candidates, 1);
    assert_eq!(before.projections.entries, 0);
    assert_eq!(search.batch_statistics().pending, 1);
    let models = search
        .next_batch(batch, |_, candidates| {
            Ok::<_, std::convert::Infallible>(vec![BatchVerdict::Residual; candidates.len()])
        })
        .unwrap();
    assert_eq!(models.len(), 1);
    assert!(models[0].theory().same_instance(&input));
    assert_eq!(models[0].atoms().count(), 0);
    assert_eq!(search.statistics().candidates, 1);
    assert_eq!(search.statistics().projections, before.projections);
    assert_eq!(search.batch_statistics().pending, 0);
    assert!(matches!(
        search.next_batch(batch, |_, _| -> Result<Vec<BatchVerdict>, ()> {
            panic!("the original history stop precedes another checker call")
        }),
        Err(BatchError::Search(Incomplete::ProjectionLimit {
            resource: ProjectionResource::Entries,
            required: 1,
            limit: 0,
        }))
    ));
    assert!(!search.exhausted());
}
