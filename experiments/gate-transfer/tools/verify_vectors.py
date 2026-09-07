#!/usr/bin/env python3
"""Independently replay all retained gate vectors without Rust, Lean or a GPU."""

import csv
import hashlib
import itertools
import json
from pathlib import Path

HERE = Path(__file__).resolve().parents[1]
ALIASES = {
    "Distinct": (0, 1, 2),
    "LeftRight": (0, 0, 1),
    "LeftOutput": (0, 1, 0),
    "RightOutput": (0, 1, 1),
    "All": (0, 0, 0),
}
OPERATIONS = {
    "And": lambda x, y: x and y,
    "Or": lambda x, y: x or y,
    "Implies": lambda x, y: not x or y,
}


def expected(operation, slots, code):
    masks = tuple((code >> (2 * position)) & 3 for position in range(3))
    supported = [set(), set(), set()]
    for row in itertools.product((False, True), repeat=3):
        if not all(masks[p] & (1 << int(row[p])) for p in range(3)):
            continue
        if not all(slots[p] != slots[q] or row[p] == row[q]
                   for p in range(3) for q in range(3)):
            continue
        if operation(row[0], row[1]) != row[2]:
            continue
        for position, value in enumerate(row):
            supported[position].add(value)
    return sum(sum(1 << int(value) for value in values) << (2 * position)
               for position, values in enumerate(supported))


def verify(path):
    data = path.read_bytes()
    if len(data) > 131072:
        raise ValueError("retained finite vector file exceeds its 128KiB ceiling")
    reader = csv.DictReader(data.decode().splitlines(), delimiter="\t")
    if reader.fieldnames != ["operation", "aliases", "domains", "reference", "bitwise", "lookup"]:
        raise ValueError("unexpected vector header")
    rows = list(reader)
    keys = list(itertools.product(OPERATIONS, ALIASES, range(64)))
    if len(rows) != len(keys):
        raise ValueError("missing or extra finite contract rows")
    for row, (operation, alias, code) in zip(rows, keys):
        if None in row or (row["operation"], row["aliases"], row["domains"]) != (
                operation, alias, str(code)):
            raise ValueError("changed, missing or reordered finite contract key")
        value = str(expected(OPERATIONS[operation], ALIASES[alias], code))
        if any(row[column] != value for column in ("reference", "bitwise", "lookup")):
            raise ValueError("retained transfer disagrees with independent relation")
    return {"status": "PASS", "cases": len(rows), "raw": "evidence/vectors.tsv",
            "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(),
            "scope": "finite transfer equality; no shader/device/performance claim"}


if __name__ == "__main__":
    print(json.dumps(verify(HERE / "evidence/vectors.tsv"), indent=2))
