// Two linear row passes. Bounds independently relax repeated condition slots;
// they can lose deductions but cannot remove a satisfying completion. Atomic
// domains only shrink, so stale lower/upper bounds remain conservative.
fn threshold_gate(base: u32, node: Node) {
    var minimum = vec4<u32>(0u);
    var maximum = vec4<u32>(0u);
    var empty = false;
    for (var index = 0u; index < node.right; index++) {
        let slot = node_at(numeric_condition(node, index)).output;
        let domain = atomicLoad(&domains[base + slot]);
        let weight = numeric_value(node, index);
        empty = empty || domain == 0u;
        if (domain == 2u) { minimum = wide_add(minimum, weight); }
        if ((domain & 2u) != 0u) { maximum = wide_add(maximum, weight); }
    }
    let bound = numeric_bound(node);
    var allowed = select(0u, 1u, wide_less(minimum, bound)) |
        select(0u, 2u, !wide_less(maximum, bound));
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
            let weight = numeric_value(node, index);
            if (output == 2u && wide_less(wide_sub(maximum, weight), bound)) {
                narrow(base + slot, 2u);
            }
            if (output == 1u && !wide_less(wide_add(minimum, weight), bound)) {
                narrow(base + slot, 1u);
            }
        }
    }
}
fn extremum_gate(base: u32, node: Node) {
    let constant = numeric_constant(node);
    if (constant != 0u) { narrow(base + node.output, constant); return; }
    var false_possible = true;
    var empty = false;
    var has_witness = false;
    var multiple = false;
    var witness = 0u;
    for (var index = 0u; index < node.right; index++) {
        let slot = node_at(numeric_condition(node, index)).output;
        let domain = atomicLoad(&domains[base + slot]);
        empty = empty || domain == 0u;
        if (numeric_qualifies(node, index)) {
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
            if (numeric_qualifies(node, index)) {
                narrow(base + node_at(numeric_condition(node, index)).output, 1u);
            }
        }
    } else if (output == 2u && has_witness && !multiple) {
        narrow(base + witness, 2u);
    }
}
