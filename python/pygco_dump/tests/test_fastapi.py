from __future__ import annotations

import gzip
import json

from fastapi import FastAPI
from fastapi.testclient import TestClient

from pygco_dump.fastapi import gc_heap_dump_route


def test_fastapi_route_downloads_gzip_jsonl() -> None:
    app = FastAPI()
    app.add_api_route("/debug/gc-heap-dump", gc_heap_dump_route(), methods=["GET"])

    response = TestClient(app).get("/debug/gc-heap-dump?include_referents=false")

    assert response.status_code == 200
    assert response.headers["content-type"] == "application/gzip"
    assert "content-length" not in response.headers
    payload = gzip.decompress(response.content).decode("utf-8")
    records = [json.loads(line) for line in payload.splitlines()]
    assert records[0]["format"] == "pygco-dump-jsonl"
    assert records[-1]["phase"] == "end"
