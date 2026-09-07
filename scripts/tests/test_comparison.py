"""Portable benchmark-contract checks; never launch either answer-set solver."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
from comparison_contract import clingo, contracts, identity, native, split_atoms

spec = importlib.util.spec_from_file_location("compare_optimal", SCRIPTS / "compare-optimal.py")
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


def result(output, code=0):
    return subprocess.CompletedProcess([], code, output, "")


def reference(models, costs=None, optimal=None):
    witnesses = [{"Value": model, **({"Costs": cost} if cost is not None else {})}
                 for model, cost in models]
    summary = {"Number": len(models), "More": "no"}
    if costs is not None:
        summary.update({"Costs": costs, "Optimal": optimal, "Optimum": "yes"})
    status = "OPTIMUM FOUND" if costs is not None else "SATISFIABLE" if models else "UNSATISFIABLE"
    return {"Result": status, "Models": summary, "Call": [{"Witnesses": witnesses}]}


def native_output(models, cost=None):
    text = ""
    for index, model in enumerate(models, 1):
        text += f"Answer: {index}\n" + " ".join(model) + "\n"
        if cost is not None:
            text += "Optimization:" + "".join(f" {value}" for value in cost) + "\n"
    text += ("OPTIMUM FOUND" if cost is not None else "SATISFIABLE" if models else "UNSATISFIABLE") + "\n"
    return text + f"Coverage: exhausted\nModels: {len(models)}; candidates examined: 5\n"


class ContractTests(unittest.TestCase):
    def test_optn_removes_one_replay_and_preserves_hidden_tie_multiplicity(self):
        doc = reference([(["worse"], [2]), (["shown"], [1]), (["shown"], [1]), (["shown"], [1])], [1], 2)
        answer = clingo(result(json.dumps(doc), 30))
        self.assertEqual(answer["models"], [["shown"], ["shown"]])
        self.assertEqual(answer["model_count"], 2)
        self.assertEqual(answer["raw_witness_count"], 4)
        self.assertEqual(identity(answer), identity(native(result(native_output([["shown"], ["shown"]], [1])))))

    def test_empty_displays_and_duplicate_symbols_are_not_deduplicated(self):
        for models in [[[], []], [["a", "a"], ["a"]]]:
            doc = reference([(models[0], [0])] + [(model, [0]) for model in models], [0], len(models))
            self.assertEqual(identity(clingo(result(json.dumps(doc), 30))), identity(native(result(native_output(models, [0])))))
        self.assertNotEqual(split_atoms("a a"), split_atoms("a"))

    def test_quoted_symbols_preserve_spaces_escapes_and_literal_newlines(self):
        symbols = ['p("two words")', 'q("a\\\"b")', 'r("a\\\\b")', 'n("one\nAnswer: 42\ntwo")', 't("x\ty")']
        actual = native(result(native_output([symbols], [-2, 0])))
        doc = reference([(symbols, [-2, 0]), (symbols, [-2, 0])], [-2, 0], 1)
        self.assertEqual(identity(actual), identity(clingo(result(json.dumps(doc), 30))))
        self.assertEqual(split_atoms('a(1,2), b("x,y")', comma=True), ['a(1,2)', 'b("x,y")'])

    def test_unsat_and_objective_free_models_share_the_companion_contract(self):
        for models in [[], [[]], [["b"], ["a"], ["a"]]]:
            doc = reference([(model, None) for model in models])
            code = 10 if models else 20
            self.assertEqual(identity(clingo(result(json.dumps(doc), code))), identity(native(result(native_output(models)))))

    def test_invalid_reference_counts_costs_and_replay_are_refused(self):
        valid = reference([(["a"], [1]), (["a"], [1])], [1], 1)
        alterations = [
            lambda doc: doc["Models"].update(More="yes"),
            lambda doc: doc["Models"].update(Number=9),
            lambda doc: doc["Models"].update(Optimal=2),
            lambda doc: doc["Models"].update(Optimum="no"),
            lambda doc: doc["Models"].update(Costs=[True]),
            lambda doc: doc["Call"][0]["Witnesses"][0].update(Costs=[0]),
            lambda doc: doc["Call"][0]["Witnesses"][0].update(Costs=[1, 0]),
            lambda doc: doc["Call"][0]["Witnesses"][0].update(Value=["unreplayed"]),
        ]
        for alter in alterations:
            doc = json.loads(json.dumps(valid))
            alter(doc)
            with self.assertRaises(ValueError):
                clingo(result(json.dumps(doc), 30))

    def test_nonobject_reference_shapes_are_typed_refusals(self):
        for invalid in (None, [], 1, True, "object"):
            for location in ("output", "Models", "Call entry", "witness"):
                with self.subTest(location=location, invalid=invalid):
                    doc = reference([(["a"], None)])
                    if location == "output":
                        doc = invalid
                    elif location == "Models":
                        doc["Models"] = invalid
                    elif location == "Call entry":
                        doc["Call"] = [invalid]
                    else:
                        doc["Call"][0]["Witnesses"] = [invalid]
                    with self.assertRaisesRegex(ValueError, "must be a JSON object"):
                        clingo(result(json.dumps(doc), 10))

    def test_invalid_native_status_count_and_optimization_are_refused(self):
        valid = native_output([["a"]], [1])
        malformed = [valid.replace("exhausted", "partial"), valid.replace("Models: 1", "Models: 2"),
                     valid + "SATISFIABLE\n", valid + "Coverage: exhausted\n", valid + "INCOMPLETE: timeout\n",
                     valid.replace("Optimization: 1\n", ""), valid.replace("OPTIMUM FOUND", "SATISFIABLE"),
                     valid.replace("Optimization: 1", "Optimization: 1\nOptimization: 1"),
                     native_output([["a"], ["b"]], [1]).replace("Optimization: 1", "Optimization: 2", 1)]
        for text in malformed:
            with self.assertRaises(ValueError):
                native(result(text))

    def test_original_manifest_contracts_use_full_tie_count_and_shown_models(self):
        answer = {"satisfiable": True, "cost": [-2, 0], "model_count": 2,
                  "models": [["a(1,2)", 'b("x,y")'], ["a(1,2)", 'b("x,y")']]}
        entries = [{"tag": tag, "arguments": value} for tag, value in [
            ("expect", "sat"), ("cost", "{-2,0}"), ("count", "optimal 2"),
            ("optimal", '{a(1,2),b("x,y")}'), ("cautious", "optimal {a(1,2)}"), ("note", "retained evidence")]]
        contracts(answer, entries)
        with self.assertRaises(ValueError):
            contracts(answer, [{"tag": "count", "arguments": "1"}])
        with self.assertRaises(ValueError):
            contracts(answer, [{"tag": "new-unsupported", "arguments": ""}])

    def test_historical_objective_free_send_records_still_normalize(self):
        document = json.loads((SCRIPTS.parent / "docs/verification/send-money-20260905/comparison.json").read_text())
        for entry in document["runs"]:
            answer = (clingo if entry["solver"] == "clingo" else native)(subprocess.CompletedProcess([], entry["returncode"], entry["stdout"], entry["stderr"]))
            self.assertEqual(answer["models"], document["models"])
            self.assertIsNone(answer["cost"])


class ProcessAndCatalogTests(unittest.TestCase):
    def test_terminal_summary_omits_payloads_without_changing_full_evidence(self):
        report = {"status": "incomplete", "summary": {"zetesis": {"median": 1}},
                  "answer": {"satisfiable": True, "cost": [-2, 0], "model_count": 2,
                             "models": [["private-model-payload"], ["private-model-payload"]]},
                  "failure": {"reason": "output byte limit", "solver": "clingo", "returncode": -9,
                              "stdout": "large-stdout-payload", "stderr": "large-stderr-payload"}}
        original = json.loads(json.dumps(report))
        summary = runner.terminal_summary(report)
        self.assertEqual(summary["answer"], {"satisfiable": True, "cost": [-2, 0], "model_count": 2})
        self.assertEqual(summary["summary"], report["summary"])
        self.assertEqual(summary["failure"], {"reason": "output byte limit", "solver": "clingo", "returncode": -9})
        self.assertNotIn("payload", json.dumps(summary))
        self.assertEqual(report, original)

    def test_bounded_capture_has_no_solver_dependency(self):
        output, elapsed = runner.bounded([sys.executable, "-c", "import sys; print('ok'); print('diagnostic',file=sys.stderr)"], 5, 1024)
        self.assertEqual(output.stdout, "ok\n")
        self.assertEqual(output.stderr, "diagnostic\n")
        self.assertGreaterEqual(elapsed, 0)

    def test_output_limit_and_timeout_produce_incomplete_evidence(self):
        with self.assertRaises(runner.ProcessFailure) as output:
            runner.bounded([sys.executable, "-c", "print('x'*100000)"], 5, 32)
        self.assertEqual(output.exception.evidence["reason"], "output byte limit")
        self.assertLessEqual(len(output.exception.evidence["stdout"]) + len(output.exception.evidence["stderr"]), 32)
        with self.assertRaises(runner.ProcessFailure) as timeout:
            runner.bounded([sys.executable, "-c", "import time; time.sleep(10)"], 0.05, 1024)
        self.assertIn("timeout", timeout.exception.evidence["reason"])

    def test_original_transitive_graph_hashes_are_checked(self):
        manifest = SCRIPTS.parent / "docs/verification/kr-domains-target-manifest.json"
        corpus = (SCRIPTS.parent / "validation/corpus/kr-domains").resolve()
        case, sources, size = runner.source_catalog(manifest, corpus, "scenarios/task-allocation/variant-04/05-larger-mix.lp")
        self.assertEqual(len(sources), len(case["transitive_source_paths"]))
        self.assertGreater(size, 0)
        with self.assertRaises(ValueError):
            runner.source_catalog(manifest, corpus, case["path"], max_files=0)
        with self.assertRaises(ValueError):
            runner.source_catalog(manifest, corpus, case["path"], max_bytes=0)

    def test_full_report_protocol_with_fake_binaries_and_no_solver_execution(self):
        manifest = json.loads((SCRIPTS.parent / "docs/verification/kr-domains-target-manifest.json").read_text())
        case = next(case for case in manifest["cases"] if case["expected_satisfiability"] == "unsat")
        with tempfile.TemporaryDirectory() as temporary:
            temporary = Path(temporary)
            paths = {}
            for name in ("zetesis", "clingo"):
                path = temporary / name
                text = json.dumps(reference([])) if name == "clingo" else native_output([])
                code = 20 if name == "clingo" else 0
                path.write_text(f"#!{sys.executable}\nimport sys\nif sys.argv[1] in ('--help','--version'):\n print('synthetic protocol fixture')\n sys.exit(0)\nprint({text!r})\nsys.exit({code})\n")
                path.chmod(0o700)
                paths[name] = str(path)
            report = temporary / "report.json"
            command = [sys.executable, str(SCRIPTS / "compare-optimal.py"), "--case", case["path"],
                       "--zetesis", paths["zetesis"], "--clingo", paths["clingo"], "--runs", "4",
                       "--warmups", "0", "--memory-runs", "0", "--timeout", "5", "--report", str(report)]
            completed = subprocess.run(command, capture_output=True, text=True, timeout=15, check=False)
            self.assertEqual(completed.returncode, 0, completed.stderr)
            document = json.loads(report.read_text())
            self.assertEqual(document["status"], "complete")
            self.assertEqual(document["sha256_before"], document["sha256_after"])
            self.assertEqual(len(document["runs"]), 10)
            self.assertEqual(document["answer"]["models"], [])
            timed = [entry["solver"] for entry in document["runs"] if entry["phase"] == "timed"]
            self.assertEqual(timed, ["zetesis", "clingo", "clingo", "zetesis"] * 2)
            self.assertIn("--parallel-mode=1", document["commands"]["clingo"])
            self.assertTrue(document["model_parity"])
            terminal = json.loads(completed.stdout)
            self.assertEqual(terminal["answer"], {"satisfiable": False, "cost": None, "model_count": 0})
            self.assertNotIn("models", terminal["answer"])

    def test_malformed_reference_still_writes_incomplete_report(self):
        manifest = json.loads((SCRIPTS.parent / "docs/verification/kr-domains-target-manifest.json").read_text())
        case = next(case for case in manifest["cases"] if case["expected_satisfiability"] == "unsat")
        with tempfile.TemporaryDirectory() as temporary:
            temporary = Path(temporary)
            paths = {}
            for name in ("zetesis", "clingo"):
                path = temporary / name
                text = "null" if name == "clingo" else native_output([])
                code = 20 if name == "clingo" else 0
                path.write_text(f"#!{sys.executable}\nimport sys\nif sys.argv[1] in ('--help','--version'):\n print('synthetic malformed-output fixture')\n sys.exit(0)\nprint({text!r})\nsys.exit({code})\n")
                path.chmod(0o700)
                paths[name] = str(path)
            report = temporary / "report.json"
            command = [sys.executable, str(SCRIPTS / "compare-optimal.py"), "--case", case["path"],
                       "--zetesis", paths["zetesis"], "--clingo", paths["clingo"], "--runs", "4",
                       "--warmups", "0", "--memory-runs", "0", "--timeout", "5", "--report", str(report)]
            completed = subprocess.run(command, capture_output=True, text=True, timeout=15, check=False)
            self.assertEqual(completed.returncode, 1, completed.stderr)
            document = json.loads(report.read_text())
            self.assertEqual(document["status"], "incomplete")
            self.assertIn("must be a JSON object", document["failure"]["reason"])
            self.assertEqual(document["runs"][-1]["stdout"], "null\n")
            self.assertNotIn("summary", document)
            self.assertNotIn("model_parity", document)
            self.assertEqual(json.loads(completed.stdout)["status"], "incomplete")

    def test_memory_wrapper_reports_child_only_units_without_running_a_solver(self):
        with tempfile.TemporaryDirectory() as temporary:
            report = Path(temporary) / "rss.json"
            result, _ = runner.bounded([sys.executable, str(SCRIPTS / "comparison_memory.py"), str(report), sys.executable, "-c", "print('child')"], 5, 1024)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "child\n")
            memory = json.loads(report.read_text())
            self.assertGreater(memory["peak_rss_bytes"], 0)
            self.assertIn(memory["raw_unit"], ("KiB", "bytes"))


if __name__ == "__main__":
    unittest.main()
