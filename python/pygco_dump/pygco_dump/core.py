from __future__ import annotations

import gc
import gzip
import json
import os
import platform
import socket
import sys
import threading
import time
import uuid
from collections.abc import Iterable, Iterator
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Any, BinaryIO

FORMAT_NAME = "pygco-dump-jsonl"
FORMAT_VERSION = 1
PRODUCER = "pygco_dump"
PRODUCER_VERSION = "0.1.1"
_PRODUCER_RUN_ID = uuid.uuid4().hex
_PROCESS_STARTED_AT = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
_DUMP_LOCK = threading.Lock()
_SEQUENCE_LOCK = threading.Lock()
_DUMP_SEQUENCE = 0


class DumpInProgressError(RuntimeError):
    pass


class _LockOwningIterator(Iterator[dict[str, Any]]):
    def __init__(self, records: Iterator[dict[str, Any]]) -> None:
        self._records = records
        self._closed = False

    def __iter__(self) -> _LockOwningIterator:
        return self

    def __next__(self) -> dict[str, Any]:
        if self._closed:
            raise StopIteration
        try:
            return next(self._records)
        except StopIteration:
            self.close()
            raise
        except BaseException:
            try:
                self.close()
            except BaseException:
                pass
            raise

    def close(self) -> None:
        if self._closed:
            return
        self._closed = True
        try:
            close = getattr(self._records, "close", None)
            if close is not None:
                close()
        finally:
            _DUMP_LOCK.release()

    def __del__(self) -> None:
        try:
            self.close()
        except BaseException:
            pass


@dataclass(frozen=True)
class DumpSummary:
    dumped_count: int
    stub_count: int
    total_object_records: int
    elapsed_ms: int


def iter_gc_dump_records(
    *,
    collect: bool = False,
    include_referents: bool = True,
    include_referent_stubs: bool = True,
    include_repr: bool = False,
    repr_limit: int = 0,
    objects: Iterable[Any] | None = None,
) -> Iterator[dict[str, Any]]:
    if not _DUMP_LOCK.acquire(blocking=False):
        raise DumpInProgressError("GC object dump is already running in this process")

    try:
        started = time.perf_counter()
        if collect:
            gc.collect()
        with _SEQUENCE_LOCK:
            global _DUMP_SEQUENCE
            _DUMP_SEQUENCE += 1
            dump_sequence = _DUMP_SEQUENCE

        snapshot_objects = list(objects) if objects is not None else list(gc.get_objects())
        snapshot_ids = (
            {id(obj) for obj in snapshot_objects}
            if include_referents and include_referent_stubs
            else set()
        )
        excluded_ids = {id(snapshot_objects), id(snapshot_ids)}
        excluded_ids.add(id(excluded_ids))
        created_at = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
        records = _iter_gc_dump_records_unlocked(
            started=started,
            dump_sequence=dump_sequence,
            snapshot_objects=snapshot_objects,
            snapshot_ids=snapshot_ids,
            excluded_ids=excluded_ids,
            created_at=created_at,
            collect=collect,
            include_referents=include_referents,
            include_referent_stubs=include_referent_stubs,
            include_repr=include_repr,
            repr_limit=repr_limit,
        )
        excluded_ids.add(id(records))
        iterator = _LockOwningIterator(records)
        excluded_ids.add(id(iterator))
        return iterator
    except BaseException:
        _DUMP_LOCK.release()
        raise


def write_gc_dump(
    file: BinaryIO,
    *,
    collect: bool = False,
    include_referents: bool = True,
    include_referent_stubs: bool = True,
    include_repr: bool = False,
    repr_limit: int = 0,
    objects: Iterable[Any] | None = None,
) -> DumpSummary:
    last_record: dict[str, Any] | None = None
    records = iter_gc_dump_records(
        collect=collect,
        include_referents=include_referents,
        include_referent_stubs=include_referent_stubs,
        include_repr=include_repr,
        repr_limit=repr_limit,
        objects=objects,
    )
    try:
        with gzip.GzipFile(fileobj=file, mode="wb", compresslevel=1) as gzip_file:
            for record in records:
                last_record = record
                line = json.dumps(record, ensure_ascii=False, separators=(",", ":")).encode(
                    "utf-8"
                )
                gzip_file.write(line + b"\n")
    finally:
        close = getattr(records, "close", None)
        if close is not None:
            close()
    if not last_record or last_record.get("phase") != "end":
        raise RuntimeError("dump did not produce end metadata")
    return DumpSummary(
        dumped_count=int(last_record["dumped_count"]),
        stub_count=int(last_record["stub_count"]),
        total_object_records=int(last_record["total_object_records"]),
        elapsed_ms=int(last_record["elapsed_ms"]),
    )


