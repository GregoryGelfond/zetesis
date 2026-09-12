// One workgroup per frozen candidate. Domains are sets {false,true} encoded
// as 1,2,3. Every atomic intersection preserves all satisfying completions.
struct Params {
    atoms: u32, nodes: u32, roots: u32, variables: u32,
    words: u32, worlds: u32, max_rounds: u32, max_work: u32,
    setup_work: u32, sweep_work: u32, epoch: u32, reserved: u32,
}
struct Node { tag: u32, left: u32, right: u32, output: u32, }
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> nodes: array<Node>;
@group(0) @binding(2) var<storage, read> roots: array<u32>;
@group(0) @binding(3) var<storage, read> seeds: array<u32>;
@group(0) @binding(4) var<storage, read_write> frozen: array<u32>;
@group(0) @binding(5) var<storage, read_write> domains: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> results: array<u32>;
var<workgroup> changed: atomic<u32>;
var<workgroup> conflict: atomic<u32>;
var<workgroup> bad_root: atomic<u32>;
var<workgroup> stage: u32;
var<workgroup> subset_count: atomic<u32>;
var<workgroup> subset_last: atomic<u32>;

fn contains(world: u32, atom: u32) -> bool {
    return (seeds[world * params.words + atom / 32u] & (1u << (atom % 32u))) != 0u;
}
fn operation(tag: u32, left: bool, right: bool) -> bool {
    if (tag == 2u) { return left && right; }
    if (tag == 3u) { return left || right; }
    return !left || right;
}
fn narrow(index: u32, allowed: u32) {
    let previous = atomicAnd(&domains[index], allowed);
    let next = previous & allowed;
    if (next != previous) { atomicStore(&changed, 1u); }
    if (next == 0u) { atomicStore(&conflict, 1u); }
}
fn gate(base: u32, node: Node) {
    let left = nodes[node.left].output;
    let right = nodes[node.right].output;
    let dx = atomicLoad(&domains[base + left]);
    let dy = atomicLoad(&domains[base + right]);
    let dz = atomicLoad(&domains[base + node.output]);
    var sx = 0u; var sy = 0u; var sz = 0u;
    for (var row = 0u; row < 8u; row++) {
        let x = row & 1u;
        let y = (row >> 1u) & 1u;
        let z = (row >> 2u) & 1u;
        if ((dx & (1u << x)) == 0u || (dy & (1u << y)) == 0u || (dz & (1u << z)) == 0u) { continue; }
        if ((left == right && x != y) || (left == node.output && x != z) || (right == node.output && y != z)) { continue; }
        if (operation(node.tag, x == 1u, y == 1u) != (z == 1u)) { continue; }
        sx |= 1u << x; sy |= 1u << y; sz |= 1u << z;
    }
    narrow(base + left, sx);
    narrow(base + right, sy);
    narrow(base + node.output, sz);
}
fn finish(world: u32, status: u32, rounds: u32, work: u32) {
    let base = world * 6u;
    results[base] = params.epoch;
    results[base + 1u] = world;
    results[base + 2u] = status;
    results[base + 3u] = rounds;
    results[base + 4u] = work;
    results[base + 5u] = 0x46525031u;
}
@compute @workgroup_size(64)
fn propagate(@builtin(workgroup_id) group: vec3<u32>,
             @builtin(local_invocation_index) lane: u32) {
    let world = group.x;
    let mask_base = world * params.nodes;
    let base = world * params.variables;
    // Original truth is computed once before ANY frozen simplification. The
    // remaining lanes cooperate on all subsequent domain propagation sweeps.
    if (lane == 0u) {
        atomicStore(&bad_root, 0u);
        for (var index = 0u; index < params.nodes; index++) {
            let node = nodes[index];
            var value = false;
            if (node.tag == 1u) { value = contains(world, node.left); }
            if (node.tag >= 2u) {
                value = operation(node.tag, frozen[mask_base + node.left] != 0u,
                    frozen[mask_base + node.right] != 0u);
            }
            frozen[mask_base + index] = select(0u, 1u, value);
        }
    }
    storageBarrier();
    workgroupBarrier();
    for (var atom = lane; atom < params.atoms; atom += 64u) {
        atomicStore(&domains[base + atom], select(1u, 3u, contains(world, atom)));
    }
    for (var index = lane; index < params.nodes; index += 64u) {
        let node = nodes[index];
        // Atom nodes already share the initialized semantic atom slot. All
        // other outputs occupy distinct dense auxiliary positions.
        if (node.tag != 1u) {
            atomicStore(&domains[base + node.output],
                select(1u, 3u, frozen[mask_base + index] != 0u));
        }
    }
    storageBarrier();
    for (var index = lane; index < params.roots; index += 64u) {
        let root = roots[index];
        if (frozen[mask_base + root] == 0u) { atomicStore(&bad_root, 1u); }
        atomicAnd(&domains[base + nodes[root].output], 2u);
    }
    storageBarrier();
    workgroupBarrier();
    if (lane == 0u) { stage = atomicLoad(&bad_root); }
    if (workgroupUniformLoad(&stage) != 0u) {
        if (lane == 0u) { finish(world, 1u, 0u, params.setup_work); }
        return;
    }
    var rounds = 0u;
    var work = params.setup_work;
    loop {
        if (rounds == params.max_rounds) {
            if (lane == 0u) { finish(world, 4u, rounds, work); }
            return;
        }
        if (params.max_work - work < params.sweep_work) {
            if (lane == 0u) { finish(world, 5u, rounds, work); }
            return;
        }
        if (lane == 0u) {
            atomicStore(&changed, 0u);
            atomicStore(&conflict, 0u);
            atomicStore(&subset_count, 0u);
            atomicStore(&subset_last, 0u);
        }
        workgroupBarrier();
        for (var index = lane; index < params.nodes; index += 64u) {
            let node = nodes[index];
            // Crucial reduct boundary: an M-false composite is just false.
            // Its original connective must not constrain the children.
            if (node.tag >= 2u && frozen[mask_base + index] != 0u) { gate(base, node); }
        }
        storageBarrier();
        workgroupBarrier();
        // J is already constrained to M. Only M-true semantic atoms, never
        // auxiliary gates, can witness a strict subset. All 64 lanes merge a
        // summary, including empty final strides. The last index is used only
        // when exactly one atom remains available across the entire group.
        var available = 0u;
        var last = 0u;
        for (var atom = lane; atom < params.atoms; atom += 64u) {
            if (contains(world, atom) && (atomicLoad(&domains[base + atom]) & 1u) != 0u) {
                available += 1u;
                last = atom;
            }
        }
        atomicAdd(&subset_count, available);
        atomicMax(&subset_last, last);
        workgroupBarrier();
        if (lane == 0u) {
            let total = atomicLoad(&subset_count);
            if (total == 0u) { atomicStore(&conflict, 1u); }
            if (total == 1u) { narrow(base + atomicLoad(&subset_last), 1u); }
        }
        storageBarrier();
        workgroupBarrier();
        rounds += 1u;
        work += params.sweep_work;
        if (lane == 0u) {
            stage = 0u;
            if (atomicLoad(&conflict) != 0u) { stage = 2u; }
            else if (atomicLoad(&changed) == 0u) { stage = 3u; }
        }
        let status = workgroupUniformLoad(&stage);
        if (status != 0u) {
            if (lane == 0u) { finish(world, status, rounds, work); }
            return;
        }
    }
}
