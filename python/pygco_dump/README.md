# pygco-dump

`pygco-dump` writes low-impact Python GC object dumps in the `pygco-dump-jsonl` v1 format.

```python
from pathlib import Path
from pygco_dump import write_gc_dump

with Path("heap.jsonl.gz").open("wb") as file:
    write_gc_dump(file)
```

FastAPI:

```python
from fastapi import FastAPI
from pygco_dump.fastapi import gc_heap_dump_route

app = FastAPI()
app.add_api_route("/debug/gc-heap-dump", gc_heap_dump_route(), methods=["GET"])
```

Framework-agnostic integrations only need a binary file-like object. See `examples/` for
plain file, WSGI, ASGI, and FastAPI variants.
