#!/usr/bin/env python3
"""Prepare once, then verify an unapplied shader alternative without a GPU."""

import argparse
import difflib
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parents[1]
REPO = HERE.parents[1]
SOURCE = REPO / "crates/zetesis-wgpu/src/formula.wgsl"
SOURCE_SHA = "f7269f85632ddcfda49561e34f8b8aa0443bddecd5ee83f0113798fc2f96aca9"

GATE = """// Eight relation rows encoded in one byte: x + 2*y + 4*z.
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
"""


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="create missing artifacts once")
    args = parser.parse_args()
    original = SOURCE.read_bytes()
    if sha(original) != SOURCE_SHA:
        raise SystemExit("source shader differs from the reviewed frozen input")
    text = original.decode("utf-8")
    begin = text.index("fn gate(base: u32, node: Node) {")
    end = text.index("fn finish(", begin)
    proposed = text[:begin] + GATE + text[end:]
    # Everything outside the one transfer function stays byte-for-byte intact.
    assert proposed[:begin] == text[:begin]
    assert proposed[begin + len(GATE):] == text[end:]
    patch = "".join(difflib.unified_diff(
        text.splitlines(keepends=True), proposed.splitlines(keepends=True),
        fromfile="a/crates/zetesis-wgpu/src/formula.wgsl",
        tofile="b/crates/zetesis-wgpu/src/formula.wgsl",
    ))
    payloads = {
        "original-formula.wgsl": original,
        "proposed-formula.wgsl": proposed.encode(),
        "gate-transfer-unapplied.patch": patch.encode(),
    }
    record = {
        "scope": "unapplied gate replacement; no shader compilation or device execution",
        "source": "crates/zetesis-wgpu/src/formula.wgsl",
        "source_sha256": SOURCE_SHA,
        "generator_sha256": sha(Path(__file__).read_bytes()),
        "unchanged": ["atomic loads/intersections", "original frozen truth",
                      "M-false connective suppression", "barriers and sweep order",
                      "strict-subset clause", "epochs and results", "work/round accounting"],
        "files": {name: {"bytes": len(data), "sha256": sha(data)}
                  for name, data in payloads.items()},
    }
    payloads["patch-record.json"] = (json.dumps(record, indent=2) + "\n").encode()
    destination = HERE / "generated"
    if args.write:
        destination.mkdir(exist_ok=True)
        for name, data in payloads.items():
            path = destination / name
            if path.exists():
                raise SystemExit(f"refusing to overwrite retained artifact: {name}")
            path.write_bytes(data)
    else:
        for name, data in payloads.items():
            if (destination / name).read_bytes() != data:
                raise SystemExit(f"retained artifact differs: {name}")
    print(json.dumps({"mode": "prepared" if args.write else "read-only replay",
                      "files": len(payloads), "source_sha256": SOURCE_SHA,
                      "production_shader_changed": False}))


if __name__ == "__main__":
    main()
