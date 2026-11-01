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
// makes unobservable; CONTRIBUTING ("Carry the semantic contract into Lean and
// Rust") records the fill's status and what removing it requires.
// Each support policy has two entry points, and which one a batch runs follows
// whether its update route is allowed. `check` and `check_grouped` run a batch
// with the route allowed: each slot updates, or evaluates completely or densely
// and retains its candidate for its next update. `check_dense` and
// `check_grouped_dense` run every other batch, complete or dense truth only: a
// theory whose route is declined for its whole residency, and a batch whose
// route a limit or the runtime guard declines. Their call graph reads no update
// state, so a pipeline compiled from them binds no state (binding 8) and
// allocates none of the update's workgroup variables. Both kinds write the same
// records save the seeded bit: the full kind records every slot seeded, and
// the dense kind none.
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
// A record's word 6 holds the route in bits 0-1 and whether the slot's update
// state was seeded in this bit.
const SEEDED: u32 = 4u;
const READER_NODE: u32 = 0u;
const READER_PRODUCER: u32 = 1u;
const SUMMARY_WORDS: u32 = 3u;
// Words 9 and 15 are reserved. Whether a batch's update route is allowed is
// the entry point it runs, not a parameter.
struct Params {
    atoms: u32, nodes: u32, roots: u32, producers: u32,
    words: u32, levels: u32, work: u32, epoch: u32,
    threshold: u32, reserved_9: u32, readers: u32, atom_nodes: u32,
    count_bits: u32, state_words: u32, floor_path: u32, reserved_15: u32,
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
// One record per slot: epoch, slot, status, witness, work, marker, the route
// with whether the slot's update state was seeded (which every route writes and
// the slot's next dispatch reads beside its authority), nodes evaluated, and an
// update's path.
@group(0) @binding(7) var<storage, read_write> results: array<u32>;
// Per-slot update state of `params.state_words` words: the summary block
// (summarized, false-root count, unsupported count), the retained candidate,
// the touched and unsupported atom sets, the false-root set over root
// positions, the queued set over schedule positions, and packed support counts
// of `params.count_bits` bits per head. The full entry points run only with
// the update route allowed, and bind it; the dense entry points never name it,
// so their pipelines do not bind it.
@group(0) @binding(8) var<storage, read_write> state: array<atomic<u32>>;
// The least false root and unsupported atom: invocation 0 stores their bounds
// at entry; in the dense entry points the dense routes' barriers, and in the
// full ones the uniform load of entry_route, order those stores before any
// other invocation's atomicMin, and invocation 0 reads both after the final
// barrier.
var<workgroup> first_root: atomic<u32>;
var<workgroup> first_atom: atomic<u32>;
// Update-route counters, which the dense entry points never name: invocation 0
// zeroes them before storing entry_route, whose uniform load orders the zeroes
// before any other invocation's atomicAdd. Invocation 0 reads changed_atoms
// after the barrier that ends the change count, and performed and evaluated,
// on the update route only, after the final barrier.
var<workgroup> changed_atoms: atomic<u32>;
var<workgroup> performed: atomic<u32>;
var<workgroup> evaluated: atomic<u32>;
// The update path: the most an invocation's operations cost, by the guard's
// table, in each phase of an update after its summary, and in each phase of the
// summary when the slot's previous dispatch took dense reuse beyond the
// threshold, summed. Write: invocation 0 zeroes both with the counters above,
// before storing entry_route, whose uniform load orders the zeroes before any
// other invocation's atomicMax. Ordering: at a phase's end each invocation
// with a nonzero tally takes the maximum into phase_path[p & 1] before the
// phase's barrier; after it invocation 0 reads that variable and zeroes it
// again, and the next atomicMax into it follows a later barrier. Every phase
// of an update ends this way, the summary's included. Index: p & 1, within
// the array.
var<workgroup> phase_path: array<atomic<u32>, 2>;
// Values steering barriered control flow, written by invocation 0 and read by
// every invocation through workgroupUniformLoad, one variable per decision.
// chosen_route is written after the barrier that ends the change count, and
// verdict_counts is stored whole; its components are read from the loaded
// value.
var<workgroup> entry_route: u32;
var<workgroup> chosen_route: u32;
var<workgroup> summary_flag: u32;
var<workgroup> verdict_counts: vec2<u32>;

fn contains(world: u32, atom: u32) -> bool {
    return (candidates[world * params.words + atom / 32u] & (1u << (atom % 32u))) != 0u;
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
    if (node.tag == NODE_ALL || node.tag == NODE_ANY) { return reduction(values, node).x != 0u; }
    return node.tag == 10u;
}

// A wide conjunction's or disjunction's value, and the operands it read before
// an absorbing value ended it.
fn reduction(values: u32, node: Node) -> vec2<u32> {
    let identity = node.tag == NODE_ALL;
    for (var child = 0u; child < node.right; child++) {
        let next = (frozen[values + operand_at(node, child)] & 1u) != 0u;
        if (next != identity) { return vec2<u32>(select(1u, 0u, identity), child + 1u); }
    }
    return vec2<u32>(select(0u, 1u, identity), node.right);
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
// Filter selected atoms by their final packed counts, folding into retained bits.
// Both callers follow count publication; only private scalar state changes here.
fn accumulate_zero_counts(slot: Slot, word: u32, selected: u32, retained: u32) -> u32 {
    var missing = retained;
    var remaining = selected;
    while (remaining != 0u) {
        let bit = firstTrailingBit(remaining);
        remaining &= remaining - 1u;
        if (support_count(slot, word * 32u + bit) == 0u) { missing |= 1u << bit; }
    }
    return missing;
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
// Returns the operations: one per readers entry.
fn visit_readers(slot: Slot, node: u32, holds: bool) -> u32 {
    let start = producers[params.readers + node];
    let end = producers[params.readers + node + 1u];
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
    return end - start;
}

// Only a row's owner is ever queued: a condition's row reader names the owner,
// which recomputes the row's measure and evaluates every live guard of the row.
// Returns the operations: the row's reads and guards, and the readers visits
// of the guards that changed.
fn update_row(values: u32, slot: Slot, index: u32, node: Node) -> u32 {
    let row = numeric_row(node);
    if (nodes[row - 3u] != index) { return 0u; }
    let count = nodes[row - 2u];
    let width = nodes[row - 1u];
    let guards = row + (1u + min(width, 2u)) * node.right;
    var operations = 1u + node.right + count;
    let measure = numeric_measure(values, node, width);
    for (var position = 0u; position < count; position++) {
        let guard = nodes[guards + position];
        let holds = numeric_guard(node_at(guard), width, measure);
        let next = select(0u, 1u, holds);
        if (next != (frozen[values + guard] & 1u)) {
            frozen[values + guard] = next;
            operations += visit_readers(slot, guard, holds);
        }
    }
    return operations;
}

// Re-evaluate one queued node from final lower-level truth. Returns the
// operations, the node and its reads with its readers visits if it changed,
// and their cost by the guard's table, which counts the operands a wide gate
// read rather than all of them.
fn update_node(world: u32, values: u32, slot: Slot, index: u32) -> vec2<u32> {
    let node = node_at(index);
    if (node.tag >= 7u && node.tag <= 9u) {
        let operations = update_row(values, slot, index, node);
        return vec2<u32>(operations, operations);
    }
    var value = 0u;
    var reads = 0u;
    var read = 0u;
    if (node.tag == NODE_ALL || node.tag == NODE_ANY) {
        let reduced = reduction(values, node);
        value = reduced.x;
        reads = node.right;
        read = reduced.y;
    } else {
        value = select(0u, 1u, boolean_original(world, values, node));
        if (node.tag >= NODE_AND && node.tag <= NODE_IMPLIES) {
            reads = 2u;
            read = 2u;
        }
    }
    var visits = 0u;
    if (value != (frozen[values + index] & 1u)) {
        frozen[values + index] = value;
        visits = visit_readers(slot, index, value != 0u);
    }
    return vec2<u32>(1u + reads + visits, 1u + read + visits);
}

// End one phase of an update: offer this invocation's tally to the phase's
// maximum before the phase's barrier; after it, invocation 0 adds the maximum
// to its path and zeroes the variable for the phase after next. Called in
// uniform control flow.
fn end_phase(lane: u32, tally: u32, phase: ptr<function, u32>, path: ptr<function, u32>) {
    let index = *phase & 1u;
    if (tally != 0u) { atomicMax(&phase_path[index], tally); }
    storageBarrier();
    workgroupBarrier();
    if (lane == 0u) {
        *path += atomicLoad(&phase_path[index]);
        atomicStore(&phase_path[index], 0u);
    }
    *phase += 1u;
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
// Its barriers end three phases: the clears, the producer and root visits, and
// the retained-word scan with one count read per present atom. Each counts this
// invocation's own iterations into the work; returns their sum. When `pathed`,
// each phase also offers its tally to the update's path (see `end_phase`): a
// summary after the slot took dense reuse beyond the threshold recurs with the
// change stream, so the runtime guard weighs it. A summary after a batch that
// seeded every slot, the guard's probe included, is a cost once per seeding and
// offers nothing. Called in uniform control flow.
fn summarize(lane: u32, values: u32, slot: Slot, pathed: bool,
    phase: ptr<function, u32>, path: ptr<function, u32>) -> u32 {
    let per_word = 32u / params.count_bits;
    let count_words = (params.atoms + per_word - 1u) / per_word;
    let root_words = (params.roots + 31u) / 32u;
    var tally = 0u;
    for (var word = lane; word < count_words; word += WORKGROUP_SIZE) {
        atomicStore(&state[slot.counts + word], 0u);
        tally += 1u;
    }
    for (var word = lane; word < params.words; word += WORKGROUP_SIZE) {
        atomicStore(&state[slot.unsupported + word], 0u);
        tally += 1u;
    }
    for (var word = lane; word < root_words; word += WORKGROUP_SIZE) {
        atomicStore(&state[slot.false_roots + word], 0u);
        tally += 1u;
    }
    if (lane == 0u) {
        atomicStore(&state[slot.false_count], 0u);
        atomicStore(&state[slot.unsupported_count], 0u);
    }
    var operations = tally;
    end_phase(lane, select(0u, tally, pathed), phase, path);
    tally = 0u;
    for (var index = lane; index < params.producers; index += WORKGROUP_SIZE) {
        let producer = producer_at(index);
        if (enabled(producer, values)) { adjust_count(slot, producer.head, true); }
        tally += 1u;
    }
    for (var position = lane; position < params.roots; position += WORKGROUP_SIZE) {
        if ((frozen[values + roots[position]] & 1u) == 0u) {
            atomicOr(&state[slot.false_roots + position / 32u], 1u << (position % 32u));
            atomicAdd(&state[slot.false_count], 1u);
        }
        tally += 1u;
    }
    operations += tally;
    end_phase(lane, select(0u, tally, pathed), phase, path);
    tally = 0u;
    for (var word = lane; word < params.words; word += WORKGROUP_SIZE) {
        let present = atomicLoad(&state[slot.retained + word]);
        let missing = accumulate_zero_counts(slot, word, present, 0u);
        atomicStore(&state[slot.unsupported + word], missing);
        atomicAdd(&state[slot.unsupported_count], countOneBits(missing));
        tally += 1u + countOneBits(present);
    }
    if (lane == 0u) { atomicStore(&state[slot.summarized], 1u); }
    end_phase(lane, select(0u, tally, pathed), phase, path);
    return operations + tally;
}

// Bring the slot's retained evaluation to the new candidate. Every phase ends
// with both barriers in uniform control flow; loops partition index ranges among
// invocations, so no correctness argument depends on the workgroup size. Each
// phase after the summary tallies this invocation's cost by the guard's table
// and offers the tally to the phase's maximum (see `phase_path`), and so does
// each phase of a summary when `after_beyond`, the slot's previous dispatch
// having taken dense reuse beyond the threshold; the verdict scan's phase ends
// at the record's barrier. Returns this invocation's operations over every
// phase.
fn update_slot(world: u32, lane: u32, values: u32, current: u32, slot: Slot,
    after_beyond: bool, phase: ptr<function, u32>, path: ptr<function, u32>) -> u32 {
    var operations = 0u;
    if (lane == 0u) { summary_flag = atomicLoad(&state[slot.summarized]); }
    if (workgroupUniformLoad(&summary_flag) == 0u) {
        operations += summarize(lane, values, slot, after_beyond, phase, path);
    }
    // Presence: touch every changed atom, queue its atom nodes and retain the
    // new candidate's word. One owner per word.
    var tally = 0u;
    for (var word = lane; word < params.words; word += WORKGROUP_SIZE) {
        let next = candidates[current + word];
        var changed = atomicLoad(&state[slot.retained + word]) ^ next;
        if (changed != 0u) {
            atomicOr(&state[slot.touched + word], changed);
            tally += countOneBits(changed);
            let entries = params.atom_nodes + params.atoms + 1u;
            while (changed != 0u) {
                let atom = word * 32u + firstTrailingBit(changed);
                changed &= changed - 1u;
                let start = producers[params.atom_nodes + atom];
                let end = producers[params.atom_nodes + atom + 1u];
                tally += end - start;
                for (var index = start; index < end; index++) {
                    enqueue(slot, producers[entries + index]);
                }
            }
            atomicStore(&state[slot.retained + word], next);
        }
    }
    operations += tally;
    end_phase(lane, tally, phase, path);
    // Propagation, level by level. Every reader has a larger level than what it
    // reads, so a level's queued bits are all set before its round, and no visit
    // during the round sets one. The masked take collects exactly the level's
    // bits and clears only them: positions are contiguous across levels without
    // word alignment, so one word can hold bits of several levels, including
    // bits other invocations' visits are setting in this same round.
    let offsets = params.roots;
    let order = offsets + params.levels + 1u;
    var taken_nodes = 0u;
    for (var level = 0u; level < params.levels; level++) {
        let start = roots[offsets + level];
        let end = roots[offsets + level + 1u];
        tally = 0u;
        var cost = 0u;
        for (var word = start / 32u + lane; word < (end + 31u) / 32u; word += WORKGROUP_SIZE) {
            let mask = level_mask(word, start, end);
            if ((atomicLoad(&state[slot.queued + word]) & mask) != 0u) {
                var taken = atomicAnd(&state[slot.queued + word], ~mask) & mask;
                taken_nodes += countOneBits(taken);
                while (taken != 0u) {
                    let position = word * 32u + firstTrailingBit(taken);
                    taken &= taken - 1u;
                    let evaluation = update_node(world, values, slot, roots[order + position]);
                    tally += evaluation.x;
                    cost += evaluation.y;
                }
            }
        }
        operations += tally;
        end_phase(lane, cost, phase, path);
    }
    if (taken_nodes != 0u) { atomicAdd(&evaluated, taken_nodes); }
    // Settle presence and support once every count is final: recompute each
    // touched present atom's unsupported bit. One owner per word.
    tally = 0u;
    for (var word = lane; word < params.words; word += WORKGROUP_SIZE) {
        let touched = atomicExchange(&state[slot.touched + word], 0u);
        if (touched != 0u) {
            tally += countOneBits(touched);
            let old = atomicLoad(&state[slot.unsupported + word]);
            let retained = old & ~touched;
            let selected = touched & candidates[current + word];
            let missing = accumulate_zero_counts(slot, word, selected, retained);
            atomicStore(&state[slot.unsupported + word], missing);
            let before = countOneBits(old);
            let after = countOneBits(missing);
            if (after > before) { atomicAdd(&state[slot.unsupported_count], after - before); }
            if (before > after) { atomicSub(&state[slot.unsupported_count], before - after); }
        }
    }
    operations += tally;
    end_phase(lane, tally, phase, path);
    // The verdict's witnesses are the least elements of the sets the dense
    // scans search: false roots first, then unsupported present atoms. Each
    // invocation counts its own iterations of the scan.
    if (lane == 0u) {
        verdict_counts = vec2<u32>(atomicLoad(&state[slot.false_count]),
            atomicLoad(&state[slot.unsupported_count]));
    }
    let counts = workgroupUniformLoad(&verdict_counts);
    tally = 0u;
    if (counts.x != 0u) {
        let root_words = (params.roots + 31u) / 32u;
        for (var word = lane; word < root_words; word += WORKGROUP_SIZE) {
            let bits = atomicLoad(&state[slot.false_roots + word]);
            if (bits != 0u) { atomicMin(&first_root, word * 32u + firstTrailingBit(bits)); }
            tally += 1u;
        }
    } else if (counts.y != 0u) {
        for (var word = lane; word < params.words; word += WORKGROUP_SIZE) {
            let bits = atomicLoad(&state[slot.unsupported + word]);
            if (bits != 0u) { atomicMin(&first_atom, word * 32u + firstTrailingBit(bits)); }
            tally += 1u;
        }
    }
    // The record's barrier ends this phase; invocation 0 takes its maximum then.
    if (tally != 0u) { atomicMax(&phase_path[*phase & 1u], tally); }
    return operations + tally;
}

// The complete route, or dense reuse of a retained evaluation.
fn evaluate_dense(world: u32, lane: u32, grouped: bool, values: u32, supported: u32,
    reuse: bool) {
    evaluate_original(world, lane, values, reuse);

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

// Under the update route, a complete or dense evaluation retains the candidate
// it evaluated and requires a new summary; the slot's record then says it is
// seeded. The dense entry points never touch the state, and their records say
// the slot is not seeded.
fn retain_for_update(lane: u32, supported: u32, slot: Slot) {
    for (var word = lane; word < params.words; word += WORKGROUP_SIZE) {
        atomicStore(&state[slot.retained + word], candidates[supported + word]);
    }
    if (lane == 0u) { atomicStore(&state[slot.summarized], 0u); }
}

// Invocation 0's record of slot `world`, after the final barrier: epoch, slot,
// the verdict from the least false root and unsupported atom, work, marker,
// the route with whether the slot's update state was seeded, nodes evaluated,
// and an update's path, zero for every other route.
fn store_record(world: u32, marker: u32, route: u32, work: u32, nodes_evaluated: u32,
    seeded: bool, path: u32) {
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
    results[base + 6u] = route | select(0u, SEEDED, seeded);
    results[base + 7u] = nodes_evaluated;
    results[base + 8u] = path;
}

// The kernel of a batch whose update route is allowed: every slot updates,
// takes dense reuse beyond the threshold after counting its change, or, without
// a seeded record, evaluates completely or densely and seeds its state.
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
    // Whether that dispatch took dense reuse beyond the threshold, which cleared
    // the slot's summary: an update now summarizes, and its path counts it.
    let after_beyond = (results[prior + 6u] & 3u) == ROUTE_BEYOND_THRESHOLD;
    if (lane == 0u) {
        atomicStore(&first_root, params.roots);
        atomicStore(&first_atom, params.atoms);
        atomicStore(&changed_atoms, 0u);
        atomicStore(&performed, 0u);
        atomicStore(&evaluated, 0u);
        atomicStore(&phase_path[0], 0u);
        atomicStore(&phase_path[1], 0u);
        // The previous record says whether that dispatch seeded the state.
        entry_route = select(select(ROUTE_COMPLETE, ROUTE_DENSE, reuse), ROUTE_UPDATE,
            reuse && (results[prior + 6u] & SEEDED) != 0u);
    }
    // An update's phase count and, in invocation 0, its path's phase maxima.
    var phase = 0u;
    var path = 0u;
    // The routes synchronize differently, so the choice is broadcast.
    var route = workgroupUniformLoad(&entry_route);
    if (route == ROUTE_UPDATE) {
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
        route = workgroupUniformLoad(&chosen_route);
    }
    if (route == ROUTE_UPDATE) {
        let operations = update_slot(world, lane, values, supported, slot, after_beyond,
            &phase, &path);
        if (operations != 0u) { atomicAdd(&performed, operations); }
    } else {
        evaluate_dense(world, lane, grouped, values, supported, route != ROUTE_COMPLETE);
        retain_for_update(lane, supported, slot);
    }
    workgroupBarrier();
    if (lane == 0u) {
        var work = params.work;
        var nodes_evaluated = 0u;
        var update_path = 0u;
        // Dense reuse after a change count also charges the count.
        if (route == ROUTE_BEYOND_THRESHOLD) { work += 2u * params.words; }
        if (route == ROUTE_UPDATE) {
            // The floor: the change count and presence pass over both
            // candidates' words, the settling scan, the queued-set scans and
            // this record; then what the phases counted.
            work = 5u * params.words + (params.nodes + 31u) / 32u + params.levels +
                RESULT_WORDS + atomicLoad(&performed);
            nodes_evaluated = atomicLoad(&evaluated);
            // The verdict scan's phase ended at the barrier above. The host's
            // floor_path is each invocation's share of the uniform scans and
            // of this record.
            update_path = params.floor_path + path + atomicLoad(&phase_path[phase & 1u]);
        }
        store_record(world, marker, route, work, nodes_evaluated, true, update_path);
    }
}

// The kernel of every batch whose update route is not allowed: a theory whose
// route is declined for its residency, and a batch whose route a limit or the
// runtime guard declines. Complete evaluation, or dense reuse under the
// record's authority, and a record saying the slot is not seeded. It names no
// update state and no update counter.
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
        store_record(world, marker, select(ROUTE_COMPLETE, ROUTE_DENSE, reuse), params.work, 0u,
            false, 0u);
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
