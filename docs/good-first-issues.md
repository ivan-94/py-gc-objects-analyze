# Good First Issue Candidates

These candidates are intentionally scoped so a new contributor can work from public fixtures and local tests without private dumps.

## Docs

- Add a troubleshooting example for one CLI error, with the command, expected error, and next step.
- Add a short explanation of one Web UI page to `docs/web-ui-walkthrough.md`.
- Improve one generated screenshot caption or alt text after reviewing `docs/assets/web-ui/`.

## Tests

- Add a CLI contract test for a documented error path.
- Add a Python producer test for a safe default such as `collect=false` or `include_repr=false`.
- Add a Web UI E2E assertion for a stable golden-fixture workflow.

## Fixtures

- Add a tiny public fixture for one documented edge case, such as missing referents, stubs, or read-only SQL examples.
- Add expected high-level output notes to docs without committing private or production dump data.

## Guardrails

Good first issues should not require:

- access to private dumps,
- publishing a release,
- changing release credentials,
- changing broad architecture,
- changing compatibility promises without maintainer review.
