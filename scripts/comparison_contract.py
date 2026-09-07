"""Strict completed displayed-model contracts for compare-optimal.py.

This preserves multiplicities both within a displayed model and between models.
It cannot reconstruct atoms hidden by #show; the raw optimal/full model count is
checked independently of display identity. Integer vectors use signed i64 slots.
"""

import json
import re


def integer(value):
    if type(value) is not int or not -(1 << 63) <= value < (1 << 63):
        raise ValueError("cost is outside the signed 64-bit report contract")
    return value


def cost_vector(value):
    if not isinstance(value, list):
        raise ValueError("missing cost vector")
    return [integer(cost) for cost in value]


def natural(value):
    if type(value) is not int or not 0 <= value < (1 << 64):
        raise ValueError("missing or invalid unsigned model count")
    return value


def split_atoms(text, comma=False):
    """Tokenize a complete printed model; literal newlines in strings survive."""
    result, start, quoted, escaped, depth = [], 0, False, False, 0
    for index, char in enumerate(text):
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
            if depth < 0:
                raise ValueError("unexpected closing parenthesis in model")
        elif depth == 0 and (char.isspace() or comma and char == ","):
            if index > start:
                result.append(text[start:index])
            start = index + 1
    if quoted or depth:
        raise ValueError("incomplete quoted symbol or parentheses")
    if start < len(text):
        result.append(text[start:])
    return sorted(result)


def displayed(value):
    if not isinstance(value, list) or any(not isinstance(atom, str) for atom in value):
        raise ValueError("witness must contain a list of displayed symbol strings")
    return sorted(value)


def mapping(value, label):
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be a JSON object")
    return value


def clingo(completed):
    if completed.returncode not in (10, 20, 30):
        raise ValueError(f"clingo failed with exit {completed.returncode}")
    output = mapping(json.loads(completed.stdout), "clingo output")
    status = output.get("Result")
    if status not in ("SATISFIABLE", "UNSATISFIABLE", "OPTIMUM FOUND"):
        raise ValueError("clingo did not complete solving/optimization")
    summary = mapping(output.get("Models", {}), "clingo Models")
    if summary.get("More") != "no":
        raise ValueError("clingo did not exhaust enumeration")
    optimum = status == "OPTIMUM FOUND"
    cost = cost_vector(summary.get("Costs")) if optimum else None
    if optimum and summary.get("Optimum") != "yes":
        raise ValueError("missing completed clingo optimum evidence")
    if not optimum and "Costs" in summary:
        raise ValueError("unexpected nonoptimized summary cost")
    calls = output.get("Call")
    if not isinstance(calls, list):
        raise ValueError("missing clingo calls")
    witnesses = []
    for call in calls:
        call = mapping(call, "clingo Call entry")
        values = call.get("Witnesses", [])
        if not isinstance(values, list):
            raise ValueError("malformed clingo witnesses")
        witnesses.extend(values)
    if natural(summary.get("Number")) != len(witnesses):
        raise ValueError("raw clingo witness count mismatch")
    selected = []
    reached_best = False
    for witness in witnesses:
        witness = mapping(witness, "clingo witness")
        model = displayed(witness.get("Value"))
        if optimum:
            actual = cost_vector(witness.get("Costs"))
            if len(actual) != len(cost) or actual < cost:
                raise ValueError("witness contradicts final optimum cost")
            if actual != cost:
                if reached_best:
                    raise ValueError("worse incumbent after final-cost phase")
                continue
            reached_best = True
        elif "Costs" in witness:
            raise ValueError("unexpected objective cost in unoptimized witness")
        selected.append(model)
    count = natural(summary.get("Optimal" if optimum else "Number"))
    if optimum:
        # optN emits the final improving incumbent before enumerating all optimal
        # models. Remove exactly that one occurrence, never deduplicate displays.
        if len(selected) != count + 1 or count == 0:
            raise ValueError("optN requires optimal witnesses plus one incumbent replay")
        replay = selected.pop(0)
        if replay not in selected:
            raise ValueError("final incumbent was not replayed in optimal enumeration")
    satisfiable = status != "UNSATISFIABLE"
    if satisfiable != bool(selected) or len(selected) != count:
        raise ValueError("clingo status/count/model contradiction")
    return {"satisfiable": satisfiable, "cost": cost, "model_count": count,
            "models": sorted(selected), "raw_witness_count": len(witnesses),
            "incumbent_replays_removed": int(optimum)}


