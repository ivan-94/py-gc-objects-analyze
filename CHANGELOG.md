# Changelog

## 0.1.1 - Unreleased

Fixes:

- `pygco-dump` freezes its main object census before creating streaming serializer state, excludes producer-owned
  snapshot containers from dumps, and avoids the O(N) object-id index when referent stubs are disabled.
- The object graph UI supports filtering visible graph nodes by type and text, with matching end-to-end coverage.

## 0.1.0

Initial local Python GC object memory forensics release.

Versioned contracts:

- Dump format: `pygco-dump-jsonl` v1.
- SQLite schema: schema version 1.
- Reachability algorithm: version 1.
- Findings/report algorithms: version 1.

Included:

- `pygco-dump` Python package with streaming gzip JSONL writer and FastAPI helper.
- `pygco` Rust CLI for import, cache session listing, summary, objects, object detail, edges, paths, diff, idset, SQL, schema, subgraph export, reports, doctor, web/open, and version.
- Fresh rebuildable SQLite analysis store with scoped snapshot/object ids.
- `pygco open` stores default temporary analysis sessions under the user cache root (`PYGCO_HOME`, `XDG_CACHE_HOME/pygco`, or `~/.cache/pygco`) with a `manifest.json` for discovery.
- Local API server with common response/error envelopes, OpenAPI JSON, async jobs, cancellation, and embedded Web UI static assets.
- React Web UI for overview, objects, aggregate pages, object detail, graph, diff, findings, SQL/idset, schema, and report workflows.
- Golden and synthetic fixtures plus import/query/API benchmark scripts.

Release artifacts:

- `install.sh`
- `pygco-0.1.0-x86_64-unknown-linux-gnu.tar.gz`
- `pygco-0.1.0-x86_64-apple-darwin.tar.gz`
- `pygco-0.1.0-aarch64-apple-darwin.tar.gz`
- per-archive `.sha256` files
- `checksums.txt`

Verify a downloaded archive with the adjacent checksum file:

```bash
sha256sum -c pygco-0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256
```

On macOS:

```bash
shasum -a 256 -c pygco-0.1.0-aarch64-apple-darwin.tar.gz.sha256
```

Known boundaries:

- Local single-user analysis only; no remote SaaS, login, RBAC, or sharing.
- SQLite analysis files are temporary rebuildable artifacts, not archival storage.
- Reachability values are bounded estimates and may be marked truncated or unavailable.
