// Called uniformly by the complete workgroup. No subgroup width is assumed.
// One elected lane publishes each subgroup's maximum to the caller's atomic.
// The caller still owns initialization and the barrier between subgroups.
struct Maximum { value: u32, publish: bool }

fn partial_maximum(lane: u32, value: u32) -> Maximum {
    let maximum = subgroupMax(value);
    let publisher = subgroupBroadcastFirst(lane);
    return Maximum(maximum, lane == publisher);
}

// Formula pairs this additive partial with the shared maximum's election.
// Both collectives run before any lane branches on Maximum.publish.
fn partial_sum(value: u32) -> u32 {
    return subgroupAdd(value);
}
