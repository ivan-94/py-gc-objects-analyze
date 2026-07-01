from __future__ import annotations

import argparse
import json
import socket
import subprocess
import time
import urllib.request
from pathlib import Path
from typing import Any

TARGETS_MS = {
    "summary": 1000,
    "objects_page": 300,
    "object_detail": 500,
    "graph": 1500,
    "sql_explain": 500,
}


def main() -> None:
    parser = argparse.ArgumentParser(description="Run pygco query and local API benchmarks")
    parser.add_argument("--db", type=Path, required=True)
    parser.add_argument("--pygco", default="target/debug/pygco")
    parser.add_argument("--snapshot", type=int, default=1)
    parser.add_argument("--iterations", type=int, default=10)
    args = parser.parse_args()

    first_object_id = pick_first_object_id(args.pygco, args.db, args.snapshot)
    cli = {
        "summary": measure_cli(args.iterations, args.pygco, ["summary", str(args.db), "--snapshot", str(args.snapshot), "--format", "json"]),
        "objects_page": measure_cli(
            args.iterations,
            args.pygco,
            [
                "objects",
                str(args.db),
                "--snapshot",
                str(args.snapshot),
                "--sort",
                "reachable-size",
                "--order",
                "desc",
                "--limit",
                "100",
                "--offset",
                "0",
                "--format",
                "json",
            ],
        ),
        "object_detail": measure_cli(
            args.iterations,
            args.pygco,
            ["object", str(args.db), "--snapshot", str(args.snapshot), "--id", first_object_id, "--format", "json"],
        ),
        "graph": measure_cli(
            args.iterations,
            args.pygco,
            [
                "export-subgraph",
                str(args.db),
                "--snapshot",
                str(args.snapshot),
                "--id",
                first_object_id,
                "--depth",
                "2",
                "--node-limit",
                "500",
                "--edge-limit",
                "2000",
                "--format",
                "json",
            ],
        ),
        "sql_explain": measure_cli(
            args.iterations,
            args.pygco,
            [
                "sql",
                str(args.db),
                "--query",
                f"select object_id from objects where snapshot_id = {args.snapshot} order by object_id limit 100",
                "--explain",
                "--format",
                "json",
            ],
        ),
    }
    api = measure_api(args.iterations, args.pygco, args.db, args.snapshot, first_object_id)
    print(
        json.dumps(
            {
                "source_manifest": {
                    "benchmark": "benches/query_api_benchmark.py",
                    "pygco": args.pygco,
                    "database_path": str(args.db),
                    "input_requirement": "Run benches/import_benchmark.py first when the database does not exist.",
                },
                "database_path": str(args.db),
                "snapshot_id": args.snapshot,
                "object_id": first_object_id,
                "iterations": args.iterations,
                "targets_ms": TARGETS_MS,
                "cli": {name: summarize(name, samples) for name, samples in cli.items()},
                "api": {name: summarize(name, samples) for name, samples in api.items()},
            },
            indent=2,
        )
    )


def pick_first_object_id(pygco: str, db: Path, snapshot: int) -> str:
    output = subprocess.run(
        [
            pygco,
            "objects",
            str(db),
            "--snapshot",
            str(snapshot),
            "--sort",
            "object-id",
            "--order",
            "asc",
            "--limit",
            "1",
            "--format",
            "json",
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    payload = json.loads(output.stdout)
    return str(payload["rows"][0]["object_id"])


def measure_cli(iterations: int, pygco: str, args: list[str]) -> list[float]:
    samples = []
    for _ in range(iterations):
        started = time.perf_counter()
        result = subprocess.run([pygco, *args], check=False, text=True, capture_output=True)
        samples.append((time.perf_counter() - started) * 1000)
        if result.returncode != 0:
            raise SystemExit(result.stderr)
    return samples


def measure_api(iterations: int, pygco: str, db: Path, snapshot: int, object_id: str) -> dict[str, list[float]]:
    port = free_port()
    process = subprocess.Popen(
        [
            pygco,
            "web",
            str(db),
            "--host",
            "127.0.0.1",
            "--port",
            str(port),
            "--no-browser",
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )
    base = f"http://127.0.0.1:{port}"
    try:
        wait_for_api(base, process)
        endpoints: dict[str, tuple[str, str, dict[str, Any] | None]] = {
            "summary": ("GET", f"/api/summary?snapshot_id={snapshot}", None),
            "objects_page": (
                "GET",
                f"/api/objects?snapshot_id={snapshot}&sort=reachable_size&order=desc&limit=100&offset=0",
                None,
            ),
            "object_detail": ("GET", f"/api/objects/{object_id}?snapshot_id={snapshot}", None),
            "graph": (
                "GET",
                f"/api/graph?snapshot_id={snapshot}&root_object_id={object_id}&direction=both&depth=2&node_limit=500&edge_limit=2000",
                None,
            ),
            "sql_explain": (
                "POST",
                "/api/sql/explain",
                {
                    "query": f"select object_id from objects where snapshot_id = {snapshot} order by object_id limit 100",
                    "limit": 1000,
                },
            ),
        }
        return {
            name: [measure_http(base, method, path, body) for _ in range(iterations)]
            for name, (method, path, body) in endpoints.items()
        }
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def measure_http(base: str, method: str, path: str, body: dict[str, Any] | None) -> float:
    data = None
    headers = {}
    if body is not None:
        data = json.dumps(body).encode("utf-8")
        headers["content-type"] = "application/json"
    request = urllib.request.Request(f"{base}{path}", data=data, headers=headers, method=method)
    started = time.perf_counter()
    with urllib.request.urlopen(request, timeout=10) as response:
        response.read()
    return (time.perf_counter() - started) * 1000


def summarize(name: str, samples: list[float]) -> dict[str, Any]:
    sorted_samples = sorted(samples)
    target = TARGETS_MS[name]
    p95 = percentile(sorted_samples, 0.95)
    return {
        "min_ms": round(sorted_samples[0], 3),
        "p50_ms": round(percentile(sorted_samples, 0.50), 3),
        "p95_ms": round(p95, 3),
        "max_ms": round(sorted_samples[-1], 3),
        "target_ms": target,
        "within_target": p95 <= target,
    }


def percentile(sorted_samples: list[float], percentile_value: float) -> float:
    if not sorted_samples:
        return 0.0
    index = min(len(sorted_samples) - 1, max(0, int(round((len(sorted_samples) - 1) * percentile_value))))
    return sorted_samples[index]


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def wait_for_api(base: str, process: subprocess.Popen[str]) -> None:
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if process.poll() is not None:
            stderr = process.stderr.read() if process.stderr else ""
            raise SystemExit(f"pygco web exited early:\n{stderr}")
        try:
            with urllib.request.urlopen(f"{base}/api/session", timeout=0.5) as response:
                response.read()
                return
        except Exception:
            time.sleep(0.05)
    process.terminate()
    raise SystemExit("timed out waiting for pygco web")


if __name__ == "__main__":
    main()
