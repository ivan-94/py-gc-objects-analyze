from __future__ import annotations

import gzip
import io
import json
import py_compile
import threading
from pathlib import Path

import pytest

from pygco_dump import DumpInProgressError, iter_gc_dump_records, write_gc_dump


def test_iter_gc_dump_records_outputs_contract_records() -> None:
    first = {"name": "first"}
    second = [first]

    records = list(
        iter_gc_dump_records(
            collect=False,
            include_referents=True,
            include_referent_stubs=True,
            include_repr=False,
            objects=[first, second],
        )
    )

    assert records[0]["record_type"] == "metadata"
    assert records[0]["phase"] == "start"
    assert records[0]["format"] == "pygco-dump-jsonl"
    assert records[0]["format_version"] == 1
    assert records[0]["producer_run_id"]
    assert records[0]["dump_sequence"] >= 1
    assert records[0]["collect_before_dump"] is False
    assert records[0]["include_repr"] is False

    objects = [record for record in records if record["record_type"] == "object"]
    assert any(record["id"] == id(first) and record["type"] == "dict" for record in objects)
    assert all("repr" not in record for record in objects)
    assert any(record["stub"] for record in objects)

    assert records[-1]["phase"] == "end"
    assert records[-1]["dumped_count"] == 2
    assert records[-1]["total_object_records"] == len(objects)


def test_write_gc_dump_outputs_gzip_jsonl_and_repr_only_when_enabled() -> None:
    buffer = io.BytesIO()
    summary = write_gc_dump(
        buffer,
        include_referents=False,
        include_repr=True,
        repr_limit=20,
        objects=["hello"],
    )

    assert summary.total_object_records == 1
    payload = gzip.decompress(buffer.getvalue()).decode("utf-8")
    lines = [json.loads(line) for line in payload.splitlines()]
    assert lines[1]["type"] == "str"
    assert lines[1]["repr"] == "'hello'"
    assert lines[1]["referents"] == []


def test_single_flight_lock_rejects_concurrent_dump() -> None:
    gate = threading.Event()
    release = threading.Event()

    class BlockingObjects:
        def __iter__(self):
            gate.set()
            release.wait(timeout=2)
            return iter([])

    errors: list[BaseException] = []

    def run_dump() -> None:
        try:
            list(iter_gc_dump_records(objects=BlockingObjects()))
        except BaseException as exc:  # pragma: no cover - failure path captured for assertion
            errors.append(exc)

    thread = threading.Thread(target=run_dump)
    thread.start()
    gate.wait(timeout=2)
    try:
        with pytest.raises(DumpInProgressError):
            list(iter_gc_dump_records(objects=[]))
    finally:
        release.set()
        thread.join(timeout=2)
    assert not errors


def test_examples_compile() -> None:
    examples = Path(__file__).resolve().parents[1] / "examples"
    for path in examples.glob("*.py"):
        py_compile.compile(str(path), doraise=True)
