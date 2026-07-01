from __future__ import annotations

import argparse
import json
import platform
import re
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any


def main() -> None:
    parser = argparse.ArgumentParser(description="Measure pygco import peak RSS across dump sizes")
    parser.add_argument("--pygco", default="target/debug/pygco")
    parser.add_argument("--dump", action="append", required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    results = [measure_import(args.pygco, Path(dump)) for dump in args.dump]
    payload = {
        "source_manifest": {
            "benchmark": "benches/memory_benchmark.py",
            "pygco": args.pygco,
            "dumps": args.dump,
            "mode": "import --no-reachability",
        },
        "results": results,
        "assessment": assess(results),
    }
    text = json.dumps(payload, indent=2)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text + "\n", encoding="utf-8")
    print(text)


def measure_import(pygco: str, dump: Path) -> dict[str, Any]:
    with tempfile.TemporaryDirectory() as tmp:
        db = Path(tmp) / "analysis.sqlite"
        command = [
            pygco,
            "import",
            str(dump),
            "-o",
            str(db),
            "--rebuild",
            "--no-reachability",
            "--format",
            "json",
        ]
        timed_command = time_command(command)
        started = time.perf_counter()
        result = subprocess.run(timed_command, check=False, text=True, capture_output=True)
        elapsed_ms = int((time.perf_counter() - started) * 1000)
        if result.returncode != 0:
            raise SystemExit(result.stderr)
        stdout = result.stdout
        stderr = result.stderr
        if timed_command[0].endswith("time"):
            stdout = strip_time_stdout(result.stdout)
        payload = json.loads(stdout)
        snapshot = payload["snapshots"][0]
        peak_rss_bytes = parse_peak_rss_bytes(stderr)
        object_count = int(snapshot["object_count"])
        return {
            "dump": str(dump),
            "object_count": object_count,
            "edge_count": int(snapshot["edge_count"]),
            "elapsed_ms": elapsed_ms,
            "peak_rss_bytes": peak_rss_bytes,
            "peak_rss_mib": round(peak_rss_bytes / 1024 / 1024, 2) if peak_rss_bytes else None,
            "peak_rss_bytes_per_object": round(peak_rss_bytes / object_count, 2) if peak_rss_bytes and object_count else None,
        }


def time_command(command: list[str]) -> list[str]:
    if Path("/usr/bin/time").is_file():
        if platform.system() == "Darwin":
            return ["/usr/bin/time", "-l", *command]
        return ["/usr/bin/time", "-v", *command]
    return command


def strip_time_stdout(stdout: str) -> str:
    start = stdout.find("{")
    end = stdout.rfind("}")
    if start == -1 or end == -1:
        return stdout
    return stdout[start : end + 1]


def parse_peak_rss_bytes(stderr: str) -> int | None:
    mac = re.search(r"([0-9]+)\s+maximum resident set size", stderr)
    if mac:
        return int(mac.group(1))
    linux = re.search(r"Maximum resident set size \(kbytes\):\s*([0-9]+)", stderr)
    if linux:
        return int(linux.group(1)) * 1024
    return None


def assess(results: list[dict[str, Any]]) -> dict[str, Any]:
    measured = [result for result in results if result.get("peak_rss_bytes") and result.get("object_count")]
    if len(measured) < 2:
        return {"status": "insufficient_data"}
    first = measured[0]
    last = measured[-1]
    object_scale = last["object_count"] / first["object_count"]
    rss_scale = last["peak_rss_bytes"] / first["peak_rss_bytes"]
    return {
        "status": "pass" if rss_scale < object_scale * 0.5 else "review",
        "object_scale": round(object_scale, 3),
        "rss_scale": round(rss_scale, 3),
        "criterion": "peak RSS scale should be materially below object-count scale for streaming import",
    }


if __name__ == "__main__":
    main()
