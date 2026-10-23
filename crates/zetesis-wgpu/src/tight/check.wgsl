// Complete original truth plus ranked producer support, one candidate/workgroup.
// The host accepts only an opaque, checked TightPlan for the complete theory.
// Wire constants mirror tight/packing.rs and tight/update.rs. Truth bit 0 is
// the value. Bit 1 records a change and is defined only for nodes a complete or
// dense route wrote in the current dispatch: the update route neither reads nor
// maintains it, and writes changed truth with bit 1 clear. A dense dispatch
// after an update rewrites a condition's bit 1 before its row's owner reads it,
// because conditions precede their rows in either schedule. Every truth reader
// masks bit 0. Support bit a denotes an enabled producer for a in this
// candidate's row; only the complete and dense routes build it.
// Every workgroup location an invocation reads was written earlier in its
// workgroup, by the same invocation or by another ordered before the read by a
// barrier or uniform load, and every workgroup array index is within its array.
// The pipeline keeps WebGPU's zero-fill of workgroup memory, which this rule
// makes unobservable; the rule is held pending a measurement of whether
// dropping the fill pays.
// Each support policy has two entry points. `check` and `check_grouped` carry
// the update route, for a theory whose schedule may engage it. `check_dense`
// and `check_grouped_dense` evaluate complete or dense truth only, for a theory
// whose route is declined for its whole residency: their call graph reads no
// update state, so a pipeline compiled from them binds no state (binding 8)
// and allocates none of the update's workgroup variables. Both kinds write the
// same records; the dense kind records every slot unseeded.
override WORKGROUP_SIZE: u32 = 64u;
const NODE_FALSE: u32 = 0u;
const NODE_ATOM: u32 = 1u;
const NODE_AND: u32 = 2u;
const NODE_OR: u32 = 3u;
const NODE_IMPLIES: u32 = 4u;
const NODE_ALL: u32 = 5u;
const NODE_ANY: u32 = 6u;
const STATUS_STABLE: u32 = 0u;
const STATUS_NOT_MODEL: u32 = 1u;
const STATUS_RESIDUAL: u32 = 2u;
const RESULT_WORDS: u32 = 9u;
const RESULT_MAGIC: u32 = 0x54535031u;
const RESULT_GROUPED_MAGIC: u32 = 0x54534731u;
const ROUTE_COMPLETE: u32 = 0u;
const ROUTE_DENSE: u32 = 1u;
const ROUTE_UPDATE: u32 = 2u;
const ROUTE_BEYOND_THRESHOLD: u32 = 3u;
const READER_NODE: u32 = 0u;
const READER_PRODUCER: u32 = 1u;
const SUMMARY_WORDS: u32 = 3u;
struct Params {
    atoms: u32, nodes: u32, roots: u32, producers: u32,
    words: u32, levels: u32, work: u32, epoch: u32,
    threshold: u32, allowed: u32, readers: u32, atom_nodes: u32,
    count_bits: u32, state_words: u32, reserved_a: u32, reserved_b: u32,
}
struct Node { tag: u32, left: u32, right: u32, padding: u32 }
struct Producer { head: u32, body: u32, has_body: u32, padding: u32 }
// Word offsets of one slot's update state.
struct Slot {
    summarized: u32, false_count: u32, unsupported_count: u32,
    retained: u32, touched: u32, unsupported: u32,
    false_roots: u32, queued: u32, counts: u32,
}
@group(0) @binding(0) var<uniform> params: Params;
// Header word 3 holds a scheduled node's position in the level order.
@group(0) @binding(1) var<storage, read> nodes: array<u32>;
@group(0) @binding(2) var<storage, read> roots: array<u32>;
// Atomic construction stores canonical records. Grouped construction stores
// word-grouped records followed by W+1 half-open producer offsets. Once updates
// are admitted the readers index follows: node offsets and entries at
// `params.readers`, atom offsets and atom-node entries at `params.atom_nodes`.
@group(0) @binding(3) var<storage, read> producers: array<u32>;
@group(0) @binding(4) var<storage, read> candidates: array<u32>;
@group(0) @binding(5) var<storage, read_write> frozen: array<u32>;
@group(0) @binding(6) var<storage, read_write> support: array<atomic<u32>>;
// One record per slot: epoch, slot, status, witness, work, marker, route, nodes
// evaluated, and whether the slot's update state was seeded, which every route
// writes and the slot's next dispatch reads beside its authority.
@group(0) @binding(7) var<storage, read_write> results: array<u32>;
// Per-slot update state of `params.state_words` words: the summary block
// (summarized, false-root count, unsupported count), the retained candidate,
// the touched and unsupported atom sets, the false-root set over root
// positions, the queued set over schedule positions, and packed support counts
// of `params.count_bits` bits per head. Only read or written while the update
// route is allowed; otherwise it may be a placeholder. The dense entry points
// never name it, so their pipelines do not bind it.
@group(0) @binding(8) var<storage, read_write> state: array<atomic<u32>>;
// The least false root and unsupported atom: invocation 0 stores their bounds
// at entry; without the update route the dense routes' barriers, and with it
// the uniform load of entry_route, order those stores before any other
// invocation's atomicMin, and invocation 0 reads both after the final barrier.
var<workgroup> first_root: atomic<u32>;
var<workgroup> first_atom: atomic<u32>;
// Update-route counters, untouched without the route: invocation 0 zeroes them
// before storing entry_route, whose uniform load orders the zeroes before any
// other invocation's atomicAdd. Invocation 0 reads changed_atoms after the
// barrier that ends the change count, and performed and evaluated, on the
// update route only, after the final barrier.
var<workgroup> changed_atoms: atomic<u32>;
var<workgroup> performed: atomic<u32>;
var<workgroup> evaluated: atomic<u32>;
// Values steering barriered control flow, written by invocation 0 and read by
// every invocation through workgroupUniformLoad, one variable per decision.
// chosen_route is written after the barrier that ends the change count, and
// verdict_counts is stored whole; its components are read from the loaded
// value.
var<workgroup> entry_route: u32;
var<workgroup> chosen_route: u32;
var<workgroup> summary_flag: u32;
var<workgroup> verdict_counts: vec2<u32>;

