#!/usr/bin/env python3
"""Check retained proof-record consistency without running Lean or writing files.

This is not a Lean parser or kernel verifier. The admitted record convention is
plain, line-leading `theorem NAME`, explicit namespace/section/end commands, and
ASCII qualified names (apostrophes allowed). Comments may nest; strings and
comments are masked while retaining source lines. Declaration-generating macros,
private/protected/attribute-prefixed theorems, quoted names and other declaration
forms are refused. Audit.lean contains only `import Zetesis` and `#print axioms`.
Changing these conventions requires an explicit checker update and review.

Hashes establish internal consistency, not trustworthy command execution or a
verified source-to-binary relationship. Run the actual pinned Lean checks first.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path, PurePosixPath
import re
import sys

NAME = r"[A-Za-z_][A-Za-z_0-9']*(?:\.[A-Za-z_][A-Za-z_0-9']*)*"
ALLOWED_AXIOMS = {"propext", "Classical.choice", "Quot.sound"}
ARTIFACTS = {"README.md", "theorems.json", "axiom-audit.txt"}
FIXED_SOURCES = {"Audit.lean", "Zetesis.lean", "lakefile.lean", "lake-manifest.json", "lean-toolchain"}
AUDIT_COMMAND = ["lake", "env", "lean", "-DautoImplicit=false", "-DwarningAsError=true", "Audit.lean"]


class RecordError(ValueError):
    """An inconsistent or unsupported retained proof record."""


def require(condition, message):
    if not condition:
        raise RecordError(message)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON key: {key}")
        result[key] = value
    return result


def read_json(path):
    return json.loads(path.read_text(), object_pairs_hook=unique_object)


def member(root, relative):
    require(isinstance(relative, str), "record path must be a string")
    path = PurePosixPath(relative)
    require(not path.is_absolute() and path.parts and
            all(part not in (".", "..") for part in path.parts) and
            str(path) == relative, f"noncanonical record path: {relative}")
    result = root / relative
    require(result.resolve().is_relative_to(root.resolve()), f"record path escapes proofs: {relative}")
    require(result.is_file(), f"missing regular record file: {relative}")
    return result


def hashes(root, recorded, expected, label):
    require(isinstance(recorded, dict), f"{label} must be an object")
    if expected is not None:
        require(set(recorded) == expected, f"{label} inventory mismatch")
    for name, digest in recorded.items():
        require(isinstance(digest, str) and re.fullmatch(r"[0-9a-f]{64}", digest),
                f"invalid {label} digest: {name}")
        actual = hashlib.sha256(member(root, name).read_bytes()).hexdigest()
        require(actual == digest, f"{label} hash mismatch: {name}")


def masked(source):
    """Mask nested Lean comments and double-quoted strings, retaining newlines."""
    output = list(source)
    depth = 0
    string = False
    index = 0
    while index < len(source):
        pair = source[index:index + 2]
        char = source[index]
        if depth:
            if pair == "/-":
                depth += 1
            elif pair == "-/":
                depth -= 1
            else:
                output[index] = "\n" if char == "\n" else " "
                index += 1
                continue
            output[index:index + 2] = "  "
            index += 2
        elif string:
            output[index] = "\n" if char == "\n" else " "
            if char == "\\":
                index += 1
                require(index < len(source), "unterminated string escape")
                output[index] = "\n" if source[index] == "\n" else " "
            elif char == '"':
                string = False
            index += 1
        elif pair == "/-":
            depth = 1
            output[index:index + 2] = "  "
            index += 2
        elif pair == "--":
            while index < len(source) and source[index] != "\n":
                output[index] = " "
                index += 1
        elif char == '"':
            string = True
            output[index] = " "
            index += 1
        else:
            index += 1
    require(not depth and not string, "unterminated comment or string")
    return "".join(output)


def declarations(root, files):
    entries = []
    for file in sorted(files):
        stack = []
        for number, line in enumerate(masked(member(root, file).read_text()).splitlines(), 1):
            location = f"{file}:{number}"
            text = line.strip()
            if not text:
                continue
            require(not re.search(r"\b(?:lemma|axiom|sorry|admit|native_decide|macro|elab|syntax|initialize|builtin_initialize|export)\b", text)
                    and not text.startswith(("#", "@")), f"unsupported source convention at {location}")
            opening = re.fullmatch(rf"(namespace|section)(?:\s+({NAME}))?", text)
            closing = re.fullmatch(rf"end(?:\s+({NAME}))?", text)
            theorem = re.match(rf"theorem\s+({NAME})(?=$|\s|[:(\[{{])", text)
            if opening:
                kind, name = opening.groups()
                require(kind != "namespace" or name is not None, f"unnamed namespace at {location}")
                stack.append((kind, name))
            elif closing:
                require(bool(stack), f"unmatched end at {location}")
                _, name = stack.pop()
                require(closing[1] is None or closing[1] == name, f"mismatched end at {location}")
            elif theorem:
                require(line.startswith("theorem ") and len(re.findall(r"\btheorem\b", text)) == 1,
                        f"unsupported theorem declaration at {location}")
                namespace = [name for kind, name in stack if kind == "namespace"]
                require(bool(namespace), f"theorem outside namespace at {location}")
                entries.append({"name": ".".join([*namespace, theorem[1]]), "file": file, "line": number})
            else:
                require(not re.search(r"\b(?:theorem|namespace|section|end)\b", text),
                        f"unsupported declaration/scope syntax at {location}")
        require(not stack, f"unclosed scope in {file}")
    names = [entry["name"] for entry in entries]
    require(len(names) == len(set(names)), "duplicate qualified source theorem")
    require(bool(entries), "empty source theorem inventory")
    return entries


def theorem_index(root, expected):
    entries = read_json(member(root, "theorems.json"))
    require(isinstance(entries, list), "theorem index must be a list")
    indexed = []
    for entry in entries:
        require(isinstance(entry, dict) and set(entry) == {"name", "file", "line"}, "invalid theorem index row")
        require(isinstance(entry["name"], str) and isinstance(entry["file"], str)
                and type(entry["line"]) is int and entry["line"] > 0, "invalid theorem index location")
        indexed.append((entry["name"], entry["file"], entry["line"]))
    require(len({row[0] for row in indexed}) == len(indexed), "duplicate theorem index name")
    actual = {(entry["name"], entry["file"], entry["line"]) for entry in expected}
    require(set(indexed) == actual, "theorem index names/files/lines differ from source declarations")
    return [row[0] for row in indexed]


def audit_names(root, indexed):
    requested = []
    imported = False
    for line in masked(member(root, "Audit.lean").read_text()).splitlines():
        text = line.strip()
        if not text:
            continue
        if text == "import Zetesis":
            require(not imported, "duplicate Audit import")
            imported = True
            continue
        match = re.fullmatch(rf"#print axioms ({NAME})", text)
        require(match is not None, "unsupported Audit.lean command")
        requested.append(match[1])
    require(imported and len(requested) == len(set(requested)) and
            set(requested) == set(indexed), "Audit.lean names differ from unique index")
    text = member(root, "axiom-audit.txt").read_text()
    pattern = re.compile(rf"'({NAME})'\s+(?:depends on axioms:\s*\[([^\]]*)\]|does not depend on any axioms)")
    seen = []
    used = set()
    position = 0
    for match in pattern.finditer(text):
        require(not text[position:match.start()].strip(), "unexpected axiom audit output")
        values = [] if not match[2] or not match[2].strip() else [v.strip() for v in match[2].split(",")]
        require(len(values) == len(set(values)) and set(values) <= ALLOWED_AXIOMS,
                f"disallowed or duplicate axiom for {match[1]}")
        used.update(values)
        seen.append(match[1])
        position = match.end()
    require(not text[position:].strip(), "unexpected trailing axiom audit output")
    require(seen == requested, "axiom output names/order differ from Audit.lean")
    return used


def command_logs(root, record):
    commands = record.get("commands")
    require(isinstance(commands, list) and bool(commands), "missing current commands")
    log_hashes = record.get("verification_log_sha256")
    hashes(root, log_hashes, None, "verification logs")
    seen = set()
    audits = []
    for entry in commands:
        require(isinstance(entry, dict), "invalid current command")
        command = entry.get("command")
        require(isinstance(command, list) and bool(command) and all(isinstance(x, str) and x for x in command),
                "invalid current command arguments")
        require(entry.get("result") == "PASS" and type(entry.get("exit_code")) is int and entry["exit_code"] == 0,
                "current command did not pass")
        elapsed = entry.get("elapsed_seconds")
        require(type(elapsed) in (int, float) and math.isfinite(elapsed) and elapsed >= 0,
                "invalid command elapsed time")
        require(isinstance(entry.get("cwd"), str) and bool(entry["cwd"]), "missing recorded command cwd")
        log = entry.get("log")
        require(isinstance(log, str) and log in log_hashes, "current command log is not hashed")
        require(log not in seen, "duplicate current command log")
        seen.add(log)
        if command == AUDIT_COMMAND:
            audits.append(log)
    require(["lake", "build"] in [entry["command"] for entry in commands], "missing current lake build command")
    require(len(audits) == 1, "expected exactly one current strict Audit.lean command")
    require(member(root, audits[0]).read_bytes() == member(root, "axiom-audit.txt").read_bytes(),
            "current Audit command log differs from axiom-audit.txt")


def verify(root, record_path="verification.json"):
    root = Path(root).resolve()
    record = read_json(member(root, record_path))
    require(isinstance(record, dict) and type(record.get("schema_version")) is int and record["schema_version"] == 1 and record.get("status") == "PASS",
            "unsupported or unsuccessful verification record")
    modules = {str(path.relative_to(root)) for path in (root / "Zetesis").rglob("*.lean")}
    require(bool(modules), "missing semantic modules")
    hashes(root, record.get("source_sha256"), modules | FIXED_SOURCES, "source")
    hashes(root, record.get("generated_artifact_sha256"), ARTIFACTS, "generated artifacts")
    entries = declarations(root, modules)
    indexed = theorem_index(root, entries)
    axioms = audit_names(root, indexed)
    count = len(entries)
    require(type(record.get("semantic_modules")) is int and record["semantic_modules"] == len(modules),
            "semantic module count mismatch")
    require(type(record.get("theorems_audited")) is int and record["theorems_audited"] == count,
            "top-level theorem count mismatch")
    if "batch_accounting_laws" in record:
        module_count = sum(entry["file"] == "Zetesis/BatchAccounting.lean" for entry in entries)
        require(type(record["batch_accounting_laws"]) is int and module_count > 0
                and record["batch_accounting_laws"] == module_count, "batch accounting module theorem count mismatch")
    consistency = record.get("audit_consistency")
    require(isinstance(consistency, dict), "missing nested audit consistency")
    for key in ("source_theorem_declarations", "indexed_theorems", "printed_axiom_entries", "unique_qualified_names"):
        require(type(consistency.get(key)) is int and consistency[key] == count, f"nested count mismatch: {key}")
    for key in ("source_index_line_check", "allowed_transitive_axioms_only"):
        require(consistency.get(key) == "PASS", f"nested audit failure: {key}")
    require(record.get("transitive_axioms") == sorted(axioms), "transitive axiom inventory mismatch")
    require(record.get("project_axioms") == [] and record.get("proof_holes") is False
            and record.get("native_evaluation_proof_shortcuts") is False, "unsupported proof assurance flags")
    command_logs(root, record)
    return {"theorems": count, "semantic_modules": len(modules), "source_files": len(modules | FIXED_SOURCES)}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--proofs-dir", type=Path, default=Path(__file__).resolve().parents[1] / "proofs")
    parser.add_argument("--record", default="verification.json", help="proofs-relative retained record to check")
    args = parser.parse_args()
    try:
        summary = verify(args.proofs_dir, args.record)
    except (RecordError, OSError, UnicodeError, json.JSONDecodeError) as error:
        print(f"Proof record: FAIL: {error}", file=sys.stderr)
        return 1
    print(f"Proof record: PASS: {summary['theorems']} theorems; {summary['semantic_modules']} semantic modules; {summary['source_files']} source/configuration files")
    return 0


if __name__ == "__main__":
    sys.exit(main())
