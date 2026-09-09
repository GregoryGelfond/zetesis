#!/usr/bin/env python3
"""Bounded paired end-to-end CPU comparison, preserving optimal display multisets.

The original corpus manifest supplies byte-exact include closure and original
contracts. clingo optN and native exhausted output must agree before any timing
summary is accepted. The historical compare-clingo.py protocol is unchanged.
"""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import selectors
import shutil
import signal
import statistics
import subprocess
import sys
import tempfile
import time

from comparison_contract import clingo, contracts, identity, native

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    result = hashlib.sha256()
    with Path(path).open("rb") as source:
        for block in iter(lambda: source.read(1 << 20), b""):
            result.update(block)
    return result.hexdigest()


def executable(name):
    resolved = shutil.which(name)
    if resolved is None:
        raise ValueError(f"executable not found: {name}")
    return str(Path(resolved).resolve())


def source_catalog(manifest_path, corpus, case_path, max_files=256, max_bytes=16_777_216):
    if manifest_path.stat().st_size > 4_194_304:
        raise ValueError("manifest exceeds 4 MiB")
    manifest = json.loads(manifest_path.read_text())
    matches = [case for case in manifest["cases"] if case["path"] == case_path]
    if len(matches) != 1:
        raise ValueError("case must identify exactly one original manifest entry")
    case = matches[0]
    registry = {}
    for entry in manifest["cases"] + manifest["open_encodings"]:
        for record in [entry] + entry.get("includes", []):
            name, expected = record["path"], record["sha256"]
            if name in registry and registry[name] != expected:
                raise ValueError("contradictory source hashes in manifest")
            registry[name] = expected
    required = case["transitive_source_paths"]
    if not required or len(required) > max_files or len(set(required)) != len(required):
        raise ValueError("empty, duplicate or excessive original source catalog")
    if case_path not in required:
        raise ValueError("include closure does not contain its entry source")
    catalog, total = {}, 0
    for relative in required:
        lexical = Path(relative)
        path = (corpus / lexical).resolve()
        if lexical.is_absolute() or ".." in lexical.parts or not path.is_relative_to(corpus):
            raise ValueError("manifest source escapes the supplied corpus")
        total += path.stat().st_size
        if total > max_bytes:
            raise ValueError("original source byte allowance exceeded")
        actual = digest(path)
        if registry.get(relative) != actual:
            raise ValueError(f"original source hash mismatch: {relative}")
        catalog[f"source:{relative}"] = path
    return case, catalog, total


class ProcessFailure(ValueError):
    def __init__(self, reason, stdout, stderr, returncode):
        super().__init__(reason)
        self.evidence = {"reason": reason, "stdout": stdout, "stderr": stderr,
                         "returncode": returncode}


def bounded(command, timeout, max_output_bytes, cwd=None):
    """Bound captured bytes and kill only the process group created by this call."""
    if os.name != "posix":
        raise ValueError("bounded process-group cleanup currently requires POSIX")
    captured = {"stdout": bytearray(), "stderr": bytearray()}
    started = time.perf_counter_ns()
    child = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                             stderr=subprocess.PIPE, cwd=cwd, start_new_session=True)
    failure = None
    try:
        with selectors.DefaultSelector() as selector:
            for name, stream in (("stdout", child.stdout), ("stderr", child.stderr)):
                os.set_blocking(stream.fileno(), False)
                selector.register(stream, selectors.EVENT_READ, name)
            while selector.get_map():
                remaining = timeout - (time.perf_counter_ns() - started) / 1e9
                if remaining <= 0:
                    failure = "timeout"
                    break
                for key, _ in selector.select(min(remaining, 0.05)):
                    room = max_output_bytes - sum(map(len, captured.values()))
                    block = os.read(key.fileobj.fileno(), min(65_536, room + 1))
                    if not block:
                        selector.unregister(key.fileobj)
                        continue
                    if len(block) > room:
                        captured[key.data].extend(block[:room])
                        failure = "output byte limit"
                        break
                    captured[key.data].extend(block)
                if failure:
                    break
            if not failure:
                try:
                    child.wait(timeout=max(0.001, timeout - (time.perf_counter_ns() - started) / 1e9))
                except subprocess.TimeoutExpired:
                    failure = "timeout after output closed"
            if failure:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                child.wait()
    except BaseException:
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        child.wait()
        child.stdout.close()
        child.stderr.close()
        raise
    child.stdout.close()
    child.stderr.close()
    seconds = (time.perf_counter_ns() - started) / 1e9
    try:
        stdout, stderr = (bytes(captured[name]).decode("utf-8") for name in ("stdout", "stderr"))
    except UnicodeDecodeError as error:
        raise ProcessFailure("non-UTF-8 solver output", repr(captured["stdout"]),
                             repr(captured["stderr"]), child.returncode) from error
    if failure:
        raise ProcessFailure(failure, stdout, stderr, child.returncode)
    return subprocess.CompletedProcess(command, child.returncode, stdout, stderr), seconds