// Headers occupy 4*N words; wide rows occupy the checked appended tail.
fn node_at(index: u32) -> Node {
    let base = index * 4u;
    return Node(nodes[base], nodes[base + 1u], nodes[base + 2u], nodes[base + 3u]);
}
fn operand_at(node: Node, index: u32) -> u32 {
    return nodes[params.nodes * 4u + node.left + index];
}

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
    if (producer.has_body != 0u) { return (frozen[values + producer.body] & 1u) != 0u; }
    return true;
}

// Boolean evaluation is itself a cheap reduction. Compute the value and then
// compare it with the retained value; a separate changed-operand scan would
// reread the same row. Absorbing values end a wide reduction immediately.
// Every child has already been evaluated in dependency order, so skipping its
// value here does not skip a required child computation or diagnostic.
fn boolean_original(world: u32, values: u32, node: Node) -> bool {
    if (node.tag == NODE_ATOM) { return contains(world, node.left); }
    if (node.tag >= NODE_AND && node.tag <= NODE_IMPLIES) {
        return operation(node.tag, (frozen[values + node.left] & 1u) != 0u,
            (frozen[values + node.right] & 1u) != 0u);
    }
    if (node.tag == NODE_ALL || node.tag == NODE_ANY) {
        let identity = node.tag == NODE_ALL;
        for (var child = 0u; child < node.right; child++) {
            let next = (frozen[values + operand_at(node, child)] & 1u) != 0u;
            if (next != identity) { return !identity; }
        }
        return identity;
    }
    return node.tag == 10u;
}

