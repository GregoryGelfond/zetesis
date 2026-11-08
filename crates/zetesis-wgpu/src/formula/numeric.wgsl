// One conservative interval per occurrence row. Atomic domains only shrink:
// independently stale reads still enclose every compatible complete reading.
struct ThresholdBounds { minimum: vec4<u32>, maximum: vec4<u32>, empty: bool }
fn threshold_bounds(base: u32, node: Node, width: u32) -> ThresholdBounds {
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
    return ThresholdBounds(minimum, maximum, empty);
}
fn threshold_allowed(width: u32, bounds: ThresholdBounds, bound: vec4<u32>) -> u32 {
    if (bounds.empty) { return 0u; }
    return select(0u, 1u, row_less(width, bounds.minimum, bound)) |
        select(0u, 2u, !row_less(width, bounds.maximum, bound));
}
// The requirement bits name demanded false/true guards, not the row's domain.
// Repeated physical slots retain their individual weights. Each test can lose
// deductions relative to a complete grouped coefficient, but remains sound.
fn threshold_force(base: u32, node: Node, width: u32, bounds: ThresholdBounds,
    lower: vec4<u32>, upper: vec4<u32>, required: u32) {
    if (required == 0u || bounds.empty) { return; }
    for (var index = 0u; index < node.right; index++) {
        let slot = node_at(numeric_condition(node, index)).output;
        // A currently undecided slot was undecided at its first-pass visit.
        // Its weight was included in maximum and absent from minimum.
        if (atomicLoad(&domains[base + slot]) == 3u) {
            let weight = numeric_value(node, index, width);
            if ((required & 2u) != 0u &&
                row_less(width, row_sub(width, bounds.maximum, weight), lower)) {
                narrow(base + slot, 2u);
            }
            if ((required & 1u) != 0u &&
                !row_less(width, row_add(width, bounds.minimum, weight), upper)) {
                narrow(base + slot, 1u);
            }
        }
    }
}
// The existing row owner reduces conditions once, fans out allowed outputs and
// applies the strongest live requirements in one forcing pass. A frozen-false
// owner still carries its live siblings; false guards are never requirements or
// targets. No candidate row state is retained between sweeps.
// True means the shared-row transfer owns this node, including nonowners.
fn threshold_guard_family(base: u32, mask_base: u32, index: u32, node: Node) -> bool {
    let row = numeric_row(node);
    let count = nodes[row - 2u];
    if (count < 2u) { return false; }
    if (nodes[row - 3u] != index) { return true; }
    let width = numeric_width(node);
    let guards = row + (1u + min(width, 2u)) * node.right;
    var lower = vec4<u32>(0u);
    var upper = vec4<u32>(0u);
    var required = 0u;
    var eligible = false;
    for (var position = 0u; position < count; position++) {
        let guard = nodes[guards + position];
        if (frozen[mask_base + guard] == 0u) { continue; }
        eligible = true;
        let guard_node = node_at(guard);
        let domain = atomicLoad(&domains[base + guard_node.output]);
        let bound = numeric_bound(guard_node);
        if (domain == 0u) { atomicStore(&conflict, 1u); return true; }
        if (domain == 2u) {
            required |= 2u;
            if (row_less(width, lower, bound)) { lower = bound; }
        }
        if (domain == 1u) {
            if ((required & 1u) == 0u || row_less(width, bound, upper)) { upper = bound; }
            required |= 1u;
        }
    }
    if (!eligible) { return true; }
    if ((required & 1u) != 0u && !row_less(width, lower, upper)) {
        atomicStore(&conflict, 1u);
        return true;
    }
    let bounds = threshold_bounds(base, node, width);
    for (var position = 0u; position < count; position++) {
        let guard = nodes[guards + position];
        if (frozen[mask_base + guard] == 0u) { continue; }
        let guard_node = node_at(guard);
        let bound = numeric_bound(guard_node);
        var allowed = threshold_allowed(width, bounds, bound);
        if (!row_less(width, lower, bound)) { allowed &= 2u; }
        if ((required & 1u) != 0u && !row_less(width, bound, upper)) { allowed &= 1u; }
        narrow(base + guard_node.output, allowed);
    }
    threshold_force(base, node, width, bounds, lower, upper, required);
    return true;
}
// Single-guard rows retain the same two passes and settled-output short cut.
fn threshold_gate(base: u32, node: Node) {
    let width = numeric_width(node);
    let bounds = threshold_bounds(base, node, width);
    let bound = numeric_bound(node);
    let allowed = threshold_allowed(width, bounds, bound);
    let output = atomicLoad(&domains[base + node.output]) & allowed;
    narrow(base + node.output, allowed);
    if (allowed != 3u) { return; }
    if (output != 1u && output != 2u) { return; }
    threshold_force(base, node, width, bounds, bound, bound, output);
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
