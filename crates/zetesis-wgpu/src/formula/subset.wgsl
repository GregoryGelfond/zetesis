// Every lane contributes, including empty final strides. The following
// workgroup barrier publishes the complete count and last available atom.
fn merge_subset(lane: u32, available: u32, last: u32) {
    atomicAdd(&subset_count, available);
    atomicMax(&subset_last, last);
}