// Both schedules call the same scalar transfer. The change bit records the
// output change, including when another child changes behind an absorbing value.
// Native row owners retain their separate, potentially cheaper reuse check.
fn original(world: u32, values: u32, index: u32, reuse: bool) {
    let node = node_at(index);
    if (node.tag >= 7u && node.tag <= 9u) {
        numeric_original(values, index, node, reuse);
        return;
    }
    let value = boolean_original(world, values, node);
    let next = select(0u, 1u, value);
    let changed = reuse && next != (frozen[values + index] & 1u);
    frozen[values + index] = next | select(0u, 2u, changed);
}

fn slot_at(world: u32) -> Slot {
    let base = world * params.state_words;
    let retained = base + SUMMARY_WORDS;
    let touched = retained + params.words;
    let unsupported = touched + params.words;
    let false_roots = unsupported + params.words;
    let queued = false_roots + (params.roots + 31u) / 32u;
    let counts = queued + (params.nodes + 31u) / 32u;
    return Slot(base, base + 1u, base + 2u, retained, touched, unsupported,
        false_roots, queued, counts);
}

// A head's count word and bit shift. Counts never carry or borrow into a
// neighbour: every intermediate value is a sum over the head's producers of
// their old or new enabled state, within [0, producers(head)], which the
// host's width admits.
fn count_location(slot: Slot, head: u32) -> vec2<u32> {
    let per_word = 32u / params.count_bits;
    return vec2<u32>(slot.counts + head / per_word, (head % per_word) * params.count_bits);
}
fn support_count(slot: Slot, head: u32) -> u32 {
    let location = count_location(slot, head);
    let word = atomicLoad(&state[location.x]);
    if (params.count_bits == 32u) { return word; }
    return (word >> location.y) & ((1u << params.count_bits) - 1u);
}
fn adjust_count(slot: Slot, head: u32, holds: bool) {
    let location = count_location(slot, head);
    let one = 1u << location.y;
    if (holds) { atomicAdd(&state[location.x], one); } else { atomicSub(&state[location.x], one); }
}
fn enqueue(slot: Slot, node: u32) {
    let position = nodes[node * 4u + 3u];
    atomicOr(&state[slot.queued + position / 32u], 1u << (position % 32u));
}

// A node or guard changed truth: queue the connectives and row owners reading
// it, adjust its producers' heads' counts (touching those heads), and flip its
// root positions' bits. Root positions share words, so each flip is atomic.
fn visit_readers(slot: Slot, node: u32, holds: bool) {
    let start = producers[params.readers + node];
    let end = producers[params.readers + node + 1u];
    atomicAdd(&performed, end - start);
    let entries = params.readers + params.nodes + 1u;
    for (var index = start; index < end; index++) {
        let entry = producers[entries + index];
        let kind = entry >> 30u;
        let subject = entry & 0x3fffffffu;
        if (kind == READER_NODE) {
            enqueue(slot, subject);
        } else if (kind == READER_PRODUCER) {
            adjust_count(slot, subject, holds);
            atomicOr(&state[slot.touched + subject / 32u], 1u << (subject % 32u));
        } else {
            atomicXor(&state[slot.false_roots + subject / 32u], 1u << (subject % 32u));
            if (holds) {
                atomicSub(&state[slot.false_count], 1u);
            } else {
                atomicAdd(&state[slot.false_count], 1u);
            }
        }
    }
}

// Only a row's owner is ever queued: a condition's row reader names the owner,
// which recomputes the row's measure and evaluates every live guard of the row.
fn update_row(values: u32, slot: Slot, index: u32, node: Node) {
    let row = numeric_row(node);
    if (nodes[row - 3u] != index) { return; }
    let count = nodes[row - 2u];
    let width = nodes[row - 1u];
    let guards = row + (1u + min(width, 2u)) * node.right;
    atomicAdd(&performed, 1u + node.right + count);
    let measure = numeric_measure(values, node, width);
    for (var position = 0u; position < count; position++) {
        let guard = nodes[guards + position];
        let holds = numeric_guard(node_at(guard), width, measure);
        let next = select(0u, 1u, holds);
        if (next != (frozen[values + guard] & 1u)) {
            frozen[values + guard] = next;
            visit_readers(slot, guard, holds);
        }
    }
}

