# Issue Triage

Use this guide to route incoming issues without asking for private dumps by default.

## First Response Checklist

- Confirm `pygco version`.
- Confirm OS and CPU architecture.
- Confirm Python version when `pygco-dump` is involved.
- Ask for the exact command and output.
- Ask whether the dump can be shared. Do not assume it can.
- If the dump cannot be shared, ask for a minimal synthetic reproduction or redacted command output.

## Private Dumps

Dumps may include type names, module names, object sizes, references, and optionally object `repr`. Treat them as potentially sensitive.

Do not request private dumps in public issues. Use the security reporting path if a dump contains secrets, customer data, or credentials.

## Performance Reports

Ask for:

- object and edge counts,
- dump gzip size,
- SQLite size,
- command with `--profile` when available,
- machine CPU, memory, OS, and storage type,
- whether reachability was enabled.

Prefer synthetic fixtures for reproducible regressions.

## Web UI Visual Regressions

Ask for:

- browser and version,
- viewport size,
- route URL without sensitive query values,
- screenshot,
- whether the issue reproduces on golden fixtures.

## Compatibility Questions

Route compatibility questions to `docs/versioning.md` and `docs/compatibility.md`. If behavior differs from those docs, treat it as a docs or contract bug.
