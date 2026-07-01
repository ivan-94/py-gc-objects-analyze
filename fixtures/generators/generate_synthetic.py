from __future__ import annotations

import argparse
import gzip
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1] / "synthetic"

PROFILES: dict[str, dict[str, Any]] = {
    "medium": {"objects": 10_000, "fanout": 3, "pathological": False},
    "large": {"objects": 50_000, "fanout": 4, "pathological": False},
    "pathological": {"objects": 20_000, "fanout": 8, "pathological": True},
}


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate deterministic pygco synthetic dumps")
    parser.add_argument("--objects", type=int)
    parser.add_argument("--fanout", type=int)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--output-dir", type=Path, default=ROOT)
    parser.add_argument("--profile", choices=sorted(PROFILES))
    parser.add_argument("--all", action="store_true", help="Generate medium, large, and pathological fixtures")
    parser.add_argument("--pathological", action="store_true", help="Create high fanout and cycle-heavy edges")
    args = parser.parse_args()

    if args.all:
        for profile in PROFILES:
            generate_profile(profile, args.output_dir)
        return

    if args.profile:
        generate_profile(args.profile, args.output_dir if args.output is None else None, args.output)
        return

    objects = args.objects or 10_000
    fanout = args.fanout or 3
    output = args.output or args.output_dir / f"synthetic-{objects}-f{fanout}.jsonl.gz"
    write_dump(output, objects, fanout, args.pathological)


def generate_profile(profile: str, output_dir: Path | None, output: Path | None = None) -> None:
    config = PROFILES[profile]
    path = output or (output_dir or ROOT) / f"{profile}.jsonl.gz"
    write_dump(
        path,
        int(config["objects"]),
        int(config["fanout"]),
        bool(config["pathological"]),
    )


def metadata_start(objects: int) -> dict[str, Any]:
    return {
        "record_type": "metadata",
        "phase": "start",
        "format": "pygco-dump-jsonl",
        "format_version": 1,
        "producer": "pygco_dump_synthetic",
        "producer_version": "0.1.0",
        "producer_run_id": "synthetic-run",
        "dump_sequence": 1,
        "created_at": "2026-07-01T00:00:00Z",
        "process_started_at": "2026-07-01T00:00:00Z",
        "host_id": "synthetic",
        "container_id": None,
        "pid": 1,
        "python_version": "3.12.0",
        "platform": "synthetic",
        "collect_before_dump": False,
        "include_referents": True,
        "include_referent_stubs": False,
        "include_repr": False,
        "repr_limit": 0,
        "gc_count": [0, 0, 0],
        "gc_stats": None,
        "object_count": objects,
    }


def metadata_end(objects: int) -> dict[str, Any]:
    return {
        "record_type": "metadata",
        "phase": "end",
        "dumped_count": objects,
        "stub_count": 0,
        "total_object_records": objects,
        "elapsed_ms": 1,
    }


def synthetic_object(index: int, objects: int, fanout: int, pathological: bool) -> dict[str, Any]:
    object_id = index + 1
    module = "builtins" if index % 7 == 0 else f"app.module_{index % 17}"
    qualname = "dict" if module == "builtins" else f"Node{index % 31}"
    type_name = qualname if module == "builtins" else f"{module}.{qualname}"
    return {
        "record_type": "object",
        "id": object_id,
        "type": type_name,
        "module": module,
        "qualname": qualname,
        "size": 48 + (index % 13) * 8,
        "gc_tracked": True,
        "stub": False,
        "referents": synthetic_referents(object_id, objects, fanout, pathological),
    }


def synthetic_records(objects: int, fanout: int, pathological: bool) -> list[dict[str, Any]]:
    records = [metadata_start(objects)]
    for index in range(objects):
        records.append(synthetic_object(index, objects, fanout, pathological))
    records.append(metadata_end(objects))
    return records


def synthetic_referents(object_id: int, objects: int, fanout: int, pathological: bool) -> list[int]:
    if objects <= 1:
        return []
    if pathological and object_id == 1:
        return list(range(2, min(objects, fanout * 100) + 1))
    referents: list[int] = []
    for offset in range(1, fanout + 1):
        target = ((object_id + offset - 1) % objects) + 1
        if target != object_id:
            referents.append(target)
    if pathological and object_id % 10 == 0:
        referents.append(max(1, object_id - 9))
    return referents


def write_record(file: gzip.GzipFile, record: dict[str, Any]) -> None:
    file.write(json.dumps(record, separators=(",", ":")).encode("utf-8"))
    file.write(b"\n")


def write_dump(path: Path, objects: int, fanout: int, pathological: bool) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("wb") as raw:
        with gzip.GzipFile(fileobj=raw, mode="wb", compresslevel=1, mtime=0) as file:
            write_record(file, metadata_start(objects))
            for index in range(objects):
                write_record(file, synthetic_object(index, objects, fanout, pathological))
            write_record(file, metadata_end(objects))


if __name__ == "__main__":
    main()