// Re-evaluate one queued node from final lower-level truth.
fn update_node(world: u32, values: u32, slot: Slot, index: u32) {
    atomicAdd(&evaluated, 1u);
    let node = node_at(index);
    if (node.tag >= 7u && node.tag <= 9u) {
        update_row(values, slot, index, node);
        return;
    }
    var reads = 0u;
    if (node.tag >= NODE_AND && node.tag <= NODE_IMPLIES) { reads = 2u; }
    if (node.tag == NODE_ALL || node.tag == NODE_ANY) { reads = node.right; }
    atomicAdd(&performed, 1u + reads);
    let holds = boolean_original(world, values, node);
    let next = select(0u, 1u, holds);
    if (next != (frozen[values + index] & 1u)) {
        frozen[values + index] = next;
        visit_readers(slot, index, holds);
    }
}

// The bits of `word` at schedule positions [start, end).
fn level_mask(word: u32, start: u32, end: u32) -> u32 {
    let first = word * 32u;
    let low = max(start, first) - first;
    let high = min(end, first + 32u) - first;
    if (high <= low) { return 0u; }
    let upper = select((1u << high) - 1u, 0xffffffffu, high == 32u);
    return upper & ~((1u << low) - 1u);
}

// Derive the support counts, unsupported set and false-root set of the
// retained evaluation, once after each complete or dense evaluation of a slot.
fn summarize(lane: u32, values: u32, slot: Slot) {
    let per_word = 32u / params.count_bits;
    let count_words = (params.atoms + per_word - 1u) / per_word;
    let root_words = (params.roots + 31u) / 32u;
    for (var word = lane; word < count_words; word += WORKGROUP_SIZE) {
        atomicStore(&state[slot.counts + word], 0u);
    }
    for (var word = lane; word < params.words; word += WORKGROUP_SIZE) {
        atomicStore(&state[slot.unsupported + word], 0u);
    }
    for (var word = lane; word < root_words; word += WORKGROUP_SIZE) {
        atomicStore(&state[slot.false_roots + word], 0u);
    }
    if (lane == 0u) {
        atomicStore(&state[slot.false_count], 0u);
        atomicStore(&state[slot.unsupported_count], 0u);
        atomicAdd(&performed,
            count_words + 2u * params.words + root_words + params.producers + params.roots);
    }
    storageBarrier();
    workgroupBarrier();
    for (var index = lane; index < params.producers; index += WORKGROUP_SIZE) {
        let producer = producer_at(index);
        if (enabled(producer, values)) { adjust_count(slot, producer.head, true); }
    }
    for (var position = lane; position < params.roots; position += WORKGROUP_SIZE) {
        if ((frozen[values + roots[position]] & 1u) == 0u) {
            atomicOr(&state[slot.false_roots + position / 32u], 1u << (position % 32u));
            atomicAdd(&state[slot.false_count], 1u);
        }
    }
    storageBarrier();
    workgroupBarrier();
    for (var word = lane; word < params.words; word += WORKGROUP_SIZE) {
        let present = atomicLoad(&state[slot.retained + word]);
        var missing = 0u;
        var remaining = present;
        while (remaining != 0u) {
            let bit = firstTrailingBit(remaining);
            remaining &= remaining - 1u;
            if (support_count(slot, word * 32u + bit) == 0u) { missing |= 1u << bit; }
        }
        atomicStore(&state[slot.unsupported + word], missing);
        atomicAdd(&state[slot.unsupported_count], countOneBits(missing));
        atomicAdd(&performed, countOneBits(present));
    }
    if (lane == 0u) { atomicStore(&state[slot.summarized], 1u); }
    storageBarrier();
    workgroupBarrier();
}