def summarize(values):
    quartiles = statistics.quantiles(values, n=4, method="inclusive")
    return {"n": len(values), "median": statistics.median(values), "min": min(values),
            "max": max(values), "q1": quartiles[0], "q3": quartiles[2]}


def configuration():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", required=True, help="original manifest-relative input path")
    parser.add_argument("--corpus-root", type=Path, default=ROOT / "validation/corpus/kr-domains")
    parser.add_argument("--manifest", type=Path, default=ROOT / "validation/corpus/manifest.json")
    parser.add_argument("--zetesis", required=True)
    parser.add_argument("--clingo", default="clingo")
    parser.add_argument("--runs", type=int, default=21)
    parser.add_argument("--warmups", type=int, default=3)
    parser.add_argument("--memory-runs", type=int, default=5)
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("--max-output-bytes", type=int, default=4_194_304)
    parser.add_argument("--max-captured-bytes", type=int, default=67_108_864, help="cumulative retained stdout/stderr allowance")
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if not 4 <= args.runs <= 1000 or not 0 <= args.warmups <= 1000 or not (args.memory_runs == 0 or 4 <= args.memory_runs <= 1000):
        parser.error("runs 4..1000, warmups 0..1000, memory-runs 0 or 4..1000")
    if not 0 < args.timeout <= 86_400 or not 0 < args.max_output_bytes <= 67_108_864:
        parser.error("timeout must be in (0,86400], output allowance in (0,64 MiB]")
    if not 0 < args.max_captured_bytes <= 1_073_741_824:
        parser.error("cumulative captured output allowance must be in (0,1 GiB]")
    args.corpus_root = args.corpus_root.resolve()
    args.manifest = args.manifest.resolve()
    return args


def terminal_summary(report):
    """Keep model and diagnostic payloads in the full report, not terminal output."""
    summary = {key: report[key] for key in
               ("status", "summary", "model_parity", "zetesis_to_clingo_median_wall_ratio")
               if key in report}
    if "answer" in report:
        summary["answer"] = {key: report["answer"][key]
                             for key in ("satisfiable", "cost", "model_count")}
    if "failure" in report:
        summary["failure"] = {key: report["failure"][key]
                              for key in ("reason", "solver", "phase", "pair", "returncode")
                              if key in report["failure"]}
    return summary


def capture_configuration(name, binary, execute, evidence):
    """Retain successful version/help queries through the caller's bounded runner.

    Native short help may advertise a separate full view. Query it only when
    advertised, retaining the old help protocol for older binaries. At most
    three queries use the same per-process and cumulative capture allowances.
    A failed query is retained and propagated; it never falls back to an
    incomplete defaults record or starts the measured pairs.
    """
    def query(option):
        completed, _ = execute([binary, option])
        evidence[f"{name} {option}"] = {"stdout": completed.stdout, "stderr": completed.stderr}
        if completed.returncode != 0:
            raise ValueError(f"{name} {option} failed")
        return completed

    query("--version")
    help_result = query("--help")
    option = "--help"
    if name == "zetesis" and any("--help-all" in re.sub(r"\x1b\[[0-9;]*m", "", text).split()
                                  for text in (help_result.stdout, help_result.stderr)):
        option = "--help-all"
        query(option)
    return option


