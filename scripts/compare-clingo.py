#!/usr/bin/env python3
"""Measure complete, objective-free CLI solves; clingo is an external oracle only.

Use an unchanged, self-contained input. Timings include process startup, parsing,
grounding, solving and captured output. This runner does not flush OS caches.
"""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import re
import shutil
import statistics
import subprocess
import sys
import time


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def executable(name):
    resolved = shutil.which(name)
    if resolved is None:
        raise ValueError(f"executable not found: {name}")
    return str(Path(resolved).resolve())


def atoms(line):
    """Split printed symbols without splitting spaces inside quoted strings."""
    result, start, quoted, escaped, depth = [], 0, False, False, 0
    for index, char in enumerate(line):
        if quoted:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                quoted = False
        elif char == '"':
            quoted = True
        elif char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
        elif char.isspace() and depth == 0:
            if index > start:
                result.append(line[start:index])
            start = index + 1
    if quoted or depth != 0:
        raise ValueError("malformed printed model")
    if start < len(line):
        result.append(line[start:])
    return sorted(result)


def validate(name, completed):
    if name == "clingo":
        if completed.returncode not in (10, 20, 30):
            raise ValueError(f"clingo failed: {completed.returncode}: {completed.stderr}")
        output = json.loads(completed.stdout)
        if output["Result"] not in ("SATISFIABLE", "UNSATISFIABLE"):
            raise ValueError("only complete objective-free solves are supported")
        if output["Models"]["More"] != "no":
            raise ValueError("clingo did not exhaust search")
        witnesses = [w for call in output["Call"] for w in call.get("Witnesses", [])]
        if any("Costs" in witness for witness in witnesses):
            raise ValueError("optimization is outside this runner's contract")
        models = sorted(sorted(w["Value"]) for w in witnesses)
        if len(models) != output["Models"]["Number"]:
            raise ValueError("clingo witness count disagrees with reported count")
        return models
    if completed.returncode != 0:
        raise ValueError(f"zetesis failed: {completed.returncode}: {completed.stderr}")
    lines = completed.stdout.splitlines()
    if "Coverage: exhausted" not in lines or "INCOMPLETE" in lines:
        raise ValueError("zetesis did not exhaust search")
    if "OPTIMUM FOUND" in lines or any(line.startswith("Optimization:") for line in lines):
        raise ValueError("optimization is outside this runner's contract")
    models = sorted(atoms(lines[i + 1]) for i, line in enumerate(lines) if line.startswith("Answer:"))
    count = re.search(r"^Models: (\d+)", completed.stdout, re.MULTILINE)
    if count is None or len(models) != int(count.group(1)):
        raise ValueError("zetesis witness count disagrees with reported count")
    return models


def summarize(values):
    quartiles = statistics.quantiles(values, n=4, method="inclusive")
    return {"n": len(values), "median": statistics.median(values), "min": min(values),
            "max": max(values), "q1": quartiles[0], "q3": quartiles[2]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("--zetesis", required=True)
    parser.add_argument("--clingo", default="clingo")
    parser.add_argument("--runs", type=int, default=21)
    parser.add_argument("--warmups", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if args.runs < 4 or args.warmups < 0 or args.timeout <= 0:
        parser.error("require runs >= 4, warmups >= 0, timeout > 0")
    source = args.input.resolve()
    binaries = {"zetesis": executable(args.zetesis), "clingo": executable(args.clingo)}
    commands = {
        "zetesis": [binaries["zetesis"], str(source), "--models", "0", "--backend", "cpu", "--workers", "1"],
        "clingo": [binaries["clingo"], str(source), "0", "--outf=2", "--warn=none", "--parallel-mode=1"],
    }
    hashes = {name: digest(path) for name, path in {**binaries, "input": source}.items()}
    report = {
        "schema": 1, "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "platform": platform.platform(), "machine": platform.machine(),
        "processor": platform.processor(), "logical_cpus": os.cpu_count(),
        "input": str(source), "sha256": hashes, "commands": commands,
        "protocol": {"runs_per_solver": args.runs, "warmup_pairs": args.warmups,
                     "order": "seeded shuffle within each pair", "seed": 20260905,
                     "timeout_seconds": args.timeout, "cache_flush": False,
                     "timing": "perf_counter_ns around direct subprocess.run; output captured",
                     "memory": "separate fresh Python parent per solve; resource.getrusage(RUSAGE_CHILDREN).ru_maxrss, bytes on macOS; wrapper excluded from RSS",
                     "scope": "complete objective-free displayed model multiset; one CPU worker"},
        "versions": {}, "runs": [],
    }
    for name, binary in binaries.items():
        report["versions"][name] = subprocess.run([binary, "--version"], capture_output=True, text=True, check=True).stdout
    rng = random.Random(20260905)
    expected = None

    def run_pair(phase, number, memory=False):
        nonlocal expected
        order = list(commands)
        rng.shuffle(order)
        for name in order:
            memory_wrapper = [sys.executable, "-c",
                              "import resource, subprocess, sys; "
                              "p = subprocess.run(sys.argv[1:]); "
                              "print('BENCH_PEAK_RSS_BYTES=' + str(resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss), file=sys.stderr); "
                              "sys.exit(p.returncode)"]
            command = (memory_wrapper if memory else []) + commands[name]
            started = time.perf_counter_ns()
            completed = subprocess.run(command, capture_output=True, text=True, timeout=args.timeout)
            elapsed = (time.perf_counter_ns() - started) / 1e9
            models = validate(name, completed)
            if expected is None:
                expected = models
            if models != expected:
                raise ValueError(f"model parity failure for {name} {phase} {number}")
            entry = {"solver": name, "phase": phase, "pair": number, "seconds": elapsed,
                     "returncode": completed.returncode, "stdout": completed.stdout,
                     "stderr": completed.stderr, "models": models}
            if memory:
                match = re.search(r"BENCH_PEAK_RSS_BYTES=(\d+)", completed.stderr)
                if match is None:
                    raise ValueError("macOS peak RSS missing")
                entry["peak_rss_bytes"] = int(match.group(1))
                entry["measurement_command"] = command
            report["runs"].append(entry)

    run_pair("first_observed", 0)
    for index in range(args.warmups):
        run_pair("warmup", index)
    for index in range(args.runs):
        run_pair("timed", index)
    if platform.system() == "Darwin":
        for index in range(5):
            run_pair("memory", index, memory=True)
    if hashes != {name: digest(path) for name, path in {**binaries, "input": source}.items()}:
        raise ValueError("input or executable changed during measurement")
    report["model_parity"] = True
    report["models"] = expected
    report["summary"] = {}
    for name in commands:
        entries = [run for run in report["runs"] if run["solver"] == name]
        summary = {"wall_seconds": summarize([run["seconds"] for run in entries if run["phase"] == "timed"])}
        memory = [run["peak_rss_bytes"] for run in entries if run["phase"] == "memory"]
        if memory:
            summary["peak_rss_bytes"] = summarize(memory)
        report["summary"][name] = summary
    report["zetesis_to_clingo_median_wall_ratio"] = report["summary"]["zetesis"]["wall_seconds"]["median"] / report["summary"]["clingo"]["wall_seconds"]["median"]
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"summary": report["summary"], "ratio": report["zetesis_to_clingo_median_wall_ratio"], "models": expected}, indent=2))


if __name__ == "__main__":
    main()
