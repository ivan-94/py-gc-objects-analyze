# py-gc-objects-analyze

`py-gc-objects-analyze` is a local Python GC object memory forensics tool.

The project is document-driven. Start with [docs/README.md](docs/README.md).

Release-facing docs:

- [Install and build](docs/install.md)
- [Known limitations](docs/known-limitations.md)
- [Changelog](CHANGELOG.md)

First-version product boundary:

- Python runtime side only produces low-impact GC object dumps.
- Rust local tooling imports, indexes, aggregates, queries, diffs, and serves the API.
- React + shadcn/ui provides the local Web UI.
- SQLite is a temporary, rebuildable analysis database, not an archival source of truth.
- `pygco open <dump...>` is the main exploratory workflow.
