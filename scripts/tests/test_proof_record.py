"""Proof-record mutation and shell propagation tests; never invoke Lean or Cargo."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import proof_record


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Fixture:
    """A small internally consistent record, not kernel-checked proof evidence."""

    def __init__(self, root):
        self.root = root
        self.write("Zetesis/A.lean", '''import Std
namespace Zetesis
/- theorem ignored : False := by sorry
/- nested comment -/ -/
def label := "theorem also_ignored namespace Hidden end"
section Local
-- theorem another_ignored
 theorem placeholder
end Local
end Zetesis
'''.replace(' theorem placeholder', 'theorem first : True := by trivial'))
        self.write("Zetesis/Sub/B.lean", "namespace Zetesis.Sub\ntheorem second'\n    : True := by trivial\nend Zetesis.Sub\n")
        self.entries = [
            {"name": "Zetesis.first", "file": "Zetesis/A.lean", "line": 8},
            {"name": "Zetesis.Sub.second'", "file": "Zetesis/Sub/B.lean", "line": 2},
        ]
        for file, content in {
            "Zetesis.lean": "import Zetesis.A\nimport Zetesis.Sub.B\n",
            "lakefile.lean": "import Lake\n",
            "lake-manifest.json": '{"packages": []}\n',
            "lean-toolchain": "leanprover/lean4:v4.33.1\n",
            "README.md": "Fixture for a record checker, not checked Lean.\n",
            "Audit.lean": "import Zetesis\n\n#print axioms Zetesis.first\n#print axioms Zetesis.Sub.second'\n",
            "axiom-audit.txt": "'Zetesis.first' depends on axioms: [propext,\n Classical.choice,\n Quot.sound]\n'Zetesis.Sub.second\'' does not depend on any axioms\n",
            "verification/current/build.log": "Build completed successfully.\n",
        }.items():
            self.write(file, content)
        self.write_json("theorems.json", self.entries)
        self.write("verification/current/audit.log", (root / "axiom-audit.txt").read_text())
        self.record = {
            "schema_version": 1, "status": "PASS", "semantic_modules": 2,
            "theorems_audited": 2, "transitive_axioms": sorted(proof_record.ALLOWED_AXIOMS),
            "project_axioms": [], "proof_holes": False, "native_evaluation_proof_shortcuts": False,
            "commands": [self.command(["lake", "build"], "build.log"),
                         self.command(proof_record.AUDIT_COMMAND, "audit.log")],
            "audit_consistency": {
                **{key: 2 for key in ("source_theorem_declarations", "indexed_theorems", "printed_axiom_entries", "unique_qualified_names")},
                "source_index_line_check": "PASS", "allowed_transitive_axioms_only": "PASS",
            },
            "source_sha256": {path: "" for path in proof_record.FIXED_SOURCES | {"Zetesis/A.lean", "Zetesis/Sub/B.lean"}},
            "generated_artifact_sha256": {path: "" for path in proof_record.ARTIFACTS},
            "verification_log_sha256": {"verification/current/build.log": "", "verification/current/audit.log": ""},
        }
        self.refresh()

    def write(self, path, text):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text)

    def write_json(self, path, value):
        self.write(path, json.dumps(value, indent=2) + "\n")

    def command(self, command, log):
        return {"command": command, "cwd": str(self.root), "result": "PASS", "exit_code": 0,
                "elapsed_seconds": 0.1, "log": "verification/current/" + log}

    def refresh(self):
        for section in ("source_sha256", "generated_artifact_sha256", "verification_log_sha256"):
            for file in self.record[section]:
                self.record[section][file] = digest(self.root / file)
        self.save()

    def save(self):
        self.write_json("verification.json", self.record)


class ProofRecordTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.fixture = Fixture(self.root)

    def reject(self, reason):
        with self.assertRaisesRegex(proof_record.RecordError, reason):
            proof_record.verify(self.root)

    def test_nested_modules_comments_strings_sections_and_multiline_axioms(self):
        self.assertEqual(proof_record.verify(self.root),
                         {"theorems": 2, "semantic_modules": 2, "source_files": 7})
        before = {p.relative_to(self.root): digest(p) for p in self.root.rglob("*") if p.is_file()}
        proof_record.verify(self.root)
        self.assertEqual(before, {p.relative_to(self.root): digest(p) for p in self.root.rglob("*") if p.is_file()})

    def test_stale_generated_artifact_hashes(self):
        for file in sorted(proof_record.ARTIFACTS):
            with self.subTest(file=file):
                f = Fixture(self.root)
                f.write(file, (self.root / file).read_text() + "\n")
                self.reject("generated artifacts hash mismatch")

    def test_source_inventory_and_hashes_are_exact(self):
        for mutation in ("changed", "extra", "missing", "record_extra", "record_missing"):
            with self.subTest(mutation=mutation):
                f = Fixture(self.root)
                extra = self.root / "Zetesis/Extra.lean"
                extra.unlink(missing_ok=True)
                if mutation == "changed":
                    f.write("Zetesis/A.lean", "namespace Zetesis\nend Zetesis\n")
                elif mutation == "extra":
                    f.write("Zetesis/Extra.lean", "namespace Extra\nend Extra\n")
                elif mutation == "missing":
                    (self.root / "Zetesis/A.lean").unlink()
                elif mutation == "record_extra":
                    f.record["source_sha256"]["README.md"] = digest(self.root / "README.md")
                else:
                    del f.record["source_sha256"]["Zetesis/A.lean"]
                f.save()
                self.reject("source (inventory|hash) mismatch")

    def test_top_nested_and_module_counts_reject_staleness_and_boolean_counts(self):
        targets = [(None, "theorems_audited"), (None, "semantic_modules")]
        targets += [("audit_consistency", key) for key in self.fixture.record["audit_consistency"] if key.endswith(("declarations", "theorems", "entries", "names"))]
        for section, key in targets:
            for value in (1, True):
                with self.subTest(section=section, key=key, value=value):
                    f = Fixture(self.root)
                    target = f.record if section is None else f.record[section]
                    target[key] = value
                    f.save()
                    self.reject("count mismatch")

    def test_optional_module_theorem_count_is_derived(self):
        f = self.fixture
        old = "Zetesis/Sub/B.lean"
        new = "Zetesis/BatchAccounting.lean"
        (self.root / old).rename(self.root / new)
        f.record["source_sha256"][new] = f.record["source_sha256"].pop(old)
        f.entries[1]["file"] = new
        f.write_json("theorems.json", f.entries)
        f.record["batch_accounting_laws"] = 1
        f.refresh()
        proof_record.verify(self.root)
        f.record["batch_accounting_laws"] = 2
        f.save()
        self.reject("module theorem count mismatch")

    def test_index_missing_duplicate_wrong_name_file_and_line_after_rehash(self):
        for mutation in ("missing", "duplicate", "name", "file", "line", "bool_line"):
            with self.subTest(mutation=mutation):
                f = Fixture(self.root)
                rows = f.entries
                if mutation == "missing":
                    rows.pop()
                elif mutation == "duplicate":
                    rows.append(rows[0].copy())
                else:
                    rows[0]["line" if mutation == "bool_line" else mutation] = {
                        "name": "Zetesis.wrong", "file": "Zetesis/Sub/B.lean", "line": 9, "bool_line": True,
                    }[mutation]
                f.write_json("theorems.json", rows)
                f.refresh()
                self.reject("(index|location)")

    def test_audit_requests_missing_duplicate_and_unknown_after_rehash(self):
        for lines in (["Zetesis.first"], ["Zetesis.first", "Zetesis.first"], ["Zetesis.first", "Zetesis.unknown"]):
            with self.subTest(lines=lines):
                f = Fixture(self.root)
                f.write("Audit.lean", "import Zetesis\n" + "".join(f"#print axioms {name}\n" for name in lines))
                f.refresh()
                self.reject("Audit.lean names")

    def test_output_missing_duplicate_extra_reordered_and_garbage_after_rehash(self):
        original = (self.root / "axiom-audit.txt").read_text()
        first = "'Zetesis.first' does not depend on any axioms\n"
        second = "'Zetesis.Sub.second\'' does not depend on any axioms\n"
        for output in (first, first + first, original + first, second + first,
                       original + "warning: ignored\n", "unknown\n" + original):
            with self.subTest(output=output):
                f = Fixture(self.root)
                f.write("axiom-audit.txt", output)
                f.write("verification/current/audit.log", output)
                f.refresh()
                self.reject("(axiom output|audit output)")

    def test_disallowed_or_duplicate_axioms_cannot_be_laundered_by_rehashing(self):
        for axiom in ("sorryAx", "Lean.ofReduceBool", "Zetesis.custom", "propext, propext", "propext,"):
            with self.subTest(axiom=axiom):
                f = Fixture(self.root)
                output = f"'Zetesis.first' depends on axioms: [{axiom}]\n'Zetesis.Sub.second\'' does not depend on any axioms\n"
                f.write("axiom-audit.txt", output)
                f.write("verification/current/audit.log", output)
                f.refresh()
                self.reject("disallowed or duplicate axiom")

    def test_current_logs_need_hashes_success_and_exact_audit_bytes(self):
        for mutation in ("old_logs_only", "failed", "nonzero", "missing_log", "different_audit", "duplicate_log", "negative_time", "nan_time", "audit_flags", "no_build"):
            with self.subTest(mutation=mutation):
                f = Fixture(self.root)
                if mutation == "old_logs_only":
                    f.write("verification/old/audit.log", (self.root / "axiom-audit.txt").read_text())
                    f.record["verification_log_sha256"] = {"verification/old/audit.log": digest(self.root / "verification/old/audit.log")}
                elif mutation == "failed":
                    f.record["commands"][0]["result"] = "FAIL"
                elif mutation == "nonzero":
                    f.record["commands"][0]["exit_code"] = 1
                elif mutation == "missing_log":
                    (self.root / "verification/current/build.log").unlink()
                elif mutation == "different_audit":
                    f.write("verification/current/audit.log", "different but correctly hashed\n")
                    f.refresh()
                elif mutation == "duplicate_log":
                    f.record["commands"].append(f.record["commands"][0].copy())
                elif mutation in ("negative_time", "nan_time"):
                    f.record["commands"][0]["elapsed_seconds"] = -1 if mutation == "negative_time" else float("nan")
                elif mutation == "audit_flags":
                    f.record["commands"][1]["command"] = ["lake", "env", "lean", "Audit.lean"]
                else:
                    f.record["commands"].pop(0)
                f.save()
                self.reject("(command|record file)")

    def test_all_recorded_historical_log_hashes_are_also_checked(self):
        f = self.fixture
        f.write("verification/old/build.log", "historical\n")
        f.record["verification_log_sha256"]["verification/old/build.log"] = digest(self.root / "verification/old/build.log")
        f.save()
        proof_record.verify(self.root)
        f.write("verification/old/build.log", "changed\n")
        self.reject("verification logs hash mismatch")

    def test_unsupported_source_conventions_fail_closed(self):
        for source in ("namespace Zetesis\nprivate theorem hidden : True := by trivial\nend Zetesis\n",
                       "namespace Zetesis\n@[simp] theorem hidden : True := by trivial\nend Zetesis\n",
                       "namespace Zetesis\n@[simp]\ntheorem hidden : True := by trivial\nend Zetesis\n",
                       "namespace Zetesis\ntheorem «hidden» : True := by trivial\nend Zetesis\n",
                       "namespace Zetesis\nmacro \"hidden\" : command => pure default\nend Zetesis\n",
                       "namespace Zetesis\nend Other\n", "/- unfinished", 'def a := "unfinished'):
            with self.subTest(source=source):
                f = Fixture(self.root)
                f.write("Zetesis/A.lean", source)
                f.refresh()
                self.reject("(unsupported|mismatched|unterminated)")

    def test_duplicate_json_keys_and_escaping_paths_are_rejected(self):
        (self.root / "verification.json").write_text('{"status":"PASS","status":"FAIL"}')
        self.reject("duplicate JSON key")
        f = Fixture(self.root)
        f.record["schema_version"] = True
        f.save()
        self.reject("unsupported or unsuccessful")
        f = Fixture(self.root)
        f.record["verification_log_sha256"]["../elsewhere"] = "0" * 64
        f.save()
        self.reject("noncanonical record path")

    def test_cli_pass_failure_and_retained_alternative_record(self):
        command = [sys.executable, str(SCRIPTS / "proof_record.py"), "--proofs-dir", str(self.root)]
        result = subprocess.run(command, capture_output=True, text=True, timeout=10, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("2 theorems; 2 semantic modules; 7", result.stdout)
        self.fixture.record["audit_consistency"]["indexed_theorems"] = 1
        self.fixture.write_json("old.json", self.fixture.record)
        result = subprocess.run([*command, "--record", "old.json"], capture_output=True, text=True, timeout=10, check=False)
        self.assertEqual(result.returncode, 1)
        self.assertIn("nested count mismatch", result.stderr)
        self.assertNotIn("PASS", result.stdout)


class CheckShellTests(unittest.TestCase):
    def test_gate_runs_after_lean_in_proofs_and_full_and_propagates_failures(self):
        for mode, failure in (("proofs", ""), ("full", ""), ("proofs", "build"),
                              ("proofs", "audit"), ("proofs", "record"), ("full", "record")):
            with self.subTest(mode=mode, failure=failure), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / "scripts").mkdir()
                (root / "proofs").mkdir()
                (root / "bin").mkdir()
                (root / "scripts/check.sh").write_bytes((SCRIPTS / "check.sh").read_bytes())
                stub = '''#!/bin/sh
set -eu
name=${0##*/}
printf '%s %s\\n' "$name" "$*" >> "$TRACE"
case "$name:$*:$FAILURE" in
 lake:build:build|lake:*Audit.lean:audit|python3:scripts/proof_record.py:record) exit 23 ;;
esac
'''
                for name in ("lake", "cargo", "python3"):
                    path = root / "bin" / name
                    path.write_text(stub)
                    path.chmod(0o755)
                for name in ("coverage.sh", "validate.sh"):
                    path = root / "scripts" / name
                    path.write_text(stub)
                    path.chmod(0o755)
                trace = root / "trace"
                env = {**os.environ, "PATH": str(root / "bin") + os.pathsep + os.environ["PATH"],
                       "TRACE": str(trace), "FAILURE": failure}
                result = subprocess.run(["/bin/sh", str(root / "scripts/check.sh"), mode],
                                        env=env, capture_output=True, text=True, timeout=10, check=False)
                self.assertEqual(result.returncode, 23 if failure else 0, result.stderr)
                lines = trace.read_text().splitlines()
                gate = "python3 scripts/proof_record.py"
                if failure in ("build", "audit"):
                    self.assertNotIn(gate, lines)
                else:
                    self.assertEqual(lines.count(gate), 1)
                    self.assertEqual(lines[lines.index(gate) - 1], "lake env lean -DautoImplicit=false -DwarningAsError=true Audit.lean")
                    if mode == "full" and not failure:
                        self.assertLess(lines.index(gate), lines.index("coverage.sh "))
                    elif mode == "full":
                        self.assertFalse(any(line.startswith("coverage.sh") for line in lines))


if __name__ == "__main__":
    unittest.main()
