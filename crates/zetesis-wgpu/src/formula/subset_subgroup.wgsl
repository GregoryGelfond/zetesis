// Called uniformly by the complete workgroup. No subgroup width is assumed;
// elected publishers merge their subgroup's exact integer reductions. The
// caller's workgroup barrier remains necessary between distinct subgroups.
fn merge_subset(lane: u32, available: u32, last: u32) {
    let count = subgroupAdd(available);
    let final_atom = subgroupMax(last);
    let publisher = subgroupBroadcastFirst(lane);
    if (lane == publisher) {
        atomicAdd(&subset_count, count);
        atomicMax(&subset_last, final_atom);
    }
}
