from __future__ import annotations

import gzip
import io
import json
import py_compile
import threading
import tracemalloc
from pathlib import Path

import pytest

import pygco_dump.core as core
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


def test_iter_gc_dump_records_freezes_snapshot_when_called() -> None:
    records = iter_gc_dump_records(
        collect=False,
        include_referents=False,
        include_referent_stubs=False,
        include_repr=False,
    )

    created_after_snapshot: list[object] = []
    dumped_ids = {
        record["id"] for record in records if record.get("record_type") == "object"
    }

    assert id(created_after_snapshot) not in dumped_ids


def test_iter_gc_dump_records_excludes_its_returned_iterator() -> None:
    holder: list[object] = []
    records = iter_gc_dump_records(
        collect=False,
        include_referents=True,
        include_referent_stubs=True,
        include_repr=False,
    )
    records_id = id(records)
    holder.append(records)

    object_records = [record for record in records if record.get("record_type") == "object"]
    dumped_ids = {record["id"] for record in object_records}
    holder_record = next(record for record in object_records if record["id"] == id(holder))

    assert records_id not in dumped_ids
    assert records_id not in holder_record["referents"]


def test_iter_gc_dump_records_avoids_id_index_when_stubs_are_disabled() -> None:
    objects = [object() for _ in range(50_000)]

    def capture_peak(*, include_referents: bool, include_referent_stubs: bool) -> int:
        tracemalloc.start()
        records = iter_gc_dump_records(
            collect=False,
            include_referents=include_referents,
            include_referent_stubs=include_referent_stubs,
            include_repr=False,
            objects=objects,
        )
        _, peak = tracemalloc.get_traced_memory()
        records.close()  # type: ignore[attr-defined]
        tracemalloc.stop()
        return peak

    without_stubs_peak = capture_peak(
        include_referents=False,
        include_referent_stubs=False,
    )
    with_stubs_peak = capture_peak(
        include_referents=True,
        include_referent_stubs=True,
    )

    assert without_stubs_peak * 3 < with_stubs_peak


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


def test_write_gc_dump_excludes_its_gzip_writer(monkeypatch: pytest.MonkeyPatch) -> None:
    created_writers: list[gzip.GzipFile] = []
    real_gzip_file = gzip.GzipFile

    class TrackingGzipFile(real_gzip_file):
        def __init__(self, *args: object, **kwargs: object) -> None:
            super().__init__(*args, **kwargs)
            created_writers.append(self)

    monkeypatch.setattr(core.gzip, "GzipFile", TrackingGzipFile)
    buffer = io.BytesIO()

    write_gc_dump(
        buffer,
        collect=False,
        include_referents=False,
        include_referent_stubs=False,
        include_repr=False,
    )
    writer_ids = {id(writer) for writer in created_writers}

    payload = gzip.decompress(buffer.getvalue()).decode("utf-8")
    dumped_ids = {
        record["id"]
        for line in payload.splitlines()
        if (record := json.loads(line)).get("record_type") == "object"
    }
    assert writer_ids.isdisjoint(dumped_ids)


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
