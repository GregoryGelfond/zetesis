use super::*;

#[test]
fn overflowing_source_counts_refuse_at_the_maximum_ceiling() {
    let mut used = usize::MAX - 1;
    assert_eq!(charge(&mut used, 1, Resource::Nodes, usize::MAX), Ok(()));
    assert_eq!(used, usize::MAX);
    assert_eq!(
        charge(&mut used, 1, Resource::Nodes, usize::MAX),
        Err(Limit {
            resource: Resource::Nodes,
            limit: usize::MAX,
            observed: usize::MAX,
        })
    );
    assert_eq!(used, usize::MAX);
}

#[test]
fn zero_source_charge_keeps_an_imported_prefix_on_refusal() {
    let mut used = 7;
    assert_eq!(
        charge(&mut used, 0, Resource::TextBytes, 3),
        Err(Limit {
            resource: Resource::TextBytes,
            limit: 3,
            observed: 7,
        })
    );
    assert_eq!(used, 7);
}

#[test]
fn source_depth_refuses_before_charging_a_node() {
    let mut budget = Budget::new(Limits {
        nodes: 0,
        depth: 0,
        body_elements: 0,
        text_bytes: 0,
    });
    assert_eq!(
        budget.node(1),
        Err(Limit {
            resource: Resource::Depth,
            limit: 0,
            observed: 1,
        })
    );
    assert_eq!(budget.nodes, 0);
}
