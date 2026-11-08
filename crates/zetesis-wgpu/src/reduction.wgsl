// Uniform unsigned reductions with zero identities. Each portable lane is
// its own part. Callers merge the returned parts into initialized atomics and
// retain their workgroup barrier before reading the complete reduction.
struct Maximum { value: u32, publish: bool }

fn partial_maximum(lane: u32, value: u32) -> Maximum {
    return Maximum(value, true);
}

// Compute before branching on Maximum.publish so both variants are uniform.
// The caller bounds the sum of all lane contributions within u32.
fn partial_sum(value: u32) -> u32 {
    return value;
}
