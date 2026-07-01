# Cache Sessions and Session Listing Spec

## Source Manifest

### Sources

- User request: install the current project for local use, then discuss where large SQLite analysis files should live and how they should be cleaned.
- User decision: prefer a cross-platform cache location under `~/.cache`, not a macOS-specific application support directory.
- User request: add a `sessions list` subcommand.
- User request: generate the complete spec under `docs/specs/`.
- Repository overview: `README.md`.
- Install/build behavior: `docs/install.md`.
- CLI contract and current `pygco open` semantics: `docs/cli.md`.
- Quickstart session cleanup guidance: `docs/quickstart.md`.
- Dump and SQLite lifecycle model: `docs/data-model.md`.
- Runtime safety and temporary-file requirements: `docs/runtime-safety.md`.
- Current CLI implementation: `crates/pygco-cli/src/main.rs`.
- Current API session endpoint and Web API routing: `crates/pygco-api/src/lib.rs`.
- Agent workflow and Source Manifest requirements: `~/.agents/docs/agents/workflows.md`, `~/.agents/docs/agents/handoff-policy.md`.

### Produced Artifacts

- `docs/specs/2026-07-01-cache-sessions-spec.md`

### Key Decisions

- Treat analysis SQLite files created by `pygco open` as cache artifacts, not project source artifacts.
- Use a cross-platform cache root priority: `PYGCO_HOME`, then `XDG_CACHE_HOME/pygco`, then `~/.cache/pygco`.
- Keep `pygco import -o <path>` explicit and unchanged.
- Implement `pygco sessions list` as the first management command.
- Use `manifest.json` beside each cache session so listing and future cleanup do not need to open every SQLite file.
- Defer `sessions delete`, `sessions clean`, and Web UI session management to later slices, but design the manifest and cache layout so they can be added without migration.

### Verification Evidence

- Source inspection performed with `rg`, `sed`, and `git status`.
- Current behavior confirmed from `crates/pygco-cli/src/main.rs`: `pygco open` creates `.pygco/sessions/<timestamp>/analysis.sqlite` when `--session-dir` is omitted.
- Current docs confirm SQLite is a temporary, rebuildable analysis database and can be deleted after use.
- No implementation or test command was run for this spec because this artifact is design-only.

### Open Questions / Risks

- Whether `PYGCO_HOME` should be renamed to a more cache-specific variable such as `PYGCO_CACHE_DIR`. This spec keeps `PYGCO_HOME` because it was already discussed, but the implementation can add `PYGCO_CACHE_DIR` later if needed.
- Whether future Web UI session deletion should physically delete immediately or move to a trash directory first. This spec leaves trash as a future option.
- Existing sessions under project-local `.pygco/sessions` will not be auto-migrated in the first implementation.

## Problem

`pygco` creates large SQLite analysis databases from Python GC dump files. These databases are intentionally temporary and rebuildable, but the current default `pygco open` location is project-local:

```text
.pygco/sessions/<timestamp>/analysis.sqlite
```

That location has three problems:

- Large cache files are scattered across repositories.
- Users need to remember which project directory produced a session before they can inspect or delete it.
- Future Web UI cleanup and selection features need a stable, user-level inventory.

The tool should make cached analysis sessions discoverable and manageable without changing the explicit import workflow.

## Goals

- Move the default `pygco open` session location to a user-level cache root.
- Use cross-platform, XDG-compatible cache semantics instead of macOS-specific paths.
- Write durable session metadata beside every default cache session.
- Add `pygco sessions list` to discover cached analysis sessions.
- Preserve existing explicit workflows such as `pygco import -o analysis.sqlite` and `pygco web analysis.sqlite`.
- Set up a stable foundation for future delete, clean, and Web UI session management.

## Non-Goals

- Do not migrate existing project-local `.pygco/sessions` directories automatically.
- Do not make SQLite a long-term archive format.
- Do not store raw dump contents inside the cache session.
- Do not implement `sessions delete`, `sessions clean`, or Web UI session selection in the first slice.
- Do not change the SQLite schema solely for cache session management.
- Do not add authentication, sharing, multi-user workspaces, or remote session management.

## Cache Root

The default cache root is resolved in this order:

```text
1. $PYGCO_HOME
2. $XDG_CACHE_HOME/pygco
3. ~/.cache/pygco
```

Rules:

- Empty environment variable values are ignored.
- Relative environment variable values are rejected with an actionable error.
- If the home directory cannot be discovered and neither environment variable is usable, `pygco open` and `pygco sessions list` return an actionable error.
- The cache root is created lazily when `pygco open` creates a default session.
- `pygco sessions list` succeeds with an empty result if the cache root or `sessions/` directory does not exist.

## Directory Layout

