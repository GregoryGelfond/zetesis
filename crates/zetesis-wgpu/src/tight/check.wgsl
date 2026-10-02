// Complete original truth plus ranked producer support, one candidate/workgroup.
// The host accepts only an opaque, checked TightPlan for the complete theory.
// Wire constants mirror tight/packing.rs. Zero remains ordinary Boolean false.
// Support bit a denotes an enabled producer for a in this candidate's row.
const WORKGROUP_SIZE: u32 = 64u;
const NODE_FALSE: u32 = 0u;
const NODE_ATOM: u32 = 1u;
const NODE_AND: u32 = 2u;
const NODE_OR: u32 = 3u;
const NODE_IMPLIES: u32 = 4u;
const STATUS_STABLE: u32 = 0u;
const STATUS_NOT_MODEL: u32 = 1u;
const STATUS_RESIDUAL: u32 = 2u;
const RESULT_WORDS: u32 = 6u;
const RESULT_MAGIC: u32 = 0x54535031u;
const RESULT_GROUPED_MAGIC: u32 = 0x54534731u;
struct Params {
    atoms: u32, nodes: u32, roots: u32, producers: u32,
    words: u32, worlds: u32, work: u32, epoch: u32,
}
struct Node { tag: u32, left: u32, right: u32, padding: u32 }
struct Producer { head: u32, body: u32, has_body: u32, padding: u32 }
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> nodes: array<Node>;
@group(0) @binding(2) var<storage, read> roots: array<u32>;
// Atomic construction stores canonical records. Grouped construction stores
// word-grouped records followed by W+1 half-open producer offsets.
@group(0) @binding(3) var<storage, read> producers: array<u32>;
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
    if (tag == NODE_AND) { return left && right; }
    if (tag == NODE_OR) { return left || right; }
    return !left || right;
}

fn producer_at(index: u32) -> Producer {
    let base = index * 4u;
    return Producer(producers[base], producers[base + 1u],
        producers[base + 2u], producers[base + 3u]);
}

fn enabled(producer: Producer, values: u32) -> bool {
    if (producer.has_body != 0u) { return truth[values + producer.body] != 0u; }
    return true;
}

fn check_support(world: u32, lane: u32, grouped: bool) {
    let values = world * params.nodes;
    let supported = world * params.words;
    if (lane == 0u) {
        atomicStore(&first_root, params.roots);
        atomicStore(&first_atom, params.atoms);
        // Children precede their parent. Only this invocation writes truth.
        for (var index = 0u; index < params.nodes; index += 1u) {
            let node = nodes[index];
            var value = false;
            if (node.tag == NODE_ATOM) { value = contains(world, node.left); }
            if (node.tag >= NODE_AND) {
                value = operation(node.tag, truth[values + node.left] != 0u,
                    truth[values + node.right] != 0u);
            }
            truth[values + index] = select(0u, 1u, value);
        }
    }
    if (!grouped) {
        for (var word = lane; word < params.words; word += WORKGROUP_SIZE) {
            atomicStore(&support[supported + word], 0u);
        }
    }
    storageBarrier();
    workgroupBarrier();
    for (var ordinal = lane; ordinal < params.roots; ordinal += WORKGROUP_SIZE) {
        if (truth[values + roots[ordinal]] == 0u) {
            atomicMin(&first_root, ordinal);
        }
    }
    if (grouped) {
        let offsets = params.producers * 4u;
        for (var word = lane; word < params.words; word += WORKGROUP_SIZE) {
            var mask = 0u;
            let end = producers[offsets + word + 1u];
            for (var index = producers[offsets + word]; index < end; index += 1u) {
                let producer = producer_at(index);
                if (enabled(producer, values)) { mask |= 1u << (producer.head % 32u); }
            }
            // Exactly one invocation owns this word. Empty groups overwrite
            // earlier batches with zero; valid heads never set padding bits.
            atomicStore(&support[supported + word], mask);
        }
    } else {
        for (var index = lane; index < params.producers; index += WORKGROUP_SIZE) {
            let producer = producer_at(index);
            // Distinct heads may share a word. Atomic OR preserves every enabled
            // producer's bit, including duplicate producers for the same head.
            if (enabled(producer, values)) {
                atomicOr(&support[supported + producer.head / 32u], 1u << (producer.head % 32u));
            }
        }
    }
    storageBarrier();
    workgroupBarrier();
    for (var word = lane; word < params.words; word += WORKGROUP_SIZE) {
        // Candidate padding is zero. The least bit missing from complete
        // support is the least unsupported atom in this word. Reducing those
        // witnesses retains the least atom across all words and invocations.
        let missing = candidates[world * params.words + word] &
            ~atomicLoad(&support[supported + word]);
        if (missing != 0u) {
            atomicMin(&first_atom, word * 32u + firstTrailingBit(missing));
        }
    }
    workgroupBarrier();
    if (lane == 0u) {
        let root = atomicLoad(&first_root);
        let atom = atomicLoad(&first_atom);
        var status = STATUS_STABLE;
        var witness = 0u;
        if (root < params.roots) { status = STATUS_NOT_MODEL; witness = root; }
        else if (atom < params.atoms) { status = STATUS_RESIDUAL; witness = atom; }
        let base = world * RESULT_WORDS;
        results[base] = params.epoch;
        results[base + 1u] = world;
        results[base + 2u] = status;
        results[base + 3u] = witness;
        results[base + 4u] = params.work;
        // The same branch selector determines support construction and its
        // receipt. The host refuses a record from the other physical policy.
        results[base + 5u] = select(RESULT_MAGIC, RESULT_GROUPED_MAGIC, grouped);
    }
}

@compute @workgroup_size(WORKGROUP_SIZE)
fn check(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    check_support(group.x, lane, false);
}

@compute @workgroup_size(WORKGROUP_SIZE)
fn check_grouped(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    check_support(group.x, lane, true);
}