// Bring the slot's retained evaluation to the new candidate. Every phase ends
// with both barriers in uniform control flow; loops partition index ranges among
// invocations, so no correctness argument depends on the workgroup size.
fn update_slot(world: u32, lane: u32, values: u32, current: u32, slot: Slot) {
    if (lane == 0u) { summary_flag = atomicLoad(&state[slot.summarized]); }
    if (workgroupUniformLoad(&summary_flag) == 0u) {
        summarize(lane, values, slot);
    }
    // Presence: touch every changed atom, queue its atom nodes and retain the
    // new candidate's word. One owner per word.
    for (var word = lane; word < params.words; word += WORKGROUP_SIZE) {
        let next = candidates[current + word];
        var changed = atomicLoad(&state[slot.retained + word]) ^ next;
        if (changed != 0u) {
            atomicOr(&state[slot.touched + word], changed);
            atomicAdd(&performed, countOneBits(changed));
            let entries = params.atom_nodes + params.atoms + 1u;
            while (changed != 0u) {
                let atom = word * 32u + firstTrailingBit(changed);
                changed &= changed - 1u;
                let start = producers[params.atom_nodes + atom];
                let end = producers[params.atom_nodes + atom + 1u];
                atomicAdd(&performed, end - start);
                for (var index = start; index < end; index++) {
                    enqueue(slot, producers[entries + index]);
                }
            }
            atomicStore(&state[slot.retained + word], next);
        }
    }
    storageBarrier();
    workgroupBarrier();
    // Propagation, level by level. Every reader has a larger level than what it
    // reads, so a level's queued bits are all set before its round, and no visit
    // during the round sets one. The masked take collects exactly the level's
    // bits and clears only them: positions are contiguous across levels without
    // word alignment, so one word can hold bits of several levels, including
    // bits other invocations' visits are setting in this same round.
    let offsets = params.roots;
    let order = offsets + params.levels + 1u;
    for (var level = 0u; level < params.levels; level++) {
        let start = roots[offsets + level];
        let end = roots[offsets + level + 1u];
        for (var word = start / 32u + lane; word < (end + 31u) / 32u; word += WORKGROUP_SIZE) {
            let mask = level_mask(word, start, end);
            if ((atomicLoad(&state[slot.queued + word]) & mask) != 0u) {
                var taken = atomicAnd(&state[slot.queued + word], ~mask) & mask;
                while (taken != 0u) {
                    let position = word * 32u + firstTrailingBit(taken);
                    taken &= taken - 1u;
                    update_node(world, values, slot, roots[order + position]);
                }
            }
        }
        storageBarrier();
        workgroupBarrier();
    }
    // Settle presence and support once every count is final: recompute each
    // touched present atom's unsupported bit. One owner per word.
    for (var word = lane; word < params.words; word += WORKGROUP_SIZE) {
        let touched = atomicExchange(&state[slot.touched + word], 0u);
        if (touched != 0u) {
            atomicAdd(&performed, countOneBits(touched));
            let old = atomicLoad(&state[slot.unsupported + word]);
            var missing = old & ~touched;
            var remaining = touched & candidates[current + word];
            while (remaining != 0u) {
                let bit = firstTrailingBit(remaining);
                remaining &= remaining - 1u;
                if (support_count(slot, word * 32u + bit) == 0u) { missing |= 1u << bit; }
            }
            atomicStore(&state[slot.unsupported + word], missing);
            let before = countOneBits(old);
            let after = countOneBits(missing);
            if (after > before) { atomicAdd(&state[slot.unsupported_count], after - before); }
            if (before > after) { atomicSub(&state[slot.unsupported_count], before - after); }
        }
    }
    storageBarrier();
    workgroupBarrier();
    // The verdict's witnesses are the least elements of the sets the dense
    // scans search: false roots first, then unsupported present atoms.
    if (lane == 0u) {
        verdict_counts = vec2<u32>(atomicLoad(&state[slot.false_count]),
            atomicLoad(&state[slot.unsupported_count]));
    }
    let counts = workgroupUniformLoad(&verdict_counts);
    if (counts.x != 0u) {
        let root_words = (params.roots + 31u) / 32u;
        for (var word = lane; word < root_words; word += WORKGROUP_SIZE) {
            let bits = atomicLoad(&state[slot.false_roots + word]);
            if (bits != 0u) { atomicMin(&first_root, word * 32u + firstTrailingBit(bits)); }
        }
        if (lane == 0u) { atomicAdd(&performed, root_words); }
    } else if (counts.y != 0u) {
        for (var word = lane; word < params.words; word += WORKGROUP_SIZE) {
            let bits = atomicLoad(&state[slot.unsupported + word]);
            if (bits != 0u) { atomicMin(&first_atom, word * 32u + firstTrailingBit(bits)); }
        }
        if (lane == 0u) { atomicAdd(&performed, params.words); }
    }
}