Default `pygco open` sessions are stored under:

```text
<cache-root>/
  sessions/
    <session-id>/
      analysis.sqlite
      import.log
      manifest.json
```

The first implementation should use this session id format:

```text
YYYYMMDDTHHMMSSZ-<short-random>
```

Example:

```text
~/.cache/pygco/
  sessions/
    20260701T213000Z-a1b2c3d4/
      analysis.sqlite
      import.log
      manifest.json
```

The random suffix prevents collisions when multiple sessions start in the same second.

## CLI Behavior

### `pygco open`

When `--session-dir` is omitted, `pygco open` creates the session under the cache root:

```text
<cache-root>/sessions/<session-id>/
```

When `--session-dir <path>` is provided, existing behavior is preserved:

- The provided path is used exactly as the session directory.
- No cache-root session id is assigned unless the path is under the cache root.
- A `manifest.json` should still be written if the session directory can be created.

`--cleanup-on-exit` remains supported:

- It removes the session directory after the Web/API server exits.
- If manifest writing succeeded, the manifest is removed with the directory.
- If cleanup fails, the command should warn but not mask the server result.

### `pygco import`

`pygco import -o <sqlite>` remains explicit and unchanged:

```bash
pygco import before.jsonl.gz after.jsonl.gz -o analysis.sqlite --rebuild
```

It does not automatically register the output as a cache session. This keeps scripted and project-local analysis flows predictable.

### `pygco web` and `pygco api`

`pygco web <sqlite>` and `pygco api <sqlite>` continue to accept any SQLite path. They do not require the database to be in the cache root.

Future Web UI session selection should only manage cache-root sessions by default, to avoid deleting arbitrary user-provided SQLite files.

## New Command: `pygco sessions list`

Add a top-level `sessions` command with a `list` subcommand:

```bash
pygco sessions list
```

The command scans:

```text
<cache-root>/sessions/*
```

Output should use the existing CLI output format conventions. With the current shared `OutputArgs` default, JSON remains the default; docs may recommend `--format table` for human scanning.

Recommended fields:

| Field | Meaning |
| --- | --- |
| `id` | Session directory name. |
| `created_at` | Creation time from `manifest.json`, or inferred from directory metadata if missing. |
| `last_opened_at` | Last known open time, if available. |
| `size_bytes` | Recursive size of the session directory. |
| `database_path` | Absolute path to `analysis.sqlite`. |
| `snapshot_count` | Snapshot count recorded in the manifest. |
| `object_count` | Total or primary snapshot object count, if recorded. |
| `source_dumps` | Source dump paths or URIs from the manifest. |
| `status` | `ready`, `missing-db`, `missing-manifest`, or `invalid-manifest`. |

Suggested table columns:

```text
id  created_at  size  snapshots  status  source_dumps
```

Suggested JSON shape:

```json
{
  "cache_root": "/home/example/.cache/pygco",
  "sessions": [
    {
      "id": "20260701T213000Z-a1b2c3d4",
      "created_at": "2026-07-01T21:30:00Z",
      "last_opened_at": "2026-07-01T21:35:00Z",
      "size_bytes": 123456789,
      "database_path": "/home/example/.cache/pygco/sessions/20260701T213000Z-a1b2c3d4/analysis.sqlite",
      "snapshot_count": 2,
      "object_count": 1000000,
      "source_dumps": [
        "before.jsonl.gz",
        "after.jsonl.gz"
      ],
      "status": "ready"
    }
  ]
}
```

Ordering:

- Default order is newest first by `created_at`.
- Sessions with missing or invalid timestamps sort after valid sessions.

Error tolerance:

- A malformed session should not fail the whole list.
- Missing or invalid manifest files should produce rows with `status`.
- Size calculation errors should leave `size_bytes` null in JSON and display `unknown` in table output.

## Manifest

Each `pygco open` session writes:

```text
manifest.json
```

Minimum schema:

```json
{
  "schema_version": 1,
  "session_id": "20260701T213000Z-a1b2c3d4",
  "created_at": "2026-07-01T21:30:00Z",
  "last_opened_at": "2026-07-01T21:30:05Z",
  "tool_version": "0.1.0",
  "cache_root": "/home/example/.cache/pygco",
  "session_dir": "/home/example/.cache/pygco/sessions/20260701T213000Z-a1b2c3d4",
  "database_path": "/home/example/.cache/pygco/sessions/20260701T213000Z-a1b2c3d4/analysis.sqlite",
  "import_log_path": "/home/example/.cache/pygco/sessions/20260701T213000Z-a1b2c3d4/import.log",
  "source_dumps": [
    "before.jsonl.gz",
    "after.jsonl.gz"
  ],
  "import_options": {
    "reachability_mode": "full",
    "profile": false
  },
  "snapshots": [
    {
      "snapshot_id": 1,
      "source_uri": "before.jsonl.gz",
      "dump_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
      "object_count": 100,
      "edge_count": 200,
      "shallow_size_sum": 3000,
      "stub_count": 0,
      "missing_referent_count": 0
    }
  ]
}
```

