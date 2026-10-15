// Portable numeric rows shared by original-truth and frozen-query consumers.
// The admitted Theory bounds every nonnegative sum by i128::MAX. Four little-
// endian u32 limbs retain that range without optional device integer features.
fn wide_add(a: vec4<u32>, b: vec4<u32>) -> vec4<u32> {
    var result = vec4<u32>(0u);
    var carry = 0u;
    for (var index = 0u; index < 4u; index++) {
        let partial = a[index] + b[index];
        let value = partial + carry;
        let next = select(0u, 1u, partial < a[index] || value < partial);
        result[index] = value;
        carry = next;
    }
    return result;
}
// The caller establishes a >= b; no wrapped difference is used as a measure.
fn wide_sub(a: vec4<u32>, b: vec4<u32>) -> vec4<u32> {
    var result = vec4<u32>(0u);
    var borrow = 0u;
    for (var index = 0u; index < 4u; index++) {
        let partial = a[index] - b[index];
        let value = partial - borrow;
        let next = select(0u, 1u, a[index] < b[index] || partial < borrow);
        result[index] = value;
        borrow = next;
    }
    return result;
}
fn wide_less(a: vec4<u32>, b: vec4<u32>) -> bool {
    for (var remaining = 4u; remaining > 0u; remaining--) {
        let index = remaining - 1u;
        if (a[index] != b[index]) { return a[index] < b[index]; }
    }
    return false;
}
fn numeric_row(node: Node) -> u32 {
    return params.nodes * 4u + nodes[params.nodes * 4u + node.left] + 2u;
}
fn numeric_condition(node: Node, index: u32) -> u32 {
    return nodes[numeric_row(node) + index];
}
fn numeric_value(node: Node, index: u32) -> vec4<u32> {
    let start = numeric_row(node) + node.right;
    return vec4<u32>(nodes[start + index], nodes[start + node.right + index], 0u, 0u);
}
fn numeric_bound(node: Node) -> vec4<u32> {
    let start = params.nodes * 4u + node.left + 1u;
    if (node.tag == 7u) {
        return vec4<u32>(nodes[start], nodes[start + 1u], nodes[start + 2u], nodes[start + 3u]);
    }
    return vec4<u32>(nodes[start], nodes[start + 1u], 0u, 0u);
}
// 0 means variable; 1/2 are the ordinary false/true domain bits.
fn numeric_constant(node: Node) -> u32 {
    let bound = numeric_bound(node);
    if (all(bound == vec4<u32>(0u))) {
        return select(2u, 1u, node.tag == 8u);
    }
    if (node.tag != 7u) {
        let start = params.nodes * 4u + node.left + 3u;
        let count = vec4<u32>(nodes[start], nodes[start + 1u], 0u, 0u);
        if (all(bound == count)) { return select(1u, 2u, node.tag == 8u); }
    }
    return 0u;
}
fn numeric_qualifies(node: Node, index: u32) -> bool {
    let less = wide_less(numeric_value(node, index), numeric_bound(node));
    return select(!less, less, node.tag == 8u);
}
// One owner reduces each authoritative row and writes every nonconstant guard.
// Followers do not write: they may run before the owner in the same dependency
// level. All guards read identical children, so the existing level barrier makes
// the complete family visible before any parent. Serial order uses the earliest
// nonconstant guard as owner. No truth scratch or extra dispatch is needed.
fn numeric_original(base: u32, index: u32, node: Node) {
    let row = numeric_row(node);
    if (nodes[row - 2u] != index) { return; }
    let count = nodes[row - 1u];
    var sum = vec4<u32>(0u);
    let descriptor = params.nodes * 4u + node.left;
    var minimum = vec4<u32>(0u);
    var maximum = vec4<u32>(0u);
    if (node.tag != 7u) {
        minimum = wide_sub(vec4<u32>(nodes[descriptor + 3u], nodes[descriptor + 4u], 0u, 0u),
            vec4<u32>(1u, 0u, 0u, 0u));
    }
    for (var position = 0u; position < node.right; position++) {
        if (frozen[base + numeric_condition(node, position)] != 0u) {
            let value = numeric_value(node, position);
            if (node.tag == 7u) {
                sum = wide_add(sum, value);
            } else {
                if (wide_less(value, minimum)) { minimum = value; }
                if (wide_less(maximum, value)) { maximum = value; }
            }
        }
    }
    let guards = row + 3u * node.right;
    for (var position = 0u; position < count; position++) {
        let guard = nodes[guards + position];
        let guard_node = node_at(guard);
        let bound = numeric_bound(guard_node);
        var value = false;
        if (guard_node.tag == 7u) { value = !wide_less(sum, bound); }
        if (guard_node.tag == 8u) { value = wide_less(minimum, bound); }
        if (guard_node.tag == 9u) { value = !wide_less(maximum, bound); }
        frozen[base + guard] = select(0u, 1u, value);
    }
}