// The complete route, or dense reuse of a retained evaluation.
fn evaluate_dense(world: u32, lane: u32, grouped: bool, values: u32, supported: u32,
    reuse: bool) {
    if (params.levels == 0u) {
        if (lane == 0u) {
            for (var index = 0u; index < params.nodes; index++) {
                original(world, values, index, reuse);
            }
        }
    } else {
        let offsets = params.roots;
        let order = offsets + params.levels + 1u;
        for (var level = 0u; level < params.levels; level++) {
            let end = roots[offsets + level + 1u];
            for (var position = roots[offsets + level] + lane; position < end; position += WORKGROUP_SIZE) {
                original(world, values, roots[order + position], reuse);
            }
            storageBarrier();
            workgroupBarrier();
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
        if ((frozen[values + roots[ordinal]] & 1u) == 0u) {
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
        let missing = candidates[supported + word] & ~atomicLoad(&support[supported + word]);
        if (missing != 0u) {
            atomicMin(&first_atom, word * 32u + firstTrailingBit(missing));
        }
    }
}

// With the update route allowed, a dense evaluation retains the candidate it
// evaluated and requires a new summary; the slot's record then says it is
// seeded. Without the route no evaluation touches the state, and the record
// says the slot is not seeded.
fn retain_for_update(lane: u32, supported: u32, slot: Slot) {
    if (params.allowed != 0u) {
        for (var word = lane; word < params.words; word += WORKGROUP_SIZE) {
            atomicStore(&state[slot.retained + word], candidates[supported + word]);
        }
        if (lane == 0u) { atomicStore(&state[slot.summarized], 0u); }
    }
}

// Invocation 0's record of slot `world`, after the final barrier: epoch, slot,
// the verdict from the least false root and unsupported atom, work, marker,
// route, nodes evaluated, and whether the slot's update state was seeded.
fn store_record(world: u32, marker: u32, route: u32, work: u32, nodes_evaluated: u32,
    seeded: u32) {
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
    results[base + 4u] = work;
    // The same branch selector determines support construction and its
    // receipt. The host refuses a record from the other physical policy.
    results[base + 5u] = marker;
    results[base + 6u] = route;
    results[base + 7u] = nodes_evaluated;
    results[base + 8u] = seeded;
}

fn check_support(world: u32, lane: u32, grouped: bool) {
    let values = world * params.nodes;
    let supported = world * params.words;
    let prior = world * RESULT_WORDS;
    let marker = select(RESULT_MAGIC, RESULT_GROUPED_MAGIC, grouped);
    let slot = slot_at(world);
    // The host either retained explicit successful ownership or cleared these
    // hints in this same encoder. Epoch/slot/marker do not authorize reuse alone.
    // Every invocation reads the same record, which only invocation 0 stores,
    // after the barriers of every route.
    let reuse = params.epoch > 1u && results[prior] == params.epoch - 1u &&
        results[prior + 1u] == world && results[prior + 5u] == marker;
    if (lane == 0u) {
        atomicStore(&first_root, params.roots);
        atomicStore(&first_atom, params.atoms);
    }
    var route = select(ROUTE_COMPLETE, ROUTE_DENSE, reuse);
    if (params.allowed == 0u) {
        // Without the update route, choosing complete or dense evaluation steers
        // no barrier, so every invocation takes the choice from the record
        // itself; the dense routes' barriers order the stores above before any
        // other invocation's atomic.
        evaluate_dense(world, lane, grouped, values, supported, reuse);
    } else {
        if (lane == 0u) {
            atomicStore(&changed_atoms, 0u);
            atomicStore(&performed, 0u);
            atomicStore(&evaluated, 0u);
            // The previous record says whether that dispatch seeded the state.
            entry_route = select(route, ROUTE_UPDATE, reuse && results[prior + 8u] != 0u);
        }
        // The routes synchronize differently, so the choice is broadcast.
        var chosen = workgroupUniformLoad(&entry_route);
        if (chosen == ROUTE_UPDATE) {
            // Count changed atoms; more than the threshold take dense reuse.
            for (var word = lane; word < params.words; word += WORKGROUP_SIZE) {
                let changed = atomicLoad(&state[slot.retained + word]) ^ candidates[supported + word];
                if (changed != 0u) { atomicAdd(&changed_atoms, countOneBits(changed)); }
            }
            workgroupBarrier();
            if (lane == 0u) {
                chosen_route = select(ROUTE_BEYOND_THRESHOLD, ROUTE_UPDATE,
                    atomicLoad(&changed_atoms) <= params.threshold);
            }
            chosen = workgroupUniformLoad(&chosen_route);
        }
        if (chosen == ROUTE_UPDATE) {
            update_slot(world, lane, values, supported, slot);
        } else {
            evaluate_dense(world, lane, grouped, values, supported, chosen != ROUTE_COMPLETE);
            retain_for_update(lane, supported, slot);
        }
        route = chosen;
    }
    workgroupBarrier();
    if (lane == 0u) {
        var work = params.work;
        var nodes_evaluated = 0u;
        // Dense reuse after a change count also charges the count.
        if (route == ROUTE_BEYOND_THRESHOLD) { work += 2u * params.words; }
        if (route == ROUTE_UPDATE) {
            // The floor: the change count and presence pass over both
            // candidates' words, the settling scan, the queued-set scans and
            // this record; then what the phases counted.
            work = 5u * params.words + (params.nodes + 31u) / 32u + params.levels +
                RESULT_WORDS + atomicLoad(&performed);
            nodes_evaluated = atomicLoad(&evaluated);
        }
        store_record(world, marker, route, work, nodes_evaluated, params.allowed);
    }
}

// The kernel of a theory whose update route is declined for its residency:
// complete evaluation, or dense reuse under the record's authority, exactly as
// the full kernel runs with the route not allowed, and a record saying the
// slot is not seeded. It names no update state and no update counter.
fn check_support_dense(world: u32, lane: u32, grouped: bool) {
    let values = world * params.nodes;
    let supported = world * params.words;
    let prior = world * RESULT_WORDS;
    let marker = select(RESULT_MAGIC, RESULT_GROUPED_MAGIC, grouped);
    // The same authority as the full kernel's: the host either retained
    // explicit successful ownership or cleared these hints in this encoder.
    let reuse = params.epoch > 1u && results[prior] == params.epoch - 1u &&
        results[prior + 1u] == world && results[prior + 5u] == marker;
    if (lane == 0u) {
        atomicStore(&first_root, params.roots);
        atomicStore(&first_atom, params.atoms);
    }
    // The dense routes' barriers order the stores above before any other
    // invocation's atomic.
    evaluate_dense(world, lane, grouped, values, supported, reuse);
    workgroupBarrier();
    if (lane == 0u) {
        store_record(world, marker, select(ROUTE_COMPLETE, ROUTE_DENSE, reuse), params.work, 0u, 0u);
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

@compute @workgroup_size(WORKGROUP_SIZE)
fn check_dense(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    check_support_dense(group.x, lane, false);
}

@compute @workgroup_size(WORKGROUP_SIZE)
fn check_grouped_dense(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    check_support_dense(group.x, lane, true);
}
