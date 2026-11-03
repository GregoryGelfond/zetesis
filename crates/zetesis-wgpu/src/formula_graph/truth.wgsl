// Shared immutable graph and candidate reads for the formula and tight kernels.
// Node's fourth word belongs to its consumer; the header read preserves it.
// Callers retain graph validation, dependency order and all synchronization.
// Headers occupy 4*N words; wide rows occupy the checked appended tail.
// One admitted world occupies params.words consecutive u32 words. Callers
// supply a semantic atom in that row; padding is outside this operation.
fn candidate_contains(world: u32, atom: u32) -> bool {
    let bit = packed_bit(atom);
    return (candidates[world * params.words + bit.word] & bit.mask) != 0u;
}

fn node_at(index: u32) -> Node {
    let base = index * 4u;
    return Node(nodes[base], nodes[base + 1u], nodes[base + 2u], nodes[base + 3u]);
}
fn operand_at(node: Node, index: u32) -> u32 {
    return nodes[params.nodes * 4u + node.left + index];
}

// Tags 2, 3 and 4 denote conjunction, disjunction and implication.
fn operation(tag: u32, left: bool, right: bool) -> bool {
    if (tag == 2u) { return left && right; }
    if (tag == 3u) { return left || right; }
    return !left || right;
}

// Original truth only: all invocations enter together. A serial chain is owned
// by lane 0; otherwise each ready level is strided across the workgroup. Each
// level ends with both barriers. The caller keeps its own final publication
// barrier and intervening work. The consumer supplies original; formula passes
// false for reuse, while tight evaluation retains its existing reuse decision.
fn evaluate_original(world: u32, lane: u32, values: u32, reuse: bool) {
    if (params.levels == 0u) {
        if (lane == 0u) {
            for (var index = 0u; index < params.nodes; index++) {
                original(world, values, index, reuse);
            }
        }
    } else {
        let order = params.roots + params.levels + 1u;
        for (var level = 0u; level < params.levels; level++) {
            let start = roots[params.roots + level];
            let end = roots[params.roots + level + 1u];
            for (var offset = start + lane; offset < end; offset += WORKGROUP_SIZE) {
                original(world, values, roots[order + offset], reuse);
            }
            storageBarrier();
            workgroupBarrier();
        }
    }
}
