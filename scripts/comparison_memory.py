"""A fresh per-solve parent reports only its solver child's peak RSS."""
import json
from pathlib import Path
import platform
import resource
import subprocess
import sys


def main():
    system = platform.system()
    if system not in ("Darwin", "Linux"):
        raise ValueError("peak RSS units are defined here only for macOS/Linux")
    result = subprocess.run(sys.argv[2:], check=False)
    raw = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    Path(sys.argv[1]).write_text(json.dumps({
        "peak_rss_bytes": int(raw) * (1024 if system == "Linux" else 1),
        "raw_ru_maxrss": raw, "raw_unit": "KiB" if system == "Linux" else "bytes",
        "scope": "fresh parent RUSAGE_CHILDREN; excludes Python wrapper RSS",
    }))
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
