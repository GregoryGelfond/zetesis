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
    return params.nodes * 4u + nodes[params.nodes * 4u + node.left];
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
fn numeric_truth(base: u32, node: Node) -> bool {
    let constant = numeric_constant(node);
    if (constant != 0u) { return constant == 2u; }
    if (node.tag == 7u) {
        var sum = vec4<u32>(0u);
        for (var index = 0u; index < node.right; index++) {
            if (frozen[base + numeric_condition(node, index)] != 0u) {
                sum = wide_add(sum, numeric_value(node, index));
            }
        }
        return !wide_less(sum, numeric_bound(node));
    }
    var value = false;
    for (var index = 0u; index < node.right; index++) {
        value = value || (numeric_qualifies(node, index) &&
            frozen[base + numeric_condition(node, index)] != 0u);
    }
    return value;
}
