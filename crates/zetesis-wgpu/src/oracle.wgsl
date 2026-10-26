// One workgroup owns one candidate's complete closure. The host validates
// every index and buffer length before dispatch. Rules and frozen seeds never
// change during an epoch. This is the static profile: no source grounding is
// performed in this shader.
// Every workgroup location an invocation reads was written earlier in its
// workgroup, by the same invocation or by another ordered before the read by a
// barrier or uniform load, and every workgroup array index is within its array.
// The pipeline keeps WebGPU's zero-fill of workgroup memory, which this rule
// makes unobservable; CONTRIBUTING ("Carry the semantic contract into Lean and
// Rust") records the fill's status and what removing it requires.

struct Params {
    atom_count: u32,
    word_count: u32,
    rule_count: u32,
    world_count: u32,
    epoch: u32,
    reserved0: u32,
    reserved1: u32,
    reserved2: u32,
}

struct Rule {
    head: u32,
    positive_start: u32,
    positive_len: u32,
    true_start: u32,
    true_len: u32,
    false_start: u32,
    false_len: u32,
    reserved: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> rules: array<Rule>;
@group(0) @binding(2) var<storage, read> antecedents: array<u32>;
@group(0) @binding(3) var<storage, read> seeds: array<u32>;
@group(0) @binding(4) var<storage, read> carrier: array<u32>;
@group(0) @binding(5) var<storage, read_write> results: array<u32>;

// 4096 atoms, 128 words, 512 bytes. The two additional atomics and one
// synchronization value keep workgroup storage below 1 KiB.
// The closure: strided invocations zero words [0, word_count) at entry, and
// the entry barrier orders the zeroes before every rule's reads and ORs. Every
// index is a word below word_count or an atom's word, and the host bounds the
// atoms by 4096, word_count by their words and every rule's atoms by the atom
// count, so the words from word_count on are never read.
var<workgroup> known: array<atomic<u32>, 128>;
// Invocation 0 zeroes it at each round's start; the round's opening barrier
// orders the zero before any rule's store, and invocation 0 reads it after the
// barrier that ends the round.
var<workgroup> changed: atomic<u32>;
// Invocation 0 zeroes it at entry; the entry barrier orders the zero before
// any constraint's store, and invocation 0 reads it after the barriers that
// follow the constraints.
var<workgroup> violated: atomic<u32>;
// Written by invocation 0 and read by every invocation through
// workgroupUniformLoad: whether the round changed the closure.
var<workgroup> round_changed: u32;

fn seed_contains(world: u32, atom: u32) -> bool {
    return (seeds[world * params.word_count + atom / 32u] & (1u << (atom % 32u))) != 0u;
}

fn enabled(world: u32, rule: Rule) -> bool {
    for (var i = 0u; i < rule.true_len; i++) {
        if (!seed_contains(world, antecedents[rule.true_start + i])) {
            return false;
        }
    }
    for (var i = 0u; i < rule.false_len; i++) {
        if (seed_contains(world, antecedents[rule.false_start + i])) {
            return false;
        }
    }
    return true;
}

fn body_holds(rule: Rule) -> bool {
    for (var i = 0u; i < rule.positive_len; i++) {
        let atom = antecedents[rule.positive_start + i];
        if ((atomicLoad(&known[atom / 32u]) & (1u << (atom % 32u))) == 0u) {
            return false;
        }
    }
    return true;
}

@compute @workgroup_size(64)
fn check(@builtin(workgroup_id) group: vec3<u32>,
         @builtin(local_invocation_index) lane: u32) {
    let world = group.x;
    for (var word = lane; word < params.word_count; word += 64u) {
        atomicStore(&known[word], 0u);
    }
    if (lane == 0u) {
        atomicStore(&violated, 0u);
    }
    workgroupBarrier();

    loop {
        if (lane == 0u) {
            atomicStore(&changed, 0u);
        }
        workgroupBarrier();
        for (var r = lane; r < params.rule_count; r += 64u) {
            let rule = rules[r];
            if (rule.head != 0xffffffffu && enabled(world, rule) && body_holds(rule)) {
                let mask = 1u << (rule.head % 32u);
                let prior = atomicOr(&known[rule.head / 32u], mask);
                if ((prior & mask) == 0u) {
                    atomicStore(&changed, 1u);
                }
            }
        }
        workgroupBarrier();
        if (lane == 0u) {
            round_changed = atomicLoad(&changed);
        }
        // This builtin supplies a uniform control-flow value, with its own
        // workgroup synchronization; an ordinary atomic load cannot justify
        // uniform loop exit to the WGSL barrier validator.
        if (workgroupUniformLoad(&round_changed) == 0u) {
            break;
        }
    }

    for (var r = lane; r < params.rule_count; r += 64u) {
        let rule = rules[r];
        if (rule.head == 0xffffffffu && enabled(world, rule) && body_holds(rule)) {
            atomicStore(&violated, 1u);
        }
    }
    workgroupBarrier();
    let base = world * (params.word_count + 4u);
    for (var word = lane; word < params.word_count; word += 64u) {
        results[base + 4u + word] = atomicLoad(&known[word]);
    }
    // Publish completion only after every lane has stored its closure words.
    storageBarrier();
    workgroupBarrier();
    if (lane == 0u) {
        var status = atomicLoad(&violated); // bit 0: a constraint holds
        for (var word = 0u; word < params.word_count; word++) {
            if ((atomicLoad(&known[word]) & carrier[word]) != seeds[world * params.word_count + word]) {
                status |= 2u; // bit 1: the gate-carrier projection differs
            }
        }
        results[base] = params.epoch;
        results[base + 1u] = world;
        results[base + 2u] = status; // zero denotes exact acceptance in a complete record
        results[base + 3u] = 0x53524331u; // nonzero static completion marker
    }
}
