// One row owner propagates requirements among its M-true guards. For a common
// sum S, S>=h entails S>=l when h>=l; S<u entails S<v when u<=v. False frozen
// guards are constants, never requirements or targets. Stale reads only omit
// stronger requirements; atomic intersections keep every compatible completion.
// No condition sum is repeated here and no per-candidate row state is retained.
fn threshold_guard_family(base: u32, mask_base: u32, index: u32, node: Node) {
    let row = numeric_row(node);
    if (nodes[row - 3u] != index) { return; }
    let count = nodes[row - 2u];
    if (count < 2u) { return; }
    let width = numeric_width(node);
    let guards = row + (1u + min(width, 2u)) * node.right;
    var lower = vec4<u32>(0u);
    var upper = vec4<u32>(0u);
    var has_upper = false;
    for (var position = 0u; position < count; position++) {
        let guard = nodes[guards + position];
        if (frozen[mask_base + guard] == 0u) { continue; }
        let guard_node = node_at(guard);
        let domain = atomicLoad(&domains[base + guard_node.output]);
        let bound = numeric_bound(guard_node);
        if (domain == 0u) { atomicStore(&conflict, 1u); return; }
        if (domain == 2u && row_less(width, lower, bound)) { lower = bound; }
        if (domain == 1u && (!has_upper || row_less(width, bound, upper))) {
            upper = bound;
            has_upper = true;
        }
    }
    if (has_upper && !row_less(width, lower, upper)) {
        atomicStore(&conflict, 1u);
        return;
    }
    for (var position = 0u; position < count; position++) {
        let guard = nodes[guards + position];
        if (frozen[mask_base + guard] == 0u) { continue; }
        let guard_node = node_at(guard);
        let bound = numeric_bound(guard_node);
        var allowed = 3u;
        if (!row_less(width, lower, bound)) { allowed &= 2u; }
        if (has_upper && !row_less(width, bound, upper)) { allowed &= 1u; }
        narrow(base + guard_node.output, allowed);
    }
}

// Two linear row passes. Bounds independently relax repeated condition slots;
// they can lose deductions but cannot remove a satisfying completion. Atomic
// domains only shrink, so stale lower/upper bounds remain conservative.
fn threshold_gate(base: u32, node: Node) {
    let width = numeric_width(node);
    var minimum = vec4<u32>(0u);
    var maximum = vec4<u32>(0u);
    var empty = false;
    for (var index = 0u; index < node.right; index++) {
        let slot = node_at(numeric_condition(node, index)).output;
        let domain = atomicLoad(&domains[base + slot]);
        let weight = numeric_value(node, index, width);
        empty = empty || domain == 0u;
        if (domain == 2u) { minimum = row_add(width, minimum, weight); }
        if ((domain & 2u) != 0u) { maximum = row_add(width, maximum, weight); }
    }
    let bound = numeric_bound(node);
    var allowed = select(0u, 1u, row_less(width, minimum, bound)) |
        select(0u, 2u, !row_less(width, maximum, bound));
    if (empty) { allowed = 0u; }
    let output = atomicLoad(&domains[base + node.output]) & allowed;
    narrow(base + node.output, allowed);
    // Settled bounds entail the output. Output conflicts were recorded above;
    // no undecided contribution can require forcing in this case.
    if (allowed != 3u) { return; }
    if (output != 1u && output != 2u) { return; }
    for (var index = 0u; index < node.right; index++) {
        let slot = node_at(numeric_condition(node, index)).output;
        // An undecided slot was undecided in the first pass as well. Its
        // weight is in maximum and absent from minimum, even with stale reads.
        if (atomicLoad(&domains[base + slot]) == 3u) {
            let weight = numeric_value(node, index, width);
            if (output == 2u && row_less(width, row_sub(width, maximum, weight), bound)) {
                narrow(base + slot, 2u);
            }
            if (output == 1u && !row_less(width, row_add(width, minimum, weight), bound)) {
                narrow(base + slot, 1u);
            }
        }
    }
}
fn extremum_gate(base: u32, node: Node) {
    let constant = numeric_constant(node);
    if (constant != 0u) { narrow(base + node.output, constant); return; }
    let width = numeric_width(node);
    var false_possible = true;
    var empty = false;
    var has_witness = false;
    var multiple = false;
    var witness = 0u;
    for (var index = 0u; index < node.right; index++) {
        let slot = node_at(numeric_condition(node, index)).output;
        let domain = atomicLoad(&domains[base + slot]);
        empty = empty || domain == 0u;
        if (numeric_qualifies(node, index, width)) {
            false_possible = false_possible && (domain & 1u) != 0u;
            if ((domain & 2u) != 0u) {
                if (has_witness && witness != slot) { multiple = true; }
                has_witness = true;
                witness = slot;
            }
        }
    }
    var allowed = select(0u, 1u, false_possible) | select(0u, 2u, has_witness);
    if (empty) { allowed = 0u; }
    let output = atomicLoad(&domains[base + node.output]) & allowed;
    narrow(base + node.output, allowed);
    // A settled witness result cannot force another child.
    if (allowed != 3u) { return; }
    if (output == 1u) {
        for (var index = 0u; index < node.right; index++) {
            if (numeric_qualifies(node, index, width)) {
                narrow(base + node_at(numeric_condition(node, index)).output, 1u);
            }
        }
    } else if (output == 2u && has_witness && !multiple) {
        narrow(base + witness, 2u);
    }
}
