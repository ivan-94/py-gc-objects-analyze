from __future__ import annotations

import io
from typing import Any

from pygco_dump import write_gc_dump


async def app(scope: dict[str, Any], receive: Any, send: Any) -> None:
    _ = receive
    if scope["type"] != "http" or scope.get("path") != "/debug/gc-heap-dump":
        await send({"type": "http.response.start", "status": 404, "headers": []})
        await send({"type": "http.response.body", "body": b"not found"})
        return

    buffer = io.BytesIO()
    write_gc_dump(buffer, collect=False, include_repr=False)
    await send(
        {
            "type": "http.response.start",
            "status": 200,
            "headers": [
                (b"content-type", b"application/gzip"),
                (b"content-disposition", b'attachment; filename="gc-heap-dump.jsonl.gz"'),
                (b"x-content-type-options", b"nosniff"),
            ],
        }
    )
    await send({"type": "http.response.body", "body": buffer.getvalue()})
