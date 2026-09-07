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
// Eight relation rows encoded in one byte: x + 2*y + 4*z.
// These row bits are neither candidate bits nor the atomic domain encoding.
fn gate_rows(domain: u32, no: u32, yes: u32) -> u32 {
    return select(0u, no, (domain & 1u) != 0u) |
        select(0u, yes, (domain & 2u) != 0u);
}
fn gate_support(rows: u32, no: u32, yes: u32) -> u32 {
    return select(0u, 1u, (rows & no) != 0u) |
        select(0u, 2u, (rows & yes) != 0u);
}
fn gate(base: u32, node: Node) {
    let left = nodes[node.left].output;
    let right = nodes[node.right].output;
    let dx = atomicLoad(&domains[base + left]);
    let dy = atomicLoad(&domains[base + right]);
    let dz = atomicLoad(&domains[base + node.output]);
    var rows = 0xd2u; // z = (!x || y)
    if (node.tag == 2u) { rows = 0x87u; } // z = (x && y)
    if (node.tag == 3u) { rows = 0xe1u; } // z = (x || y)
    rows &= gate_rows(dx, 0x55u, 0xaau);
    rows &= gate_rows(dy, 0x33u, 0xccu);
    rows &= gate_rows(dz, 0x0fu, 0xf0u);
    // Compare physical slots. Aliased atomic loads need not be identical.
    if (left == right) { rows &= 0x99u; }
    if (left == node.output) { rows &= 0xa5u; }
    if (right == node.output) { rows &= 0xc3u; }
    narrow(base + left, gate_support(rows, 0x55u, 0xaau));
    narrow(base + right, gate_support(rows, 0x33u, 0xccu));
    narrow(base + node.output, gate_support(rows, 0x0fu, 0xf0u));
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
        atomicStore(&domains[base + params.atoms + index],
            select(1u, 3u, frozen[mask_base + index] != 0u));
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
        if (lane == 0u) {
            // J is already constrained to M. The strict-subset clause requires
            // at least one M-true semantic atom (never a gate) to become false.
            var available = 0u;
            var last = 0u;
            for (var atom = 0u; atom < params.atoms; atom++) {
                if (contains(world, atom) && (atomicLoad(&domains[base + atom]) & 1u) != 0u) {
                    available += 1u;
                    last = atom;
                }
            }
            if (available == 0u) { atomicStore(&conflict, 1u); }
            if (available == 1u) { narrow(base + last, 1u); }
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