Rules:

- `manifest.json` is informational. SQLite remains the source for detailed query results.
- Paths should be absolute where they refer to generated files.
- Source dump entries should preserve user-provided paths or URIs, plus any import summary fields already available.
- Manifest writing failure should fail `pygco open` before the Web/API server starts, because listing and cleanup depend on the manifest.
- After a successful import, the manifest should include the import summary written to `import.log`.

## API and Web UI Foundation

The first slice does not need Web UI session management, but the design should not block it.

Future API endpoints can be added under:

```text
GET    /api/sessions
DELETE /api/sessions/:session_id
POST   /api/sessions/clean
```

Rules for future Web UI management:

- Only sessions under the resolved cache root are manageable by default.
- The UI must clearly say that deleting a session removes the SQLite analysis cache, not the original dump files.
- Deletion must refuse paths outside the cache root.
- The UI can open a cache session by launching or redirecting to a server bound to that session's database, but that routing model is out of scope for the first CLI-only slice.

## Cleanup Model For Future Slices

Future commands should follow this direction:

```text
pygco sessions delete <session-id>
pygco sessions clean --older-than 7d
pygco sessions clean --max-total-size 20gb
```

Cleanup rules:

- Never delete raw dump files.
- Refuse to delete paths outside `<cache-root>/sessions`.
- Prefer deleting whole session directories.
- If a session is currently being served, deletion should fail with an actionable message.
- A future trash directory can be added if immediate deletion feels too risky:

```text
<cache-root>/trash/
```

## Compatibility

The following commands remain valid:

```bash
pygco import dump.jsonl.gz -o analysis.sqlite --rebuild
pygco web analysis.sqlite
pygco api analysis.sqlite --no-browser
pygco open dump.jsonl.gz --session-dir .pygco/sessions/manual
```

Docs that currently mention `.pygco/sessions/<timestamp>` should be updated during implementation to describe the new cache-root default and the explicit `--session-dir` override.

Existing project-local sessions are not listed by default. Users can still open them directly:

```bash
pygco web .pygco/sessions/20260701T213000Z/analysis.sqlite
```

## Files To Modify

### Required Rust Code

- `crates/pygco-cli/src/main.rs`
  - Add the `sessions` top-level command and `list` subcommand.
  - Change `pygco open` default session directory from `.pygco/sessions/<timestamp>` to `<cache-root>/sessions/<session-id>`.
  - Write `manifest.json` after import succeeds and before serving starts.
  - Preserve `--session-dir`, `--cleanup-on-exit`, `pygco import`, `pygco web`, and `pygco api` behavior.
- `crates/pygco-cli/src/cache.rs` or equivalent new module
  - Resolve `PYGCO_HOME`, `XDG_CACHE_HOME`, and `~/.cache/pygco`.
  - Validate absolute environment override paths.
  - Create default session paths and collision-resistant session ids.
- `crates/pygco-cli/src/session_manifest.rs` or equivalent new module
  - Define the manifest data model.
  - Serialize `manifest.json`.
  - Read manifests for `sessions list`.
  - Represent degraded session statuses such as `missing-db`, `missing-manifest`, and `invalid-manifest`.
- `crates/pygco-cli/Cargo.toml`
  - Add any dependency needed by the chosen session id implementation, such as the workspace `uuid` crate if a random suffix is generated from UUIDs.

### Required Tests

- `crates/pygco-cli/tests/cli_contract.rs`
  - Add CLI tests for default cache-root sessions using temporary `PYGCO_HOME`.
  - Add tests for `--session-dir` preserving explicit paths.
  - Add tests for `--cleanup-on-exit`.
  - Add tests for `pygco sessions list --format json`.
  - Add tests for empty cache roots and damaged session directories.
- New unit tests near the new cache or manifest modules
  - Cover cache root resolution.
  - Cover relative environment variable rejection.
  - Cover manifest parsing and degraded statuses.

### Required Documentation

- `docs/cli.md`
  - Update `pygco open` default session location.
  - Document `pygco sessions list`.
  - Explain cache root resolution and `--session-dir` override.
- `docs/generated/cli-help.md`
  - Regenerate after the CLI surface changes.
- `docs/quickstart.md`
  - Replace project-local cleanup guidance with cache session discovery and deletion guidance.
  - Keep explicit manual deletion examples clearly scoped to cache session directories.
