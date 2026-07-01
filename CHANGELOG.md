# Changelog

## Source Manifest

- Product boundary: `README.md`, `docs/README.md`
- CLI contract: `docs/cli.md`
- API contract: `docs/api.md`
- Dump format contract: `docs/dump-format.md`
- SQLite schema contract: `docs/sqlite-schema.md`
- Analysis model: `docs/analysis-model.md`
- Release task list: `task.md`

## 0.1.0 - Unreleased

Initial local Python GC object memory forensics release.

Versioned contracts:

- Dump format: `pygco-dump-jsonl` v1.
- SQLite schema: schema version 1.
- Reachability algorithm: version 1.
- Findings/report algorithms: version 1.

Included:

- `pygco-dump` Python package with streaming gzip JSONL writer and FastAPI helper.
- `pygco` Rust CLI for import, summary, objects, object detail, edges, paths, diff, idset, SQL, schema, subgraph export, reports, doctor, web/open, and version.
- Fresh rebuildable SQLite analysis store with scoped snapshot/object ids.
- Local API server with common response/error envelopes, OpenAPI JSON, async jobs, cancellation, and embedded Web UI static assets.
- React Web UI for overview, objects, aggregate pages, object detail, graph, diff, findings, SQL/idset, schema, and report workflows.
- Golden and synthetic fixtures plus import/query/API benchmark scripts.

Known boundaries:

- Local single-user analysis only; no remote SaaS, login, RBAC, or sharing.
- SQLite analysis files are temporary rebuildable artifacts, not archival storage.
- Reachability values are bounded estimates and may be marked truncated or unavailable.
