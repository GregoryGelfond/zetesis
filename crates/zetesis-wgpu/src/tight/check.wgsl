// Complete original truth plus ranked producer support, one candidate/workgroup.
// The host accepts only an opaque, checked TightPlan for the complete theory.
struct Params {
    atoms: u32, nodes: u32, roots: u32, producers: u32,
    words: u32, worlds: u32, work: u32, epoch: u32,
}
struct Node { tag: u32, left: u32, right: u32, padding: u32 }
struct Producer { head: u32, body: u32, has_body: u32, padding: u32 }
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> nodes: array<Node>;
@group(0) @binding(2) var<storage, read> roots: array<u32>;
@group(0) @binding(3) var<storage, read> producers: array<Producer>;
@group(0) @binding(4) var<storage, read> candidates: array<u32>;
@group(0) @binding(5) var<storage, read_write> truth: array<u32>;
@group(0) @binding(6) var<storage, read_write> support: array<atomic<u32>>;
@group(0) @binding(7) var<storage, read_write> results: array<u32>;
var<workgroup> first_root: atomic<u32>;
var<workgroup> first_atom: atomic<u32>;

fn contains(world: u32, atom: u32) -> bool {
    return (candidates[world * params.words + atom / 32u] & (1u << (atom % 32u))) != 0u;
}
fn operation(tag: u32, left: bool, right: bool) -> bool {
    if (tag == 2u) { return left && right; }
    if (tag == 3u) { return left || right; }
    return !left || right;
}

@compute @workgroup_size(64)
fn check(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    let world = group.x;
    let values = world * params.nodes;
    let supported = world * params.atoms;
    if (lane == 0u) {
        atomicStore(&first_root, params.roots);
        atomicStore(&first_atom, params.atoms);
        // Children precede their parent. Only this invocation writes truth.
        for (var index = 0u; index < params.nodes; index += 1u) {
            let node = nodes[index];
            var value = false;
            if (node.tag == 1u) { value = contains(world, node.left); }
            if (node.tag >= 2u) {
                value = operation(node.tag, truth[values + node.left] != 0u,
                    truth[values + node.right] != 0u);
            }
            truth[values + index] = select(0u, 1u, value);
        }
    }
    for (var atom = lane; atom < params.atoms; atom += 64u) {
        atomicStore(&support[supported + atom], 0u);
    }
    storageBarrier();
    workgroupBarrier();
    for (var ordinal = lane; ordinal < params.roots; ordinal += 64u) {
        if (truth[values + roots[ordinal]] == 0u) {
            atomicMin(&first_root, ordinal);
        }
    }
    for (var index = lane; index < params.producers; index += 64u) {
        let producer = producers[index];
        var enabled = true;
        if (producer.has_body != 0u) { enabled = truth[values + producer.body] != 0u; }
        if (enabled) { atomicOr(&support[supported + producer.head], 1u); }
    }
    storageBarrier();
    workgroupBarrier();
    for (var atom = lane; atom < params.atoms; atom += 64u) {
        if (contains(world, atom) && atomicLoad(&support[supported + atom]) == 0u) {
            atomicMin(&first_atom, atom);
        }
    }
    workgroupBarrier();
    if (lane == 0u) {
        let root = atomicLoad(&first_root);
        let atom = atomicLoad(&first_atom);
        var status = 0u;
        var witness = 0u;
        if (root < params.roots) { status = 1u; witness = root; }
        else if (atom < params.atoms) { status = 2u; witness = atom; }
        let base = world * 6u;
        results[base] = params.epoch;
        results[base + 1u] = world;
        results[base + 2u] = status;
        results[base + 3u] = witness;
        results[base + 4u] = params.work;
        results[base + 5u] = 0x54535031u;
    }
}
