# py-gc-objects-analyze Docs

`py-gc-objects-analyze` is a local, offline Python GC object memory forensics tool. The target user journey is:

```text
Install pygco
  -> collect one or more gzip JSONL dumps with pygco-dump
  -> import into a temporary SQLite analysis database
  -> investigate through CLI or the local Web UI
```

## Start Here

- [Quickstart](quickstart.md)
- [Demo transcript](demo.md)
- [Install and build](install.md)
- [Python producer integration](producer-integration.md)
- [Runtime safety](runtime-safety.md)
- [Known limitations](known-limitations.md)
- [Troubleshooting](troubleshooting.md)
- [Release acceptance](release-acceptance.md)

## User Guide

- [Core concepts](concepts.md)
- [Web UI walkthrough](web-ui-walkthrough.md)
- [Performance model](performance.md)
- [Versioning](versioning.md)
- [Compatibility](compatibility.md)
- [Testing strategy](testing.md)

## Reference

- [CLI contract](cli.md)
- [Generated CLI help](generated/cli-help.md)
- [Local API contract](api.md)
- [Generated OpenAPI JSON](generated/openapi.json)
- [Dump format](dump-format.md)
- [SQLite schema](sqlite-schema.md)
- [Dump and SQLite data model](data-model.md)
- [Analysis model](analysis-model.md)
- [Web UI contract](web-ui.md)

## Developer Docs

- [Architecture](architecture.md)
- [Engineering standards](project/engineering-standards.md)
- [Source Manifest guidance](project/source-manifest.md)
- [Issue triage](triage.md)
- [Good first issue candidates](good-first-issues.md)
- [Maintenance](maintenance.md)
- [Release provenance](release-provenance.md)
- [Release acceptance](release-acceptance.md)
- [References](references/README.md)

## Specs And Project Archive

Current specs:

- [Open Source Release Readiness Spec](specs/2026-07-02-open-source-release-readiness-spec.md)
- [CLI Leak Workflow Remediation Spec](specs/2026-07-02-cli-leak-workflow-remediation-spec.md)
- [Cache Sessions Spec](specs/2026-07-01-cache-sessions-spec.md)

Project/archive material:

- [CLI diagnostics workbench](cli-diagnostics-workbench.md)
- [CLI diagnostics technical spec](project/cli-diagnostics-technical-spec.md)
- [Implementation blueprint](project/implementation-blueprint.md)
- [POC migration guide](project/poc-migration-guide.md)
- [POC retrospective](poc-retrospective.md)
- [Release task list](project/task.md)

## First-Version Boundaries

- Command name: `pygco`
- Python distribution: `pygco-dump`
- Python import name: `pygco_dump`
- Main flow: `pygco open dump-a.jsonl.gz dump-b.jsonl.gz`
- Explicit flow: `pygco import dump-a.jsonl.gz dump-b.jsonl.gz -o analysis.sqlite --rebuild`, then `pygco web analysis.sqlite`
- Release binaries embed the React Web UI.
- Development uses a Rust API server plus a React dev server.