def _iter_gc_dump_records_unlocked(
    *,
    started: float,
    dump_sequence: int,
    snapshot_objects: list[Any],
    snapshot_ids: set[int],
    excluded_ids: set[int],
    created_at: str,
    collect: bool,
    include_referents: bool,
    include_referent_stubs: bool,
    include_repr: bool,
    repr_limit: int,
) -> Iterator[dict[str, Any]]:
    stub_seen: set[int] = set()

    yield {
        "record_type": "metadata",
        "phase": "start",
        "format": FORMAT_NAME,
        "format_version": FORMAT_VERSION,
        "producer": PRODUCER,
        "producer_version": PRODUCER_VERSION,
        "producer_run_id": _PRODUCER_RUN_ID,
        "dump_sequence": dump_sequence,
        "created_at": created_at,
        "process_started_at": _PROCESS_STARTED_AT,
        "host_id": _host_id(),
        "container_id": _container_id(),
        "pid": os.getpid(),
        "python_version": sys.version,
        "platform": platform.platform(),
        "collect_before_dump": collect,
        "include_referents": include_referents,
        "include_referent_stubs": include_referents and include_referent_stubs,
        "include_repr": include_repr,
        "repr_limit": repr_limit if include_repr else 0,
        "gc_count": list(gc.get_count()),
        "gc_stats": _safe_gc_stats(),
        "object_count": len(snapshot_objects),
    }

    dumped_count = 0
    stub_count = 0
    for obj in snapshot_objects:
        referents = _safe_referents(obj) if include_referents else []
        referent_ids: list[int] | None = None
        new_stubs: list[Any] | None = None
        if include_referents:
            referent_ids = []
            for referent in referents:
                referent_id = id(referent)
                if include_referent_stubs and referent_id in snapshot_ids:
                    referent_ids.append(referent_id)
                    continue
                if referent_id in excluded_ids:
                    continue
                referent_ids.append(referent_id)
                if not include_referent_stubs or referent_id in stub_seen:
                    continue
                stub_seen.add(referent_id)
                if new_stubs is None:
                    new_stubs = []
                new_stubs.append(referent)
        yield _object_record(
            obj,
            referent_ids=referent_ids,
            include_repr=include_repr,
            repr_limit=repr_limit,
            stub=False,
        )
        dumped_count += 1
        for referent in new_stubs or ():
            yield _object_record(
                referent,
                referent_ids=[],
                include_repr=False,
                repr_limit=0,
                stub=True,
            )
            stub_count += 1

    yield {
        "record_type": "metadata",
        "phase": "end",
        "dumped_count": dumped_count,
        "stub_count": stub_count,
        "total_object_records": dumped_count + stub_count,
        "elapsed_ms": int((time.perf_counter() - started) * 1000),
    }


def _object_record(
    obj: Any,
    *,
    referent_ids: list[int] | None,
    include_repr: bool,
    repr_limit: int,
    stub: bool,
) -> dict[str, Any]:
    obj_type = type(obj)
    module = str(getattr(obj_type, "__module__", "builtins") or "builtins")
    qualname = str(getattr(obj_type, "__qualname__", getattr(obj_type, "__name__", "<unknown>")))
    type_name = qualname if module == "builtins" else f"{module}.{qualname}"
    record: dict[str, Any] = {
        "record_type": "object",
        "id": id(obj),
        "type": type_name,
        "module": module,
        "qualname": qualname,
        "size": _safe_sizeof(obj),
        "gc_tracked": _safe_is_tracked(obj),
        "stub": stub,
        "referents": referent_ids if referent_ids is not None else [],
    }
    if include_repr:
        record["repr"] = _safe_repr(obj, repr_limit)
    return record


def _safe_sizeof(obj: Any) -> int | None:
    try:
        return sys.getsizeof(obj)
    except Exception:
        return None


def _safe_referents(obj: Any) -> list[Any]:
    try:
        return list(gc.get_referents(obj))
    except Exception:
        return []


def _safe_is_tracked(obj: Any) -> bool | None:
    try:
        return gc.is_tracked(obj)
    except Exception:
        return None


def _safe_repr(obj: Any, limit: int) -> str | None:
    if limit <= 0:
        return None
    try:
        value = repr(obj)
    except Exception as exc:
        return f"<repr failed: {type(exc).__name__}>"
    if len(value) <= limit:
        return value
    return value[:limit] + "...<truncated>"


def _safe_gc_stats() -> list[dict[str, int]] | None:
    try:
        return list(gc.get_stats())
    except Exception:
        return None


def _host_id() -> str | None:
    try:
        return socket.gethostname()
    except Exception:
        return None


def _container_id() -> str | None:
    cgroup = "/proc/self/cgroup"
    if not os.path.exists(cgroup):
        return None
    try:
        with open(cgroup, encoding="utf-8") as file:
            for line in file:
                value = line.strip().rsplit("/", maxsplit=1)[-1]
                if len(value) >= 12:
                    return value[:64]
    except Exception:
        return None
    return None
