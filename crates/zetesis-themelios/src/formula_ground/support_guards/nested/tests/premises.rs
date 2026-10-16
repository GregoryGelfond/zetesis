use super::*;

/// Establish a prefix under the ordinary budget, then test premise selection
/// under the supplied limits. Five distinct atoms distinguish a compact row
/// from both the complete binary fold and an existing shorter prefix.
fn with_prefix<T>(
    length: usize,
    limits: &FormulaLimits,
    cancellation: Option<&Cancellation>,
    action: impl FnOnce(&mut Builder<'_, '_, '_>, &[usize], usize) -> T,
) -> T {
    testing::Fixture::default().with(location(0), |_, computation, counters| {
        *counters = std::mem::take(counters).with_cancellation(cancellation);
        let defaults = FormulaLimits::default();
        let mut budget = Budget::new(ExpansionLimits::default(), 0);
        let mut builder = Builder::empty(
            computation,
            &defaults,
            &mut budget,
            counters,
            Purpose::Theory,
            None,
            location(0),
        )
        .unwrap();
        builder.initialize(location(0)).unwrap();
        let mut heads = Vec::new();
        for atom in 0..5 {
            heads.push(builder.node(Node::Atom(atom), location(0)).unwrap());
        }
        let mut prefix = FALSUM;
        for &head in &heads[..length] {
            prefix = builder.or(prefix, head, location(0)).unwrap();
        }
        builder.limits = limits;
        action(&mut builder, &heads, prefix)
    })
}

#[test]
fn complete_fold_is_reused_without_publication() {
    with_prefix(
        5,
        &FormulaLimits::default(),
        None,
        |builder, heads, fold| {
            let count = builder.nodes.view().len();
            let occurrences = builder.nodes.parts().occurrences();
            assert_eq!(
                premises(builder, heads, location(0)).unwrap(),
                [
                    Some(Premise {
                        node: fold,
                        heads: 5
                    }),
                    None
                ]
            );
            assert_eq!(builder.nodes.view().len(), count);
            assert_eq!(builder.nodes.parts().occurrences(), occurrences);
        },
    );
}

#[test]
fn partial_fold_covers_only_its_existing_heads() {
    with_prefix(
        3,
        &FormulaLimits::default(),
        None,
        |builder, heads, prefix| {
            let count = builder.nodes.view().len();
            let occurrences = builder.nodes.parts().occurrences();
            let selected = [
                Some(Premise {
                    node: prefix,
                    heads: 3,
                }),
                None,
            ];
            assert_eq!(premises(builder, heads, location(0)).unwrap(), selected);
            assert_eq!(builder.nodes.view().len(), count);
            assert_eq!(builder.nodes.parts().occurrences(), occurrences);
            assert!(
                nodes::find(
                    &builder.node_indices,
                    &builder.nodes,
                    Node::Or(&[prefix, heads[3]]),
                    location(0),
                    || Ok(())
                )
                .unwrap()
                .is_none(),
                "the missing intermediate prefix was never published"
            );
            assert_eq!(premises(builder, heads, location(0)).unwrap(), selected);
            assert_eq!(builder.nodes.view().len(), count);
        },
    );
}

#[test]
fn absent_premises_publish_nothing() {
    with_prefix(1, &FormulaLimits::default(), None, |builder, heads, _| {
        let count = builder.nodes.view().len();
        let occurrences = builder.nodes.parts().occurrences();
        assert_eq!(premises(builder, heads, location(0)).unwrap(), [None, None]);
        assert_eq!(builder.nodes.view().len(), count);
        assert_eq!(builder.nodes.parts().occurrences(), occurrences);
    });
}

#[test]
fn existing_flat_row_is_selected_without_a_binary_prefix() {
    with_prefix(1, &FormulaLimits::default(), None, |builder, heads, _| {
        let flat = builder.node(Node::Or(heads), location(0)).unwrap();
        let count = builder.nodes.view().len();
        assert_eq!(
            premises(builder, heads, location(0)).unwrap(),
            [
                None,
                Some(Premise {
                    node: flat,
                    heads: 5
                })
            ]
        );
        assert_eq!(builder.nodes.view().len(), count);
    });
}

#[test]
fn distinct_flat_and_binary_premises_are_retained() {
    with_prefix(
        3,
        &FormulaLimits::default(),
        None,
        |builder, heads, prefix| {
            let flat = builder.node(Node::Or(heads), location(0)).unwrap();
            let count = builder.nodes.view().len();
            assert_eq!(
                premises(builder, heads, location(0)).unwrap(),
                [
                    Some(Premise {
                        node: prefix,
                        heads: 3
                    }),
                    Some(Premise {
                        node: flat,
                        heads: 5
                    }),
                ]
            );
            assert_eq!(builder.nodes.view().len(), count);
        },
    );
}

#[test]
fn either_existing_premise_propagates_its_condition() {
    for require_flat in [false, true] {
        with_prefix(
            3,
            &FormulaLimits::default(),
            None,
            |builder, heads, prefix| {
                let flat = builder.node(Node::Or(heads), location(0)).unwrap();
                let condition = builder.node(Node::Atom(5), location(0)).unwrap();
                let mut roots = vec![if require_flat { flat } else { prefix }];
                for premise in premises(builder, heads, location(0))
                    .unwrap()
                    .into_iter()
                    .flatten()
                {
                    let implication = builder
                        .node(Node::Implies(premise.node, condition), location(0))
                        .unwrap();
                    let negative = builder.neg(implication, location(0)).unwrap();
                    roots.push(builder.neg(negative, location(0)).unwrap());
                }
                let theory = Theory::new(
                    6,
                    std::mem::take(&mut builder.nodes).into_parts(),
                    roots,
                    FormulaLimits::default().theory,
                )
                .unwrap();
                let region = closed(&theory);
                assert!(region.is_held(5));
                assert!((0..5).all(|atom| region.is_open(atom)));
            },
        );
    }
}

#[test]
fn premise_work_refusal_preserves_the_graph() {
    let defaults = FormulaLimits::default();
    let exact = with_prefix(3, &defaults, None, |builder, heads, _| {
        premises(builder, heads, location(0)).unwrap();
        builder.counters.accounting.work
    });
    for cutoff in 0..=exact {
        let limits = FormulaLimits {
            max_work: cutoff,
            ..defaults
        };
        with_prefix(3, &limits, None, |builder, heads, _| {
            let count = builder.nodes.view().len();
            let occurrences = builder.nodes.parts().occurrences();
            let result = premises(builder, heads, location(0));
            if cutoff == exact {
                assert!(result.unwrap()[0].is_some());
            } else {
                assert!(matches!(
                    result,
                    Err(FormulaFailure::Limit {
                        resource: FormulaResource::Work,
                        ..
                    })
                ));
            }
            assert_eq!(builder.nodes.view().len(), count);
            assert_eq!(builder.nodes.parts().occurrences(), occurrences);
        });
    }
}

#[test]
fn cancelled_premise_publishes_nothing() {
    let cancellation = Cancellation::default();
    with_prefix(
        3,
        &FormulaLimits::default(),
        Some(&cancellation),
        |builder, heads, _| {
            let count = builder.nodes.view().len();
            cancellation.cancel();
            assert!(matches!(
                premises(builder, heads, location(0)),
                Err(FormulaFailure::Interrupted {
                    reason: Stop::Cancelled,
                    ..
                })
            ));
            assert_eq!(builder.nodes.view().len(), count);
        },
    );
}