- `docs/install.md`
  - Update the first-analysis workflow note that currently says `pygco open` keeps `.pygco/sessions/<timestamp>/`.
- `docs/runtime-safety.md`
  - Update the temporary file section to describe cache-root sessions.
  - Preserve the `.tmp.sqlite` safety requirements.
- `docs/data-model.md`
  - Clarify that default `pygco open` SQLite files live in the user cache, while explicit `pygco import -o` paths remain user-chosen.
- `CHANGELOG.md`
  - Mention the behavior change for default `pygco open` session placement and the new `pygco sessions list` command.

### Optional Or Future Files

- `crates/pygco-api/src/lib.rs`
  - Not required for the first CLI-only slice.
  - Future Web UI session management would add `/api/sessions` endpoints here.
- `web/app/src/app/router.tsx`
  - Not required for the first CLI-only slice.
  - Future Web UI session management would add navigation for a session manager page.
- `web/app/src/pages/...`
  - Not required for the first CLI-only slice.
  - Future Web UI session management would add a page or panel for listing and deleting cache sessions.
- `web/app/src/generated/api-client.ts` and `web/app/src/generated/openapi.json`
  - Not required for the first CLI-only slice.
  - Future API additions would require regeneration.

## Implementation Plan

### Slice 1: Cache Root and Manifest

- Add a small module for cache path resolution.
- Change `pygco open` default session directory to `<cache-root>/sessions/<session-id>`.
- Generate collision-resistant session ids.
- Write `manifest.json` after import succeeds and before serving starts.
- Keep `import.log` behavior.
- Add unit tests for cache root resolution and session id format.
- Add CLI contract tests for default session location using temporary `PYGCO_HOME`.

### Slice 2: `pygco sessions list`

- Add `Command::Sessions(SessionsArgs)`.
- Add `SessionsCommand::List`.
- Scan `<cache-root>/sessions`.
- Parse `manifest.json` when present.
- Calculate recursive session size.
- Emit table and JSON using existing output conventions.
- Add tests for empty cache, valid manifest, missing database, missing manifest, and invalid manifest.

### Slice 3: Documentation

- Update `docs/cli.md`.
- Update `docs/quickstart.md`.
- Update `docs/install.md`.
- Update `docs/runtime-safety.md`.
- Regenerate generated CLI help if the project requires it.

### Future Slice: Cleanup

- Add `sessions delete`.
- Add `sessions clean`.
- Add active-session safety checks.
- Add docs and tests.

### Future Slice: Web UI Session Management

- Add API endpoints for listing and deleting cache sessions.
- Add a Web UI session management page.
- Keep arbitrary SQLite paths readable but not Web-manageable by default.

## Testing Strategy

Rust tests:

- Cache root resolution with `PYGCO_HOME`.
- Cache root resolution with `XDG_CACHE_HOME`.
- Fallback to `~/.cache/pygco`.
- Rejection of relative cache environment variable values.
- `pygco open` writes `analysis.sqlite`, `import.log`, and `manifest.json` under a temporary `PYGCO_HOME`.
- `pygco open --session-dir <path>` still honors explicit paths.
- `pygco open --cleanup-on-exit` removes the session directory.
- `pygco sessions list --format json` returns empty list for missing cache root.
- `pygco sessions list` handles valid, missing, and invalid manifests.

Docs checks:

- CLI docs mention the new cache root.
- Quickstart cleanup instructions use `pygco sessions list`.
- Runtime safety docs describe cache sessions and explicit cleanup.

Manual smoke:

```bash
CACHE_DIR="$(mktemp -d)"
PYGCO_HOME="$CACHE_DIR" pygco open fixtures/golden/tiny-v1.jsonl.gz --no-browser
```

In another terminal while the server is running:

```bash
PYGCO_HOME="$CACHE_DIR" pygco sessions list --format table
```

## Acceptance Criteria

- Running `pygco open dump.jsonl.gz --no-browser` without `--session-dir` creates a session under the resolved cache root.
- The session contains `analysis.sqlite`, `import.log`, and `manifest.json`.
- Running `pygco sessions list` shows that session.
- Running `pygco sessions list --format json` returns machine-readable session data including `cache_root` and `sessions`.
- Explicit `pygco import -o <path>` behavior is unchanged.
- Explicit `pygco open --session-dir <path>` behavior is preserved.
- Missing or damaged session directories do not prevent listing other sessions.
- Documentation explains that cached SQLite files are safe to delete because source dumps are the durable input.

## Rollout Notes

This is a behavior change for `pygco open` only. Users who expect sessions in the project directory can keep that behavior with:

```bash
pygco open dump.jsonl.gz --session-dir .pygco/sessions/manual
```

The change should be mentioned in `CHANGELOG.md` when implemented.
