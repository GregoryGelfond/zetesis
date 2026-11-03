use super::*;

#[test]
fn imported_work_preserves_its_native_refusal_prefix() {
    for limit in [0, 7, u64::MAX] {
        let cancellation = Cancellation::default();
        let mut work = Work {
            template: Some(3),
            bindings: 11,
            keys: 13,
            key_bytes: 17,
            nodes: 19,
            ..Work::new(
                ObjectivePlanLimits {
                    max_work: limit,
                    ..ObjectivePlanLimits::default()
                },
                &cancellation,
            )
        };
        let initial = limit.min(2);
        work.account(initial).unwrap();
        let prefix = ObjectiveBoundStatistics {
            work: initial,
            bindings: 11,
            keys: 13,
            key_bytes: 17,
            nodes: 19,
        };
        let failure = |kind| ObjectiveBoundError {
            kind,
            template: Some(3),
            statistics: prefix,
        };
        if limit < u64::MAX {
            assert_eq!(
                work.account(limit - initial + 1).unwrap_err(),
                failure(ObjectiveBoundErrorKind::Limit(ObjectiveBoundResource::Work))
            );
        }
        if initial != 0 {
            // Representability is checked before the configured ceiling.
            assert_eq!(
                work.account(u64::MAX).unwrap_err(),
                failure(ObjectiveBoundErrorKind::Overflow)
            );
        }
        assert_eq!(work.statistics(), prefix);
        cancellation.cancel();
        for amount in [0, u64::MAX] {
            assert_eq!(
                work.charge(amount).unwrap_err(),
                failure(ObjectiveBoundErrorKind::Control(Stop::Cancelled))
            );
        }
        // Imported work has already happened; cancellation must not erase it.
        work.account(limit - initial).unwrap();
        work.account(0).unwrap();
        let complete = ObjectiveBoundStatistics {
            work: limit,
            ..prefix
        };
        assert_eq!(work.statistics(), complete);
        assert_eq!(
            work.account(1).unwrap_err(),
            ObjectiveBoundError {
                kind: if limit == u64::MAX {
                    ObjectiveBoundErrorKind::Overflow
                } else {
                    ObjectiveBoundErrorKind::Limit(ObjectiveBoundResource::Work)
                },
                template: Some(3),
                statistics: complete,
            }
        );
    }
}

#[test]
fn node_capacity_refusals_preserve_the_work_prefix_and_graph() {
    // Empty conjunction constructs true with two nodes and two child occurrences.
    // Work is admitted first, then nodes, then operands, before any graph append.
    for (max_work, max_nodes, max_operands, refused, charged) in [
        (0, 0, 0, Some(ObjectiveBoundResource::Work), 0),
        (16, 1, 1, Some(ObjectiveBoundResource::Nodes), 1),
        (16, 2, 1, Some(ObjectiveBoundResource::Operands), 1),
        (16, 2, 2, None, 3),
    ] {
        let cancellation = Cancellation::default();
        let mut work = Work {
            template: Some(3),
            bindings: 7,
            keys: 11,
            key_bytes: 13,
            ..Work::new(
                ObjectivePlanLimits {
                    max_nodes,
                    max_operands,
                    max_work,
                    ..ObjectivePlanLimits::default()
                },
                &cancellation,
            )
        };
        let mut nodes = FormulaNodes::default();
        let result = work.node(&mut nodes, NodeView::And(&[]));
        let statistics = ObjectiveBoundStatistics {
            work: charged,
            bindings: 7,
            keys: 11,
            key_bytes: 13,
            nodes: if refused.is_some() { 0 } else { 2 },
        };
        if let Some(resource) = refused {
            assert_eq!(
                result.unwrap_err(),
                ObjectiveBoundError {
                    kind: ObjectiveBoundErrorKind::Limit(resource),
                    template: Some(3),
                    statistics,
                }
            );
            assert_eq!((nodes.view().len(), nodes.parts().occurrences()), (0, 0));
        } else {
            result.unwrap();
            assert_eq!((nodes.view().len(), nodes.parts().occurrences()), (2, 2));
        }
        assert_eq!(work.statistics(), statistics);
    }
}