def main():
    args = configuration()
    case, sources, total_bytes = source_catalog(args.manifest, args.corpus_root, args.case)
    binaries = {"zetesis": executable(args.zetesis), "clingo": executable(args.clingo)}
    commands = {
        "zetesis": [binaries["zetesis"], str(sources[f"source:{args.case}"]), "--models", "0",
                    "--backend", "cpu", "--workers", "1", "--oracle", "auto", "--grounder", "auto"],
        "clingo": [binaries["clingo"], str(sources[f"source:{args.case}"]), "0", "--outf=2",
                   "--opt-mode=optN", "--warn=none", "--parallel-mode=1"],
    }
    files = {**sources, **binaries, "manifest": args.manifest,
             "runner": Path(__file__).resolve(), "contract_parser": Path(__file__).with_name("comparison_contract.py").resolve(),
             "memory_wrapper": Path(__file__).with_name("comparison_memory.py").resolve(),
             "python": Path(sys.executable).resolve()}
    if args.report.resolve() in {Path(path).resolve() for path in files.values()}:
        raise ValueError("report must not overwrite a source, manifest, binary or script")
    before = {name: {"path": str(path), "sha256": digest(path)} for name, path in files.items()}
    report = {
        "schema": 2, "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "platform": platform.platform(), "machine": platform.machine(), "logical_cpus": os.cpu_count(),
        "python_version": sys.version, "processor": platform.processor(),
        "case": case, "source_bytes": total_bytes, "sha256_before": before, "commands": commands,
        "working_directory": str(args.corpus_root), "status": "running", "runs": [],
        "protocol": {"runs_per_solver": args.runs, "warmup_pairs": args.warmups, "memory_pairs": args.memory_runs,
                     "order": "balanced alternating first solver in successive pairs within each phase",
                     "first_pair_order": ["zetesis", "clingo"], "timeout_seconds": args.timeout,
                     "max_output_bytes": args.max_output_bytes,
                     "max_captured_bytes": args.max_captured_bytes, "cache_flush": False,
                     "timing": "perf_counter_ns around direct Popen and bounded selector capture; includes startup, parsing, grounding, solving and output",
                     "memory": "separate fresh Python parent; child getrusage peak RSS, excluding wrapper; macOS bytes or Linux KiB converted to bytes",
                     "scope": "complete optimal or objective-free displayed model MULTISET, raw full tie count and complete objective vector; one CPU worker/thread",
                     "hidden_atoms": "not reconstructed from #show; original manifest contracts and raw full model counts checked",
                     "native_limits": "native default evidence has not completed; no limit override",
                     "original_contracts": "checked from byte-pinned manifest before and during measured phases"},
        "configuration_evidence": {},
    }
    expected = None
    captured_bytes = 0
    def execute(command, cwd=None):
        nonlocal captured_bytes
        allowance = min(args.max_output_bytes, args.max_captured_bytes - captured_bytes)
        if allowance <= 0:
            raise ValueError("cumulative captured output limit")
        completed, seconds = bounded(command, args.timeout, allowance, cwd)
        captured_bytes += len(completed.stdout.encode()) + len(completed.stderr.encode())
        return completed, seconds
    try:
        for name, binary in binaries.items():
            option = capture_configuration(name, binary, execute, report["configuration_evidence"])
            if name == "zetesis":
                report["protocol"]["native_limits"] = f"binary defaults captured in native {option} output; no limit override"
        def pair(phase, number):
            nonlocal expected
            names = ["zetesis", "clingo"] if number % 2 == 0 else ["clingo", "zetesis"]
            for name in names:
                with tempfile.TemporaryDirectory(prefix="zetesis-comparison-") as temporary:
                    memory = phase == "memory"
                    rss_path = Path(temporary) / "rss.json"
                    command = ([sys.executable, str(files["memory_wrapper"]), str(rss_path)] if memory else []) + commands[name]
                    try:
                        result, seconds = execute(command, args.corpus_root)
                    except ProcessFailure as error:
                        error.evidence.update(solver=name, phase=phase, pair=number, command=command)
                        raise
                    entry = {"solver": name, "phase": phase, "pair": number, "seconds": seconds,
                             "returncode": result.returncode, "stdout": result.stdout, "stderr": result.stderr,
                             "measurement_command": command,
                             "phase_profile_evidence": result.stderr.splitlines() if name == "zetesis" else []}
                    report["runs"].append(entry)
                    answer = (clingo if name == "clingo" else native)(result)
                    entry["answer"] = answer
                    contracts(answer, case["contracts"])
                    actual = identity(answer)
                    if expected is None:
                        expected = actual
                    if actual != expected:
                        raise ValueError(f"complete answer contract mismatch: {name}, {phase}, {number}")
                    if memory:
                        if rss_path.stat().st_size > 4096:
                            raise ValueError("excessive memory-wrapper record")
                        entry["memory"] = json.loads(rss_path.read_text())
                        peak = entry["memory"].get("peak_rss_bytes")
                        if type(peak) is not int or peak < 0:
                            raise ValueError("invalid child peak RSS record")
        pair("first_observed", 0)
        for index in range(args.warmups):
            pair("warmup", index)
        for index in range(args.runs):
            pair("timed", index)
        for index in range(args.memory_runs):
            pair("memory", index)
        report["answer"] = expected
        report["status"] = "complete"
    except (ValueError, OSError, KeyError, TypeError) as error:
        report["status"] = "incomplete"
        report["failure"] = getattr(error, "evidence", {"reason": str(error)})
    finally:
        try:
            report["sha256_after"] = {name: {"path": str(path), "sha256": digest(path)} for name, path in files.items()}
            if report["sha256_after"] != before:
                report["status"] = "incomplete"
                report["integrity_failure"] = "source, manifest, dependency, binary or benchmark script changed"
        except OSError as error:
            report["status"] = "incomplete"
            report["integrity_failure"] = str(error)
    if report["status"] == "complete":
        report["model_parity"] = True
        report["summary"] = {}
        for name in commands:
            entries = [run for run in report["runs"] if run["solver"] == name]
            summary = {"wall_seconds": summarize([run["seconds"] for run in entries if run["phase"] == "timed"])}
            memory = [run["memory"]["peak_rss_bytes"] for run in entries if run["phase"] == "memory"]
            if memory:
                summary["peak_rss_bytes"] = summarize(memory)
            report["summary"][name] = summary
        report["zetesis_to_clingo_median_wall_ratio"] = report["summary"]["zetesis"]["wall_seconds"]["median"] / report["summary"]["clingo"]["wall_seconds"]["median"]
    report["retained_stdout_stderr_bytes"] = captured_bytes
    report["finished_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(terminal_summary(report), indent=2))
    return 0 if report["status"] == "complete" else 1


if __name__ == "__main__":
    sys.exit(main())
