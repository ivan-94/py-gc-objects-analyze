# Compatibility

This page summarizes what compatibility users can expect from the first open source releases.

## Supported Runtime Inputs

- Python producer package: Python `>=3.10`.
- Dump files: gzip JSONL produced by `pygco_dump`.
- CLI analysis store: SQLite databases produced by `pygco import` or `pygco open`.

## Binary Targets

P0 release binaries are planned for:

- Linux x86_64
- macOS x86_64
- macOS Apple Silicon

Windows, Linux arm64, Docker images, and Homebrew distribution are not P0 targets.

## Stable Enough for Automation

These surfaces should be treated conservatively:

- documented CLI flags,
- documented JSON outputs,
- dump format major version,
- release artifact names,
- installer environment variables.

Changes to these surfaces require docs and tests in the same PR.

## Rebuildable Artifacts

SQLite analysis databases and cached `pygco open` sessions are rebuildable. Keep the original dump files if you need to reproduce or share an investigation.

If an older SQLite database is missing newer optional tables, commands should either degrade gracefully or return a clear error with a next step.

## Non-goals

The first releases do not promise:

- remote multi-user server compatibility,
- stable internal Rust crate APIs,
- stable Web UI component internals,
- long-term SQLite migration support,
- compatibility with dumps from unrelated tools.
