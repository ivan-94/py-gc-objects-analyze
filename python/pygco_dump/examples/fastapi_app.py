from __future__ import annotations

from fastapi import FastAPI

from pygco_dump.fastapi import gc_heap_dump_route

app = FastAPI()
app.add_api_route("/debug/gc-heap-dump", gc_heap_dump_route(), methods=["GET"])
