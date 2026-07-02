# Contributing

Thanks for helping improve `py-gc-objects-analyze`. This project is document-driven and test-driven: user-visible behavior, CLI contracts, SQLite schema, APIs, Web UI behavior, report formats, test strategy, and release flow should be documented before implementation changes land.

## Development Setup

Prerequisites:

- Rust stable toolchain.
- Python 3.10 or newer for `pygco-dump`.
- Node.js 22 with Corepack for the Web UI.

Useful commands:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
python -m pip install -e "python/pygco_dump[fastapi,test]"
python -m pytest python/pygco_dump
(cd web/app && corepack pnpm install --frozen-lockfile)
(cd web/app && corepack pnpm build)
python3 scripts/check_docs_commands.py
```

Generated docs:

```bash
cargo build -p pygco-cli
python3 scripts/generate_cli_docs.py
python3 scripts/export_openapi.py
python3 scripts/check_docs_commands.py
```

## Pull Request Expectations

- Keep changes focused and explain the user-visible effect.
- Update docs/specs before changing documented behavior.
- Add or update tests for Rust, Python, Web UI, or release tooling changes.
- Regenerate CLI/OpenAPI docs when CLI help or API schema changes.
- Record compatibility impact for CLI JSON, dump format, SQLite schema, API, report output, and release artifacts.
- Include a Source Manifest in durable artifacts such as PRDs, issues, HAT guides, PR bodies, and handoff docs.

## Test Matrix By Area

- Rust CLI/analysis/store/importer/API: `cargo test --workspace`.
- Rust style: `cargo fmt --check` and `cargo clippy --all-targets --all-features -- -D warnings`.
- Python producer: `python -m pytest python/pygco_dump`.
- Web UI: `(cd web/app && corepack pnpm build)` and `(cd web/app && corepack pnpm test:e2e)`.
- Docs contracts: `python3 scripts/check_docs_commands.py`.
- Release tooling: installer tests, release archive smoke, and clean-machine HAT when release artifacts change.

## Handling Dumps

Do not attach private dumps to public issues unless you are certain they are safe to share. Dumps can contain sensitive object metadata. Prefer synthetic fixtures or reduced reproductions.

For issue routing, see [docs/triage.md](docs/triage.md). For compatibility expectations, see [docs/versioning.md](docs/versioning.md) and [docs/compatibility.md](docs/compatibility.md).
