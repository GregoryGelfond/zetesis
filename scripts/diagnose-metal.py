#!/usr/bin/env python3
"""Compare native Metal discovery and installed zetesis; submit no GPU work."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("result_directory", type=Path, help="new evidence directory; never overwritten")
    parser.add_argument("--zetesis", default="zetesis", help="installed CLI name or path")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    dest = args.result_directory.resolve()
    dest.mkdir(parents=True, exist_ok=False)
    records = []

    def run(name, command):
        record = {"name": name, "command": command}
        try:
            result = subprocess.run(command, capture_output=True, timeout=45, check=False)
            stdout, stderr = result.stdout, result.stderr
            record.update(exit_code=result.returncode, termination="exited")
        except subprocess.TimeoutExpired as error:
            stdout, stderr = error.stdout or b"", error.stderr or b""
            record.update(exit_code=None, termination="timeout")
        except OSError as error:
            stdout, stderr = b"", str(error).encode()
            record.update(exit_code=None, termination="launch-error")
        (dest / f"{name}.stdout").write_bytes(stdout)
        (dest / f"{name}.stderr").write_bytes(stderr)
        records.append(record)
        (dest / "commands.json").write_text(json.dumps(records, indent=2) + "\n")
        return record, stdout

    (dest / "status.txt").write_text("INCOMPLETE: discovery has not finished.\n")
    metadata = {
        "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "scope": "adapter discovery only; no GPU command buffers or solver queries submitted",
        "platform": sys.platform,
        "relevant_environment": {k: v for k, v in os.environ.items()
                                 if k.startswith(("WGPU_", "MTL_", "METAL_"))},
    }
    source = root / "scripts/probes/metal-discovery.m"
    metadata["probe_source_sha256"] = hashlib.sha256(source.read_bytes()).hexdigest()
    shutil.copy2(source, dest / source.name)
    run("system", ["uname", "-a"])
    run("graphics", ["system_profiler", "SPDisplaysDataType", "-json"])
    run("compiler", ["xcrun", "--sdk", "macosx", "clang", "--version"])
    built, _ = run("compile", ["xcrun", "--sdk", "macosx", "clang", "-fobjc-arc",
                              "-Wall", "-Wextra", "-Werror", "-framework", "Foundation",
                              "-framework", "Metal", "-framework", "CoreGraphics",
                              str(source), "-o", str(dest / "native-metal")])
    native = None
    if built["exit_code"] == 0:
        metadata["probe_binary_sha256"] = hashlib.sha256((dest / "native-metal").read_bytes()).hexdigest()
        run("native-linkage", ["otool", "-L", str(dest / "native-metal")])
        native_record, data = run("native-metal", [str(dest / "native-metal")])
        if native_record["exit_code"] in (0, 2):
            try:
                native = json.loads(data)
            except (ValueError, UnicodeDecodeError):
                pass
    binary = shutil.which(args.zetesis)
    if binary:
        metadata["zetesis_path"] = binary
        metadata["zetesis_sha256"] = hashlib.sha256(Path(binary).read_bytes()).hexdigest()
        run("zetesis-version", [binary, "--version"])
        run("zetesis-linkage", ["otool", "-L", binary])
        run("zetesis-devices", [binary, "devices"])
    else:
        metadata["zetesis_path"] = None
    metadata["native_discovery"] = native
    if native is None:
        status = "INCOMPLETE: native probe failed; inspect retained diagnostics."
    elif native.get("device_count", 0) == 0:
        status = "NO_NATIVE_ADAPTER: this process cannot enumerate a Metal device; no physical execution qualified."
    else:
        status = "NATIVE_ADAPTER_VISIBLE: compare zetesis-devices; physical compute qualification remains separate."
    metadata["status"] = status
    (dest / "summary.json").write_text(json.dumps(metadata, indent=2) + "\n")
    (dest / "status.txt").write_text(status + "\n")
    print(status)
    print(f"Evidence: {dest}")
    return 0 if native and native.get("device_count", 0) > 0 else 2


if __name__ == "__main__":
    raise SystemExit(main())
