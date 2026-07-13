# Versioning

`pygco` uses separate versioned contracts because the tool has several surfaces with different compatibility needs.

## Release Version

The CLI and Python package use semantic versions:

```text
0.1.1
```

Before 1.0, minor releases may still adjust behavior, but user-visible breaking changes must be called out in `CHANGELOG.md`.

## Dump Format

The Python producer writes gzip JSONL with a dump format version:

```text
pygco-dump-jsonl v1
```

Rules:

- Unknown major versions are rejected.
- Optional forward-compatible fields may be ignored by older readers.
- Required field removals or semantic changes require a new major dump format.

## SQLite Schema

SQLite databases are local rebuildable analysis artifacts. The schema has a version so `pygco` can produce clear errors and graceful fallbacks.

Rules:

- Old databases without newer optional tables should fail clearly or degrade gracefully.
- Long-term SQLite migrations are not a P0 promise.
- Re-importing from the original dump is the preferred recovery path.

## Algorithm Versions

Reachability and findings/report algorithms store explicit versions. When a scoring or interpretation algorithm changes, update the relevant version and changelog entry.

Current versions for 0.1.1:

- Dump format: `pygco-dump-jsonl` v1.
- SQLite schema: v1.
- Reachability algorithm: v1.
- Findings/report algorithms: v1.

## CLI and JSON Output

Human-readable CLI output may change to improve clarity. JSON output should avoid unannounced breaking changes once a command is documented as stable.

If a JSON field must change:

- document the change in `CHANGELOG.md`,
- add or update contract tests,
- prefer adding a field before removing or renaming one.

## Local API and Web UI

The local API is primarily consumed by the bundled Web UI. Generated OpenAPI JSON is tracked so drift is visible in CI, but the local API is not a hosted public service contract.

The Web UI is versioned with the release binary and embedded into release builds.
