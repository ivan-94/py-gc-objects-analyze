from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any


def main() -> None:
    parser = argparse.ArgumentParser(description="Run pygco import benchmark and emit JSON")
    parser.add_argument("--dump", action="append", required=True, help="Input dump path; may repeat")
    parser.add_argument("--pygco", default="target/debug/pygco")
    parser.add_argument("--keep-db", type=Path)
    args = parser.parse_args()

    output = args.keep_db
    with tempfile.TemporaryDirectory() as tmp:
        if output is None:
            output = Path(tmp) / "analysis.sqlite"
        started = time.perf_counter()
        process = subprocess.Popen(
            [
                args.pygco,
                "import",
                *args.dump,
                "-o",
                str(output),
                "--rebuild",
                "--profile",
                "--format",
                "json",
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        peak_rss_kib = 0
        while process.poll() is None:
            peak_rss_kib = max(peak_rss_kib, sample_rss_kib(process.pid))
            time.sleep(0.05)
        stdout, stderr = process.communicate()
        peak_rss_kib = max(peak_rss_kib, sample_rss_kib(process.pid))
        elapsed_ms = int((time.perf_counter() - started) * 1000)
        if process.returncode != 0:
            raise SystemExit(stderr)
        payload: dict[str, Any] = json.loads(stdout)
        total_objects = sum(int(snapshot.get("object_count", 0)) for snapshot in payload.get("snapshots", []))
        print(
            json.dumps(
                {
                    "source_manifest": {
                        "benchmark": "benches/import_benchmark.py",
                        "pygco": args.pygco,
                        "dumps": args.dump,
                    },
                    "dumps": args.dump,
                    "database_path": str(output),
                    "elapsed_ms": elapsed_ms,
                    "peak_rss_kib": peak_rss_kib,
                    "rss_bytes_per_object": round((peak_rss_kib * 1024) / max(total_objects, 1), 3),
                    "snapshots": payload.get("snapshots", []),
                    "profile": payload.get("profile", []),
                },
                indent=2,
            )
        )


def sample_rss_kib(pid: int) -> int:
    try:
        result = subprocess.run(
            ["ps", "-o", "rss=", "-p", str(pid)],
            check=False,
            text=True,
            capture_output=True,
        )
    except OSError:
        return 0
    if result.returncode != 0:
        return 0
    value = result.stdout.strip()
    return int(value) if value.isdigit() else 0


if __name__ == "__main__":
    main()
