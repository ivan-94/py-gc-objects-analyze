from __future__ import annotations

import argparse
import json
import socket
import subprocess
import time
import urllib.request
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description="Export pygco OpenAPI JSON from the local API server")
    parser.add_argument("--pygco", default="target/debug/pygco")
    parser.add_argument("--dump", default="fixtures/golden/tiny-v1.jsonl.gz")
    parser.add_argument("--work-dir", type=Path, default=Path(".scratch/openapi-export"))
    parser.add_argument("--output", type=Path, default=Path("docs/generated/openapi.json"))
    args = parser.parse_args()

    args.work_dir.mkdir(parents=True, exist_ok=True)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    db = args.work_dir / "analysis.sqlite"
    subprocess.run(
        [
            args.pygco,
            "import",
            args.dump,
            "-o",
            str(db),
            "--rebuild",
            "--format",
            "json",
        ],
        check=True,
        text=True,
        capture_output=True,
    )

    port = free_port()
    process = subprocess.Popen(
        [
            args.pygco,
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
    try:
        base = f"http://127.0.0.1:{port}"
        wait_for_api(base, process)
        with urllib.request.urlopen(f"{base}/api/openapi.json", timeout=5) as response:
            payload = json.loads(response.read())
        openapi = payload["data"]
        openapi["x-source-manifest"] = {
            "generator": "scripts/export_openapi.py",
            "pygco": args.pygco,
            "dump": args.dump,
            "endpoint": "/api/openapi.json",
            "api_source": "crates/pygco-api/src/lib.rs",
            "contract": "docs/api.md",
        }
        args.output.write_text(json.dumps(openapi, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


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
