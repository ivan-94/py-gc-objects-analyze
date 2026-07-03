# Maintenance

## Dependency Updates

Dependabot is configured for:

- GitHub Actions
- Cargo
- Web UI npm packages under `web/app`
- Python package metadata under `python/pygco_dump`

Patch and minor updates are grouped by ecosystem. Major updates stay separate by default and should be reviewed as compatibility work.

## Maintainer Response

For dependency PRs:

- Check the generated lockfile or metadata diff.
- Confirm the PR only touches the intended ecosystem.
- Run or wait for CI gates relevant to the ecosystem.
- For major updates, read upstream release notes and check whether docs, generated files, or compatibility notes need updates.
- For release tooling updates, run installer/package smoke checks before merging.

Minimum expected gates:

- Docs: `python3 scripts/check_docs_commands.py`
- Rust: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --workspace`
- Python: `ruff check python/pygco_dump`, `python -m pytest python/pygco_dump`
- Web: `(cd web/app && corepack pnpm build)`, `(cd web/app && corepack pnpm test:e2e)`
- Release: `scripts/test_install.sh`, `scripts/package_release.sh`

## Release Operations

Use `docs/release-acceptance.md` for release candidate acceptance and `docs/release-provenance.md` for artifact verification expectations.
