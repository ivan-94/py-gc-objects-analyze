from __future__ import annotations

import gzip
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1] / "golden"
EXPECTED = ROOT / "expected"


def main() -> None:
    ROOT.mkdir(parents=True, exist_ok=True)
    EXPECTED.mkdir(parents=True, exist_ok=True)
    fixtures = {
        "tiny-v1.jsonl.gz": tiny_records("tiny", 1),
        "stubs-v1.jsonl.gz": stubs_records("stubs", 1),
        "missing-referents-v1.jsonl.gz": missing_records("missing", 1),
        "cycles-v1.jsonl.gz": cycles_records("cycles", 1),
        "diff-before-v1.jsonl.gz": diff_before_records(),
        "diff-after-v1.jsonl.gz": diff_after_records(),
    }
    for name, records in fixtures.items():
        write_dump(ROOT / name, records)
    write_json(EXPECTED / "summary.json", expected_summary())
    write_json(EXPECTED / "objects.json", expected_objects())
    write_json(EXPECTED / "diff.json", expected_diff())
    write_json(EXPECTED / "reachability.json", expected_reachability())


def metadata_start(run: str, sequence: int, object_count: int) -> dict[str, Any]:
    return {
        "record_type": "metadata",
        "phase": "start",
        "format": "pygco-dump-jsonl",
        "format_version": 1,
        "producer": "pygco_dump",
        "producer_version": "0.1.0",
        "producer_run_id": f"fixture-{run}",
        "dump_sequence": sequence,
        "created_at": f"2026-07-01T00:00:0{sequence}Z",
        "process_started_at": "2026-07-01T00:00:00Z",
        "host_id": "fixture-host",
        "container_id": None,
        "pid": 4242,
        "python_version": "3.12.0",
        "platform": "fixture",
        "collect_before_dump": False,
        "include_referents": True,
        "include_referent_stubs": True,
        "include_repr": False,
        "repr_limit": 0,
        "gc_count": [0, 0, 0],
        "gc_stats": None,
        "object_count": object_count,
    }


def metadata_end(dumped: int, stubs: int) -> dict[str, Any]:
    return {
        "record_type": "metadata",
        "phase": "end",
        "dumped_count": dumped,
        "stub_count": stubs,
        "total_object_records": dumped + stubs,
        "elapsed_ms": 1,
    }


def obj(
    object_id: int,
    type_name: str,
    module: str,
    qualname: str,
    size: int,
    referents: list[int],
    *,
    stub: bool = False,
    gc_tracked: bool = True,
) -> dict[str, Any]:
    return {
        "record_type": "object",
        "id": object_id,
        "type": type_name,
        "module": module,
        "qualname": qualname,
        "size": size,
        "gc_tracked": gc_tracked,
        "stub": stub,
        "referents": referents,
    }


def tiny_records(run: str, sequence: int) -> list[dict[str, Any]]:
    return [
        metadata_start(run, sequence, 4),
        obj(1, "dict", "builtins", "dict", 280, [2, 3, 4]),
        obj(2, "list", "builtins", "list", 120, [3]),
        obj(3, "app.Widget", "app", "Widget", 40, []),
        obj(4, "cachetools.LRUCache", "cachetools", "LRUCache", 160, [3]),
        metadata_end(4, 0),
    ]


def stubs_records(run: str, sequence: int) -> list[dict[str, Any]]:
    return [
        metadata_start(run, sequence, 1),
        obj(10, "app.Container", "app", "Container", 88, [11]),
        obj(11, "int", "builtins", "int", 28, [], stub=True, gc_tracked=False),
        metadata_end(1, 1),
    ]


def missing_records(run: str, sequence: int) -> list[dict[str, Any]]:
    return [
        metadata_start(run, sequence, 1),
        obj(20, "app.Container", "app", "Container", 88, [999]),
        metadata_end(1, 0),
    ]


def cycles_records(run: str, sequence: int) -> list[dict[str, Any]]:
    return [
        metadata_start(run, sequence, 2),
        obj(30, "app.Node", "app", "Node", 50, [31]),
        obj(31, "app.Node", "app", "Node", 50, [30]),
        metadata_end(2, 0),
    ]


def diff_before_records() -> list[dict[str, Any]]:
    return [
        metadata_start("diff", 1, 2),
        obj(100, "dict", "builtins", "dict", 280, [101]),
        obj(101, "app.Widget", "app", "Widget", 40, []),
        metadata_end(2, 0),
    ]


def diff_after_records() -> list[dict[str, Any]]:
    return [
        metadata_start("diff", 2, 3),
        obj(100, "dict", "builtins", "dict", 280, [101, 102]),
        obj(101, "app.Widget", "app", "Widget", 80, []),
        obj(102, "redis.ConnectionPool", "redis", "ConnectionPool", 72, []),
        metadata_end(3, 0),
    ]


def expected_summary() -> dict[str, Any]:
    return {
        "tiny": {
            "object_count": 4,
            "edge_count": 5,
            "stub_count": 0,
            "missing_referent_count": 0,
            "shallow_size_sum": 600,
            "top_type_by_count": "app.Widget",
            "top_type_by_shallow_size": "dict",
        }
    }


def expected_objects() -> dict[str, Any]:
    return {
        "tiny_object_ids": ["1", "2", "3", "4"],
        "stubs": [{"object_id": "11", "type": "int", "stub": 1}],
        "missing_edges": [{"from_id": "20", "to_id": "999"}],
    }


def expected_diff() -> dict[str, Any]:
    return {
        "summary_delta": {
            "object_count": 1,
            "edge_count": 1,
            "shallow_size_sum": 112,
        },
        "confidence": "high",
        "new_object_ids": ["102"],
        "changed_object_ids": ["101"],
    }


def expected_reachability() -> dict[str, Any]:
    return {
        "tiny": {
            "1": {"reachable_count": 4, "reachable_size": 600, "truncated": 0},
            "2": {"reachable_count": 2, "reachable_size": 160, "truncated": 0},
            "3": {"reachable_count": 1, "reachable_size": 40, "truncated": 0},
            "4": {"reachable_count": 2, "reachable_size": 200, "truncated": 0},
        },
        "cycles": {
            "30": {"reachable_count": 2, "reachable_size": 100, "truncated": 0},
            "31": {"reachable_count": 2, "reachable_size": 100, "truncated": 0},
        },
    }


def write_dump(path: Path, records: list[dict[str, Any]]) -> None:
    with path.open("wb") as raw:
        with gzip.GzipFile(fileobj=raw, mode="wb", compresslevel=1, mtime=0) as file:
            for record in records:
                file.write(json.dumps(record, ensure_ascii=False, separators=(",", ":")).encode())
                file.write(b"\n")


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
