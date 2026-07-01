from __future__ import annotations

import gzip
import json
from collections.abc import Iterable, Iterator
from collections.abc import Callable
from itertools import chain
from typing import Any

from .core import DumpInProgressError, iter_gc_dump_records


def gc_heap_dump_route() -> Callable[..., object]:
    try:
        from fastapi import HTTPException, Query
        from fastapi.responses import StreamingResponse
    except Exception as exc:  # pragma: no cover
        raise RuntimeError("Install pygco-dump[fastapi] to use FastAPI helpers") from exc

    def route(
        collect: bool = Query(default=False),
        include_referents: bool = Query(default=True),
        include_referent_stubs: bool = Query(default=True),
        include_repr: bool = Query(default=False),
        repr_limit: int = Query(default=0, ge=0, le=500),
    ) -> StreamingResponse:
        records = iter_gc_dump_records(
            collect=collect,
            include_referents=include_referents,
            include_referent_stubs=include_referent_stubs,
            include_repr=include_repr,
            repr_limit=repr_limit,
        )
        try:
            first = next(records)
        except DumpInProgressError as exc:
            raise HTTPException(status_code=409, detail=str(exc)) from exc
        except StopIteration as exc:  # pragma: no cover
            raise HTTPException(status_code=500, detail="dump produced no records") from exc

        return StreamingResponse(
            _gzip_jsonl_stream(chain([first], records)),
            media_type="application/gzip",
            headers={
                "Content-Disposition": 'attachment; filename="gc-heap-dump.jsonl.gz"',
                "X-Content-Type-Options": "nosniff",
            },
        )

    return route


class _ChunkBuffer:
    def __init__(self) -> None:
        self._chunks: list[bytes] = []

    def write(self, data: bytes) -> int:
        self._chunks.append(bytes(data))
        return len(data)

    def flush(self) -> None:
        return None

    def drain(self) -> list[bytes]:
        chunks = self._chunks
        self._chunks = []
        return chunks


def _gzip_jsonl_stream(records: Iterable[dict[str, Any]]) -> Iterator[bytes]:
    buffer = _ChunkBuffer()
    with gzip.GzipFile(fileobj=buffer, mode="wb", compresslevel=1) as gzip_file:
        for record in records:
            line = json.dumps(record, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
            gzip_file.write(line + b"\n")
            yield from buffer.drain()
    yield from buffer.drain()