def native(completed):
    if completed.returncode != 0:
        raise ValueError(f"zetesis failed with exit {completed.returncode}")
    lines = completed.stdout.split("\n")
    witnesses, metadata, index = [], [], 0
    while index < len(lines):
        line = lines[index]
        index += 1
        if line.startswith("Answer:"):
            if not re.fullmatch(r"Answer:\s*\d+", line) or index >= len(lines):
                raise ValueError("malformed/missing native answer")
            text = lines[index]
            index += 1
            while True:
                try:
                    model = split_atoms(text)
                    break
                except ValueError:
                    if index >= len(lines):
                        raise ValueError("unterminated native model") from None
                    text += "\n" + lines[index]
                    index += 1
            witnesses.append([model, None])
        elif line.startswith("Optimization:"):
            if not witnesses or witnesses[-1][1] is not None:
                raise ValueError("orphan or duplicate native cost")
            raw = line.removeprefix("Optimization:").split()
            if any(not re.fullmatch(r"-?\d+", value) for value in raw):
                raise ValueError("malformed native cost vector")
            witnesses[-1][1] = [integer(int(value)) for value in raw]
        else:
            metadata.append(line)
    if [line for line in metadata if line.startswith("Coverage:")] != ["Coverage: exhausted"]:
        raise ValueError("native coverage is not uniquely exhausted")
    if any(line.startswith("INCOMPLETE") for line in metadata):
        raise ValueError("native execution is incomplete")
    statuses = [line for line in metadata if line in ("SATISFIABLE", "UNSATISFIABLE", "OPTIMUM FOUND")]
    if len(statuses) != 1:
        raise ValueError("native status is absent or contradictory")
    summaries = [line for line in metadata if line.startswith("Models:")]
    if len(summaries) != 1:
        raise ValueError("native count is absent or contradictory")
    match = re.fullmatch(r"Models:\s*(\d+)(?:;.*)?", summaries[0])
    if match is None or natural(int(match.group(1))) != len(witnesses):
        raise ValueError("native witness count mismatch")
    status = statuses[0]
    satisfiable = status != "UNSATISFIABLE"
    if satisfiable != bool(witnesses):
        raise ValueError("native status/model contradiction")
    if status == "OPTIMUM FOUND":
        if any(cost is None for _, cost in witnesses):
            raise ValueError("native optimum needs every model cost")
        cost = witnesses[0][1]
        if any(actual != cost for _, actual in witnesses):
            raise ValueError("native optimum output includes a nonoptimal witness")
    else:
        cost = None
        if any(actual is not None for _, actual in witnesses):
            raise ValueError("objective records without completed native optimum")
    return {"satisfiable": satisfiable, "cost": cost, "model_count": len(witnesses),
            "models": sorted(model for model, _ in witnesses),
            "raw_witness_count": len(witnesses), "incumbent_replays_removed": 0}


def identity(answer):
    return {key: answer[key] for key in ("satisfiable", "cost", "model_count", "models")}


def braced(text):
    text = text.strip()
    if not text.startswith("{") or not text.endswith("}"):
        raise ValueError("original contract requires braces")
    return text[1:-1].strip()


def contracts(answer, entries):
    """Check the same original manifest contract tags as the native validator."""
    for entry in entries:
        tag, value = entry["tag"], entry["arguments"].strip()
        if tag == "note":
            continue
        if tag == "expect":
            if value not in ("sat", "unsat") or answer["satisfiable"] != (value == "sat"):
                raise ValueError("original @expect mismatch")
        elif tag == "cost":
            raw = braced(value).replace(",", " ").split()
            if answer["cost"] != [integer(int(item)) for item in raw]:
                raise ValueError("original @cost mismatch")
        elif tag == "count":
            if answer["model_count"] != natural(int(value.removeprefix("optimal").strip())):
                raise ValueError("original @count mismatch")
        elif tag in ("model", "optimal"):
            expected = split_atoms(braced(value), comma=True)
            if expected not in answer["models"]:
                raise ValueError(f"original @{tag} witness mismatch")
        elif tag == "cautious":
            if not value.startswith("optimal"):
                raise ValueError("unsupported cautious contract scope")
            expected = set(split_atoms(braced(value.removeprefix("optimal")), comma=True))
            if not answer["models"] or any(not expected.issubset(model) for model in answer["models"]):
                raise ValueError("original @cautious optimal mismatch")
        else:
            raise ValueError(f"unsupported original contract @{tag}")
