// One workgroup owns one world. All instances read immutable round snapshots.
// Newly derived heads go only to the separate output delta, never to antecedents.
struct Dimensions { words: u32, rules: u32, worlds: u32, epoch: u32 }
@group(0) @binding(0) var<uniform> dimensions: Dimensions;
@group(0) @binding(1) var<storage, read> offsets: array<u32>;
@group(0) @binding(2) var<storage, read> records: array<u32>;
@group(0) @binding(3) var<storage, read> snapshots: array<u32>;
@group(0) @binding(4) var<storage, read> seeds: array<u32>;
@group(0) @binding(5) var<storage, read_write> output: array<atomic<u32>>;

const CONSTRAINT_HEAD: u32 = 0u;
const RECORD_HEADER_WORDS: u32 = 4u;

fn positive(world: u32, atom: u32) -> bool {
    return (snapshots[world * dimensions.words + atom / 32u] & (1u << (atom % 32u))) != 0u;
}
fn frozen(world: u32, atom: u32) -> bool {
    return (seeds[world * dimensions.words + atom / 32u] & (1u << (atom % 32u))) != 0u;
}

@compute @workgroup_size(64)
fn consequence(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    let world = group.x;
    let result = world * (dimensions.words + 3u);
    if lane == 0u {
        atomicStore(&output[result + dimensions.words + 1u], world);
        atomicStore(&output[result + dimensions.words + 2u], dimensions.epoch);
    }
    for (var rule = lane; rule < dimensions.rules; rule += 64u) {
        let offset = offsets[rule];
        let head = records[offset];
        let positives = records[offset + 1u];
        let required_true = records[offset + 2u];
        let required_false = records[offset + 3u];
        var cursor = offset + RECORD_HEADER_WORDS;
        var enabled = true;
        for (var index = 0u; index < positives; index += 1u) {
            enabled = enabled && positive(world, records[cursor + index]);
        }
        cursor += positives;
        for (var index = 0u; index < required_true; index += 1u) {
            enabled = enabled && frozen(world, records[cursor + index]);
        }
        cursor += required_true;
        for (var index = 0u; index < required_false; index += 1u) {
            enabled = enabled && !frozen(world, records[cursor + index]);
        }
        if enabled {
            if head == CONSTRAINT_HEAD {
                atomicOr(&output[result + dimensions.words], 1u);
            } else {
                let atom = head - 1u;
                if !positive(world, atom) {
                    atomicOr(&output[result + atom / 32u], 1u << (atom % 32u));
                }
            }
        }
    }
}
