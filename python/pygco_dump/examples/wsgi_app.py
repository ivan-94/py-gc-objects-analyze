from __future__ import annotations

import io
from collections.abc import Callable, Iterable
from http import HTTPStatus
from typing import Any

from pygco_dump import write_gc_dump


def application(
    environ: dict[str, Any],
    start_response: Callable[[str, list[tuple[str, str]]], None],
) -> Iterable[bytes]:
    if environ.get("PATH_INFO") != "/debug/gc-heap-dump":
        start_response(f"{HTTPStatus.NOT_FOUND.value} Not Found", [])
        return [b"not found"]

    buffer = io.BytesIO()
    write_gc_dump(buffer, collect=False, include_repr=False)
    start_response(
        f"{HTTPStatus.OK.value} OK",
        [
            ("Content-Type", "application/gzip"),
            ("Content-Disposition", 'attachment; filename="gc-heap-dump.jsonl.gz"'),
            ("X-Content-Type-Options", "nosniff"),
        ],
    )
    return [buffer.getvalue()]
