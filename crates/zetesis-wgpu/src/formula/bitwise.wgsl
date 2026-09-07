// Eight relation rows encoded in one byte: x + 2*y + 4*z.
// These row bits are neither candidate bits nor the atomic domain encoding.
fn gate_rows(domain: u32, no: u32, yes: u32) -> u32 {
    return select(0u, no, (domain & 1u) != 0u) |
        select(0u, yes, (domain & 2u) != 0u);
}
fn gate_support(rows: u32, no: u32, yes: u32) -> u32 {
    return select(0u, 1u, (rows & no) != 0u) |
        select(0u, 2u, (rows & yes) != 0u);
}
fn gate(base: u32, node: Node) {
    let left = nodes[node.left].output;
    let right = nodes[node.right].output;
    let dx = atomicLoad(&domains[base + left]);
    let dy = atomicLoad(&domains[base + right]);
    let dz = atomicLoad(&domains[base + node.output]);
    var rows = 0xd2u; // z = (!x || y)
    if (node.tag == 2u) { rows = 0x87u; } // z = (x && y)
    if (node.tag == 3u) { rows = 0xe1u; } // z = (x || y)
    rows &= gate_rows(dx, 0x55u, 0xaau);
    rows &= gate_rows(dy, 0x33u, 0xccu);
    rows &= gate_rows(dz, 0x0fu, 0xf0u);
    // Compare physical slots. Aliased atomic loads need not be identical.
    if (left == right) { rows &= 0x99u; }
    if (left == node.output) { rows &= 0xa5u; }
    if (right == node.output) { rows &= 0xc3u; }
    narrow(base + left, gate_support(rows, 0x55u, 0xaau));
    narrow(base + right, gate_support(rows, 0x33u, 0xccu));
    narrow(base + node.output, gate_support(rows, 0x0fu, 0xf0u));
}
