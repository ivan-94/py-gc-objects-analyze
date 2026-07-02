# Quickstart

This guide is for a first-time user running a local Python GC object memory investigation.

## 1. Install The Tools

Install `pygco` from GitHub Releases:

```bash
curl -fsSL https://github.com/ivan-94/py-gc-objects-analyze/releases/latest/download/install.sh | sh
```

Install the Python dump producer in the environment that runs the target process:

```bash
python -m pip install "pygco-dump[fastapi]"
```

See [Install and build](install.md) for manual binary install, source builds, upgrades, uninstalls, and release verification.

## 2. Run A Fixture First

Before touching a service, confirm the local analyzer works:

```bash
pygco open fixtures/golden/tiny-v1.jsonl.gz --no-browser
```

Open the printed local URL. `pygco open` will:

1. Create a new temporary analysis session.
2. Import the dump into a fresh SQLite database.
3. Build indexes and basic analysis data.
4. Start a local API server and Web UI.

Default sessions live under `PYGCO_HOME`, `XDG_CACHE_HOME/pygco`, or `~/.cache/pygco`:

```text
<cache-root>/sessions/<timestamp-random>/
  analysis.sqlite
  import.log
  manifest.json
```

## 3. Add A FastAPI Dump Endpoint

Inside the Python service you want to inspect:

```python
from pygco_dump.fastapi import gc_heap_dump_route

app.add_api_route(
    "/debug/gc-heap-dump",
    gc_heap_dump_route(),
    methods=["GET"],
)
```

The endpoint only streams gzip JSONL dumps. It does not aggregate, analyze, redact, authorize, or schedule collection.

Do not expose this endpoint to untrusted users. Dumps can contain sensitive object metadata, and `collect=true` may affect service latency. Read [Runtime safety](runtime-safety.md) and [Python Producer integration](producer-integration.md) before using this against shared or production services.

FastAPI is the smallest HTTP example. Celery workers, Gunicorn/uWSGI `prefork`, management commands, and daemon processes can call the lower-level `write_gc_dump()` API directly. Multi-process services should collect per-PID dumps when process identity matters.

## 4. Collect Before And After Dumps

```bash
curl -o before.jsonl.gz "http://service/debug/gc-heap-dump?collect=false"
curl -o after.jsonl.gz "http://service/debug/gc-heap-dump?collect=false"
```

Use `collect=true` only when you intentionally want the target process to run GC before dumping.

## 5. Open The Web UI

```bash
pygco open before.jsonl.gz after.jsonl.gz
```

If you need a reproducible command sequence or automation-friendly output, use the explicit flow:

```bash
pygco import before.jsonl.gz after.jsonl.gz -o analysis.sqlite --rebuild
pygco summary analysis.sqlite
pygco diff analysis.sqlite --from 1 --to 2
pygco web analysis.sqlite
```

If `analysis.sqlite` already exists, `pygco import` fails unless you pass `--rebuild`.

## 6. Investigation Order

1. Overview: confirm object count, edge count, shallow size, top types, and top modules.
2. Diff: identify growing types, modules, cohorts, and object lifecycle confidence.
3. Objects: sort by reachable size, shallow size, in edges, or out edges.
4. Object detail: inspect referents, referrers, and local reference graph.
5. Owner paths: sample bounded retaining or referent paths.
6. Findings and leads: treat heuristics as candidates, not proof.
7. SQL and idsets: run temporary read-only validation queries when needed.

## 7. Delete Cached Sessions

SQLite analysis files are temporary. Keep the original dump files, then remove sessions when no longer needed:

```bash
pygco sessions list --format table
rm -rf ~/.cache/pygco/sessions/<session-id>
```
