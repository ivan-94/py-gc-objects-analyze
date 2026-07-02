# Open Source Release Readiness Spec

## Source Manifest

### Sources

- User request: review the repository as an open source project and produce a detailed P0/P1 plan under `docs/specs/`.
- User decisions:
  - P0 should publish `pygco` as GitHub Releases binary artifacts and `pygco-dump` on PyPI.
  - Do not publish the Rust CLI or internal crates to crates.io in P0.
  - Do not ship Docker or Homebrew in P0/P1.
  - Provide a general installer command based on `curl ... | sh`.
  - The installer script should be a GitHub Release asset, addressed through `https://github.com/ivan-94/py-gc-objects-analyze/releases/latest/download/install.sh`.
  - Keep binary release automation lightweight: Linux native build on Ubuntu; macOS Apple Silicon and Intel artifacts from one macOS runner when Rust cross-compilation is straightforward.
- Agent workflow requirements: `~/.agents/docs/agents/workflows.md`, `~/.agents/docs/agents/handoff-policy.md`.
- Repository guidance: `AGENTS.md`.
- Current project entrypoints: `README.md`, `docs/README.md`, `docs/install.md`, `docs/quickstart.md`, `docs/producer-integration.md`, `docs/runtime-safety.md`, `docs/known-limitations.md`, `docs/testing.md`, `docs/architecture.md`, `docs/performance.md`, `CHANGELOG.md`.
- Current implementation and contracts: `Cargo.toml`, `crates/pygco-cli/Cargo.toml`, `crates/pygco-cli/src/main.rs`, `crates/pygco-api/build.rs`, `python/pygco_dump/pyproject.toml`, `python/pygco_dump/README.md`, `web/app/package.json`, `.github/workflows/ci.yml`, `justfile`.
- Existing release/task material: `docs/project/task.md`, `docs/project/engineering-standards.md`.

### Produced Artifacts

- `docs/specs/2026-07-02-open-source-release-readiness-spec.md`
- `README.md`, `docs/README.md`, `docs/install.md`, `docs/quickstart.md`, `docs/release-acceptance.md`
- P1 docs: `docs/troubleshooting.md`, `docs/versioning.md`, `docs/compatibility.md`, `docs/triage.md`, `docs/maintenance.md`, `docs/release-provenance.md`
- `LICENSE`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`
- `.github/ISSUE_TEMPLATE/*.yml`, `.github/pull_request_template.md`
- `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `.github/workflows/release-acceptance.yml`, `.github/workflows/publish-python.yml`, `.github/workflows/benchmarks.yml`
- `.github/dependabot.yml`, `.github/labels.yml`
- `scripts/install.sh`, `scripts/package_release.sh`, `scripts/test_install.sh`, `scripts/render_release_notes.py`, `scripts/capture_web_screenshots.sh`, `scripts/release_preflight.sh`
- HAT reports: `docs/hats/2026-07-02-release-acceptance-v0.1.0-rc.3.md`
- release/package metadata updates in `Cargo.toml`, `crates/*/Cargo.toml`, `python/pygco_dump/pyproject.toml`, `justfile`

### Key Decisions

- Treat the open source launch as a product release readiness project, not as a code feature.
- P0 release artifacts are:
  - GitHub Release assets for the `pygco` CLI binary, with embedded Web UI.
  - A GitHub Release asset named `install.sh`.
  - Checksums for all downloadable release assets.
  - PyPI package `pygco-dump`.
- P0 install path is `curl -fsSL https://github.com/ivan-94/py-gc-objects-analyze/releases/latest/download/install.sh | sh`, plus documented manual install.
- The Rust workspace crates remain internal implementation boundaries for 0.1.0. crates.io publishing is explicitly out of scope.
- Docker, Homebrew, remote SaaS, hosted docs infrastructure, and signing/provenance hardening are not P0 blockers.
- Existing docs already contain useful internal material, but the open source entry path must be rewritten around a new user journey.

### Verification Evidence

- Reviewed current README, docs, CI workflow, package metadata, release checklist, testing strategy, architecture docs, and implementation entrypoints with `rg`, `sed`, `find`, and `git status`.
- Updated release-facing documentation, metadata, governance files, installer/package scripts, lightweight unit-test CI, binary release workflow, and Python publishing workflow.
- Re-ran the full local regression set on 2026-07-02 after adding screenshots, demo docs, troubleshooting links, and release notes verification guidance.
- Local verification completed so far:
  - `python3 scripts/check_docs_commands.py`
  - `cargo fmt --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test --workspace`
  - `scripts/test_install.sh`
  - `(cd web/app && corepack pnpm install --frozen-lockfile)`
  - `(cd web/app && corepack pnpm build)`
  - `CI=1 pnpm --dir web/app test:e2e`
  - `. .scratch/build-venv/bin/activate && ruff check python/pygco_dump && python -m pytest python/pygco_dump`
  - `PYGCO_WEB_DIST="$PWD/web/app/dist" cargo build --release -p pygco-cli`
  - `scripts/package_release.sh`
  - `./target/release/pygco open fixtures/golden/tiny-v1.jsonl.gz --no-browser --port 0 --cleanup-on-exit`
  - clean virtualenv wheel install from `python/pygco_dump/dist/pygco_dump-0.1.0-py3-none-any.whl[fastapi]`
  - `ruby -e 'require "yaml"; ... YAML.load_file(...)'` for GitHub workflows and issue templates
  - local benchmark command smoke for `fixtures/synthetic/medium.jsonl.gz`, `benches/import_benchmark.py`, and `benches/query_api_benchmark.py`
  - `CI=1 scripts/capture_web_screenshots.sh .scratch/web-ui-screenshots-check-quiet`
  - `CI=1 scripts/capture_web_screenshots.sh docs/assets/web-ui`
  - Manual visual review of `docs/assets/web-ui/{overview,object-detail,graph,diff,sql,report}.png`
  - Troubleshooting spot checks: malformed dump import exited `10`, read-only SQL rejection exited `20`, invalid CLI argument exited `2`
  - `scripts/release_preflight.sh` was run read-only; after push it exits `0` and reports default branch, draft release, workflow state, and current PyPI absence.
- GitHub external verification completed after the repository was pushed:
  - `main` is pushed to `ivan-94/py-gc-objects-analyze`.
  - GitHub Actions CI run `28582013621` passed on commit `9c38ba0`.
  - Pull request CI run `28582123395` passed on a Dependabot PR.
  - After maintainer feedback, PR/push CI was simplified to unit checks only; docs command, release artifact, Web E2E, and benchmark checks stay in local release preparation or dedicated release/benchmark workflows.
  - Rust PR/push CI uses `cargo test --workspace --lib --bins` so integration/contract tests do not block routine PRs.
  - GitHub Actions CI run `28587516812` passed on commit `a81fcdf` with only `rust-unit`, `python-unit`, and `web-unit`.
  - Manual benchmark workflow run `28581056797` passed.
  - GitHub Release workflow run `28582024816` passed for tag `v0.1.0-rc.2` and produced a draft release.
  - `gh release download v0.1.0-rc.2 --repo ivan-94/py-gc-objects-analyze --dir .scratch/rc2` downloaded `install.sh`, `checksums.txt`, three target archives, and their per-archive `.sha256` files.
  - `cd .scratch/rc2 && shasum -a 256 -c checksums.txt` passed for all three target archives.
  - The downloaded macOS Apple Silicon archive ran `pygco version`, `pygco import fixtures/golden/tiny-v1.jsonl.gz`, and `pygco summary`.
  - GitHub Release workflow run `28584497200` passed for tag `v0.1.0-rc.3` after fixing installer stamping.
  - `gh release view v0.1.0-rc.3 --json assets` shows `install.sh`, `checksums.txt`, and Linux x86_64, macOS x86_64, and macOS Apple Silicon archives plus per-archive `.sha256` files.
  - `cd .scratch/rc3 && shasum -a 256 -c checksums.txt` passed for all three `v0.1.0-rc.3` archives.
  - The `v0.1.0-rc.3` installer asset is stamped with `DEFAULT_VERSION="0.1.0-rc.3"` and still keeps the unstamped sentinel check.
  - GitHub draft release notes for `v0.1.0-rc.3` were updated with version-stamped artifact names and checksum commands.
  - Manual Linux release acceptance workflow run `28586150834` passed on Ubuntu 24.04.4 x86_64 and uploaded import/summary/objects/diff/report/open artifacts.
  - macOS Apple Silicon HAT installed from downloaded release assets and ran `version`, `open`, import, summary, objects, diff, and report commands.
  - macOS x86_64 archive ran `version`, `import`, and `summary` under Rosetta.
  - Clean local-wheel FastAPI helper smoke produced an `application/gzip` dump stream with metadata start/end records.
  - `publish-python` workflow dispatch run `28586504894` built `pygco_dump-0.1.0.tar.gz` and `pygco_dump-0.1.0-py3-none-any.whl`, passed `twine check`, and tested the built wheel.
  - The same TestPyPI publish rehearsal failed at the upload step with `invalid-publisher`; the reported trusted publisher claim was `repo:ivan-94/py-gc-objects-analyze:environment:testpypi`.
  - Retried TestPyPI publish rehearsal run `28589579759`; package build, `twine check`, artifact upload, and built-wheel smoke passed again. Upload still failed with `invalid-publisher`; claims were `repository=ivan-94/py-gc-objects-analyze`, `workflow_ref=ivan-94/py-gc-objects-analyze/.github/workflows/publish-python.yml@refs/heads/main`, `ref=refs/heads/main`, and `environment=testpypi`.
  - Release workflow dry run `28587055092` completed with `tag=dry-run`, built release artifacts/notes, and skipped draft release creation.
  - Release workflow dry run `28587529503` completed with `tag=dry-run-attest` on commit `a81fcdf`; `Attest release artifacts` created provenance for 8 subjects.
  - `gh attestation verify .scratch/dry-run-attest/release-linux/pygco-dry-run-attest-x86_64-unknown-linux-gnu.tar.gz --repo ivan-94/py-gc-objects-analyze --format json` verified the Linux dry-run archive against `release.yml@refs/heads/main`, commit `a81fcdf`, run `28587529503`, and SLSA provenance subjects for all release assets.
  - `sed 's#dist/##' pygco-dry-run-attest-x86_64-unknown-linux-gnu.tar.gz.sha256 | shasum -a 256 -c -` passed for the downloaded Linux dry-run archive.
  - Final local unit-only verification after documentation evidence updates: `cargo test --workspace --lib --bins`, `. .scratch/build-venv/bin/activate && python -m pytest python/pygco_dump`, and `pnpm --dir web/app test`.
  - Latest unit-only push CI run `28588236402` passed on commit `9ddb0e7`.
  - Dependency automation review used `gh pr list --state open --json number,title,author,headRefName,url` and `gh pr view <number> --json files`; npm update PRs expose `web/app/pnpm-lock.yaml`, Cargo update PRs expose `Cargo.lock`, and GitHub Actions update PRs touch workflow YAML only.
- Chrome DOM verification was run without screenshots:
  - Repository page rendered README install/quickstart content, `releases/latest/download/install.sh`, the `pygco-dump[fastapi]` install command, and license/contributing/security entry links.
  - Rendered docs pages for install, quickstart, runtime safety, troubleshooting, and contributing were reachable from GitHub and showed the expected section headings/content.
  - Issue template chooser rendered bug, memory investigation feedback, documentation issue, feature request, and release checklist templates.
  - Draft release page rendered as `v0.1.0-rc.2`; GitHub kept the full asset list in a loading state during DOM inspection, so complete asset evidence comes from `gh release download` and checksum verification.
  - Local Web UI opened from the HAT `pygco web: http://127.0.0.1:3776/` URL and rendered Overview, Objects, Object Graph, Diff, SQL, and Report content without screenshots.
- GitHub labels and tracker issues were created for all P0/P1 slices.

### Open Questions / Risks

- The GitHub owner/repository URL is set to `https://github.com/ivan-94/py-gc-objects-analyze`.
- The implemented P0 binary target matrix is Linux x86_64, macOS x86_64, and macOS Apple Silicon. The `v0.1.0-rc.3` tag run produced all three; macOS x86_64 has a Rosetta runtime smoke but a physical Intel Mac remains useful before non-draft publication.
- PyPI project ownership and Trusted Publishing configuration happen outside the repository and must be completed by a maintainer.
- Release artifact signing and SBOM generation remain optional hardening, not P0 blockers. Checksums are a P0 blocker, and GitHub artifact attestations are now validated for future release workflow runs.
- Current external preflight status:
  - `gh` is authenticated as `ivan-94`.
  - `main` is pushed and protected by CI workflow definitions.
  - Draft release `v0.1.0-rc.3` exists with complete downloadable artifacts.
  - Release acceptance HAT report exists at `docs/hats/2026-07-02-release-acceptance-v0.1.0-rc.3.md`.
  - Tracker issues exist for all P0/P1 slices.
  - `pygco-dump` is not visible on PyPI.

## Background

`py-gc-objects-analyze` is close to a usable local memory forensics tool: it has a Rust CLI, Python dump producer, embedded React Web UI, SQLite analysis store, generated CLI/OpenAPI docs, fixtures, benchmarks, and CI coverage. The open source gap is not primarily algorithmic. The gap is that a new external user cannot yet confidently answer:

- What problem does this solve?
- Is it safe to try against my service?
- How do I install it without building from source?
- How do I collect a dump and open the UI?
- What artifacts are published and verified?
- How do I report bugs, security concerns, or contribute?
- What compatibility promises apply to CLI JSON, dump format, SQLite, and algorithms?

The current root README is a thin pointer to internal docs. The install docs focus on source builds. The CI workflow is useful but not a release pipeline. Package metadata and governance files are incomplete. This spec defines the work needed to reach a credible 0.1.0 open source delivery.

## Release Readiness Definition

The repository is P0 release-ready when a maintainer can tag a release and a new user can, from a clean machine:

1. Install `pygco` with the README installer command.
2. Install `pygco-dump` with `python -m pip install "pygco-dump[fastapi]"`.
3. Run a fixture first analysis with `pygco open ... --no-browser`.
4. Integrate the FastAPI producer from the quickstart.
5. Understand runtime safety limitations before exposing any dump endpoint.
6. Find the license, contribution process, issue templates, security reporting path, changelog, and compatibility promises.
7. Verify release downloads through checksums.

## Goals

- Make the project understandable from the root README without requiring prior context.
- Provide a working binary release path for `pygco` that embeds the real Web UI assets.
- Publish `pygco-dump` as a real Python package with complete package metadata.
- Keep routine PR/push CI as fast unit-test feedback, with release-critical docs, Web UI, and packaged artifact checks in dedicated release preparation workflows.
- Add minimum open source governance and security reporting material.
- Preserve document-driven development and TDD expectations.
- Split the work into slices that can become independently grabbable issues.

## Non-Goals

- Do not publish `pygco-cli` or internal Rust crates to crates.io for 0.1.0.
- Do not add Docker images.
- Do not add Homebrew distribution.
- Do not build a hosted documentation site for P0.
- Do not add login, SaaS, remote sharing, RBAC, or multi-user features.
- Do not guarantee long-term SQLite migration compatibility beyond the existing rebuildable analysis model.
- Do not turn `curl | sh` into the only install method; manual install remains required for inspectable and locked-down environments.

## Scope Overview

P0 is the minimum credible open source launch. P1 is post-launch trust, documentation, and operations hardening.

| Priority | Theme | Outcome |
| --- | --- | --- |
| P0 | Identity and metadata | The repository presents a coherent open source project with correct package metadata. |
| P0 | Install and release | Users can install `pygco` from GitHub Releases and `pygco-dump` from PyPI. |
| P0 | CI and release gates | Routine PR/push CI runs unit tests only; dedicated release workflows cover artifact and packaged Web UI checks. |
| P0 | Governance and safety | License, contributing, security, issue/PR templates, and safety docs exist. |
| P0 | Clean-machine acceptance | A documented acceptance script proves the new-user path. |
| P1 | Docs depth | Screenshots, troubleshooting, compatibility details, and examples improve confidence. |
| P1 | Operations | Dependency automation, release notes polish, provenance, and maintainer workflow mature. |

## Task Tracking

Use this spec as the execution checklist. Keep task state in place:

- Leave incomplete items as `- [ ]`.
- Mark an item as `- [x]` only after its acceptance condition is met or its verification command has been run.
- If a task is intentionally skipped, keep it unchecked and add a short note with the decision and owner.
- When an implementation PR closes a slice, update that slice's `Tasks`, `Acceptance`, and `Verification` checkboxes in the same PR.
- If a slice is split into GitHub issues, keep the issue title linked from the relevant task instead of deleting the task from this spec.

## Verification Model

Every checkbox in this spec must be verifiable before it is marked done. Use the following evidence types:

- Local command evidence: deterministic commands such as docs checks, Rust tests, Python builds, Web builds, installer smoke tests, package archive inspection, and YAML parsing.
- Local manual evidence: a maintainer walkthrough in a clean checkout or temporary directory, with commands and observed output recorded in the relevant slice.
- External service evidence: GitHub Actions runs, draft GitHub Release assets, TestPyPI/PyPI package pages, and downloaded artifacts. These remain unchecked until the external run or publish happens.
- Browser evidence: Chrome-rendered Markdown, GitHub Release pages, or local Web UI pages. Record the URL and visible result; screenshots are not required.
- HAT evidence: `docs/release-acceptance.md` results with release tag, artifact URLs, machine OS/arch, Python version, commands, and failures mapped back to a P0 slice.

Do not mark a task complete just because the file exists. Mark it complete only when the acceptance condition is satisfied or the named verification has run.

Browser-based checks may use the Chrome plugin when a task depends on rendered pages, release pages, local Web UI behavior, screenshots, or human-readable documentation layout. Chrome verification is acceptable evidence for:

- README and docs rendered Markdown review.
- GitHub Release asset/download page inspection.
- `pygco open` local Web UI walkthrough.
- Screenshot and visual walkthrough review.
- HAT steps that require opening the printed local URL or inspecting a release page.

When Chrome is used, record the URL, browser-visible result, and the related checkbox or HAT id in the verification notes or HAT report.

## P0 Slices

### P0-S1. Repository Identity and Package Metadata

Priority: P0
Owner: maintainer/release
Depends on: real repository URL decision (resolved: `ivan-94/py-gc-objects-analyze`)

Goal: remove placeholder identity and make package metadata suitable for GitHub, PyPI, and release artifacts.

Tasks:

- [x] Replace placeholder repository metadata in `Cargo.toml` with `https://github.com/ivan-94/py-gc-objects-analyze`.
- [x] Add workspace package metadata needed by Rust tooling even if crates are not published: `description`, `homepage`, `documentation`, `readme`, `keywords`, and `categories` where appropriate.
- [x] Decide whether all internal crates inherit common metadata or keep minimal crate-local package metadata. Decision: internal crates inherit workspace metadata and set `publish = false`.
- [x] Update `python/pygco_dump/pyproject.toml` with license, authors/maintainers, classifiers, project URLs, keywords, and supported Python versions.
- [x] Resolve the Python version mismatch: `docs/install.md` and `pyproject.toml` both use Python `>=3.10`.
- [x] Add or update `CHANGELOG.md` with a release-facing `0.1.0 - Unreleased` section that names artifact types and compatibility versions.

Acceptance:

- [x] `rg "example[.]invalid|T[B]D|TO[D]O" README.md docs/README.md docs/install.md docs/quickstart.md docs/release-acceptance.md Cargo.toml python/pygco_dump/pyproject.toml` has no release-blocking hits.
- [x] `python -m build` from `python/pygco_dump` produces wheel and sdist with correct metadata.
- [x] `cargo metadata --no-deps` succeeds.
- [x] Changelog names tool version, dump format version, SQLite schema version, reachability algorithm version, and findings/report algorithm version.

Verification:

- [x] `cargo metadata --no-deps`
- [x] `cd python/pygco_dump && python -m build`
- [x] `python3 scripts/check_docs_commands.py`

### P0-S2. Root README and User Entry Path

Priority: P0
Owner: docs/product
Depends on: P0-S1 release URL placeholders

Goal: make the root README sufficient for a new user to understand, install, and complete the first analysis.

Tasks:

- [x] Rewrite `README.md` around the external user journey:
  - [x] one-sentence positioning
  - [x] problem statement
  - [x] when to use / when not to use
  - [x] install `pygco`
  - [x] install `pygco-dump`
  - [x] first fixture analysis
  - [x] FastAPI producer snippet
  - [x] safety warning for dump endpoints
  - [x] links to docs
  - [x] project status and compatibility promises
- [x] Add badges only after corresponding workflows exist. Decision: no decorative badges added before a real public CI badge is useful.
- [x] Keep advanced internals in `docs/README.md`; the root README should not become a full reference manual.
- [x] Update `docs/README.md` so it presents user guide, reference, developer, and archive sections instead of one long flat list.
- [x] Update `docs/quickstart.md` so it starts from install and includes a fixture-first path before service integration.

Acceptance:

- [ ] A first-time user can follow README commands in order without opening internal project specs. Requires clean-machine or temporary-checkout walkthrough against real release assets.
- [x] README includes both `curl ... | sh` and manual install references.
- [x] Safety guidance is visible before the dump endpoint example or directly beside it.
- [x] `docs/README.md` separates current user-facing docs from historical/spec/project docs.

Verification:

- [x] `python3 scripts/check_docs_commands.py`
- [ ] Manual README walkthrough on a clean checkout or temporary directory.
- [x] Optional Chrome DOM check: render the GitHub repository page and confirm visible install, quickstart, safety, and docs links.

### P0-S3. Governance, License, and Security Baseline

Priority: P0
Owner: maintainer
Depends on: none

Goal: provide the minimum files external contributors and security reporters expect.

Tasks:

- [x] Add root `LICENSE` matching the MIT license declared in Cargo metadata.
- [x] Add `CONTRIBUTING.md` with:
  - [x] document-driven development rule
  - [x] TDD rule
  - [x] local setup commands
  - [x] docs generation/check commands
  - [x] test matrix by area
  - [x] pull request expectations
- [x] Add `SECURITY.md` with:
  - [x] supported versions
  - [x] private vulnerability reporting path
  - [x] warning that dumps may include sensitive object metadata
  - [x] warning not to expose debug dump endpoints to untrusted users
  - [x] local API binding expectations
- [x] Add `CODE_OF_CONDUCT.md` or explicitly decide not to include one before launch. Decision: include one.
- [x] Add GitHub issue templates:
  - [x] bug report
  - [x] feature request
  - [x] memory investigation result / diagnostic feedback
  - [x] docs issue
- [x] Add PR template with Source Manifest, tests, docs, compatibility, and release note checkboxes.

Acceptance:

- [x] GitHub renders license and contribution entrypoints from repository root.
- [x] Security reporting path is clear without requiring users to open runtime safety docs.
- [x] PR template reminds contributors to update docs/specs before behavior changes.
- [x] Issue templates collect `pygco version`, OS, Python version, command, dump source shape, and whether the dump can be shared.

Verification:

- [x] `find .github -maxdepth 3 -type f`
- [x] Manual review of rendered Markdown on GitHub.
- [x] Optional Chrome DOM check: inspect rendered GitHub repository links and issue template chooser.
- [x] `python3 scripts/check_docs_commands.py`

### P0-S4. Installer Contract and Release Artifact Layout

Priority: P0
Owner: release
Depends on: P0-S1, P0-S2

Goal: define and implement a stable GitHub Release artifact contract for `pygco`.

Release assets:

```text
install.sh
pygco-<version>-<target>.tar.gz
pygco-<version>-<target>.tar.gz.sha256
checksums.txt
```

Initial target policy:

- [x] P0 must support at least Linux x86_64 and macOS Apple Silicon.
- [x] Add macOS x86_64 if the runner/toolchain path is straightforward.
- [x] Windows is not a P0 installer target because the primary installer is POSIX shell. It can be revisited later with a separate PowerShell installer.

Installer contract:

- [x] Default install dir: `$HOME/.local/bin`.
- [x] Override install dir with `PYGCO_INSTALL_DIR`.
- [x] Override version with `PYGCO_VERSION`; unstamped source scripts require explicit `PYGCO_VERSION`, while release-stamped `install.sh` defaults to latest release assets.
- [x] Detect OS and CPU architecture.
- [x] Download the matching release tarball and checksum.
- [x] Verify SHA-256 before installing.
- [x] Install only the `pygco` executable.
- [x] Print the installed path and `pygco version`.
- [x] Fail with actionable errors for unsupported OS/arch, missing `curl`, missing `tar`, checksum mismatch, or unwritable install dir.
- [x] Document manual install for users who do not allow `curl | sh`.

Tasks:

- [x] Add `scripts/install.sh` or `release/install.sh` as the source installer script.
- [x] Add installer tests that run against local fake release assets, not live GitHub.
- [x] Add release packaging script or `just dist` command that produces target archives from a release build.
- [x] Ensure release build embeds real `web/app/dist`, not the fallback page from `crates/pygco-api/build.rs`.
- [x] Update `docs/install.md` with installer, manual install, source build, upgrade, and uninstall sections.

Acceptance:

- [x] `curl -fsSL https://github.com/ivan-94/py-gc-objects-analyze/releases/latest/download/install.sh | sh` is the documented primary binary install path.
- [x] Manual install can verify per-archive `.sha256` files and place `pygco` on `PATH`; `checksums.txt` is generated by the release workflow for full-release verification.
- [x] A release archive extracted locally can run `pygco version`.
- [x] `pygco open fixtures/golden/tiny-v1.jsonl.gz --no-browser` starts and prints a local URL when run from a release binary.

Verification:

- [x] Installer unit/smoke tests against local fixtures.
- [x] `(cd web/app && corepack pnpm install --frozen-lockfile)`
- [x] `(cd web/app && corepack pnpm build)`
- [x] `cargo build --release -p pygco-cli`
- [x] `./target/release/pygco version`
- [x] `./target/release/pygco open fixtures/golden/tiny-v1.jsonl.gz --no-browser`

### P0-S5. GitHub Release Workflow

Priority: P0
Owner: release/CI
Depends on: P0-S4

Goal: automate release artifact creation on tags.

Tasks:

- [x] Add `.github/workflows/release.yml`.
- [x] Trigger on `v*` tags and optionally `workflow_dispatch` for dry runs.
- [x] Build Web UI before Rust release binary.
- [x] Build release binaries per target matrix.
- [x] Package `pygco` archives with README/license snippets if useful.
- [x] Upload `install.sh`, archives, per-archive `.sha256`, and `checksums.txt`.
- [x] Run artifact smoke checks before upload:
  - [x] binary exists
  - [x] `pygco version`
  - [x] embedded Web UI is not the placeholder fallback
  - [x] fixture import smoke before packaging
- [x] Generate release notes from `CHANGELOG.md` or require manual notes in the GitHub release.

Acceptance:

- [x] A tag can produce a draft GitHub Release with all P0 assets.
- [x] Release fails if Web UI assets are missing or the placeholder page is embedded.
- [x] Release fails if checksums are missing.
- [x] Release does not publish crates.io packages.

Verification:

- [x] `workflow_dispatch` dry run on a non-release tag or branch.
- [x] Download uploaded artifacts from the draft release and run checksum/runtime smoke checks.
- [x] Optional Chrome DOM check: inspect the draft GitHub Release page and confirm the draft tag/release page renders. Full asset list was verified through `gh release download` because GitHub kept the asset widget loading in Chrome.

External note: tag `v0.1.0-rc.3` created a successful draft release. Workflow dispatch dry run `28587055092` completed with `tag=dry-run`, built artifacts and release notes, and skipped draft release creation.

### P0-S6. PyPI Release Workflow for `pygco-dump`

Priority: P0
Owner: release/Python
Depends on: P0-S1

Goal: publish `pygco-dump` through a repeatable workflow.

Tasks:

- [x] Add package build command to `justfile` or release docs.
- [x] Add `.github/workflows/publish-python.yml` using PyPI Trusted Publishing.
- [x] Build wheel and sdist from `python/pygco_dump`.
- [x] Validate package metadata before upload.
- [x] Run tests against the built wheel in a clean virtual environment.
- [ ] Publish to TestPyPI first for rehearsal, then PyPI for real releases.
- [x] Document maintainer-side PyPI project setup steps.

Acceptance:

- [ ] A clean environment can run `python -m pip install "pygco-dump[fastapi]"` after publish.
- [x] The FastAPI helper example imports and responds in tests.
- [x] Source distribution contains README, package source, examples, and license metadata.

Verification:

- [x] `cd python/pygco_dump && python -m build`
- [x] `python -m pip install dist/*.whl`
- [x] `python -m pytest python/pygco_dump`
- [ ] TestPyPI rehearsal before first PyPI publish.

External note: TestPyPI/PyPI Trusted Publishing must be configured in the package index before publish can pass. Rehearsal runs `28586504894` and `28589579759` confirmed the repository workflow builds and validates the package; TestPyPI rejected upload with `invalid-publisher` for `repo:ivan-94/py-gc-objects-analyze:environment:testpypi`.

### P0-S7. Lightweight PR CI and Release Gates

Priority: P0
Owner: CI/maintainer
Depends on: P0-S1 through P0-S4

Goal: keep ordinary PR feedback fast and unit-test oriented, while reserving release artifact, Web E2E, and benchmark validation for dedicated release/benchmark workflows.

Tasks:

- [x] Keep PR/push CI lightweight:
  - [x] `rust-unit`: `cargo test --workspace --lib --bins`
  - [x] `python-unit`: `python -m pytest python/pygco_dump` on the default supported Python version
  - [x] `web-unit`: `pnpm --dir web/app test`
- [x] Remove heavyweight PR/push CI checks:
  - [x] docs command freshness check that requires a built `pygco`
  - [x] Rust integration/contract tests
  - [x] Rust clippy and release build
  - [x] generated CLI/OpenAPI freshness
  - [x] Web build and Playwright E2E
  - [x] release archive packaging smoke
  - [x] synthetic benchmark smoke
- [x] Keep release artifact and embedded Web UI checks in `.github/workflows/release.yml`.
- [x] Keep scheduled/manual performance checks in `.github/workflows/benchmarks.yml`.
- [x] Keep maintainer-side full verification commands documented for release preparation.

Acceptance:

- [x] PR CI does not install browser dependencies or run Playwright.
- [x] PR CI only has Rust, Python, and Web unit jobs.
- [x] PR CI does not build release artifacts.
- [x] PR CI does not run benchmark smoke tests.
- [x] Tag release workflow still fails if Web UI assets are missing or placeholder assets are embedded.

Verification:

- [x] First lightweight main CI run after the simplification: `28583986094`.
- [x] Latest lightweight main CI confirmation on commit `a81fcdf`: run `28587516812`.
- [x] Local lightweight equivalent:
  - [x] `cargo test --workspace --lib --bins`
  - [x] `python -m pytest python/pygco_dump`
  - [x] `pnpm --dir web/app test`

### P0-S8. Clean-Machine Acceptance Guide

Priority: P0
Owner: QA/release
Depends on: P0-S2 through P0-S7

Goal: define the final human acceptance loop for 0.1.0.

Tasks:

- [x] Add `docs/release-acceptance.md` or a section in `docs/install.md` that describes the clean-machine release test.
- [x] Cover:
  - [x] install `pygco` through the release installer
  - [x] verify checksum path
  - [x] install `pygco-dump`
  - [x] run fixture open
  - [x] run explicit import/summary/diff
  - [x] run FastAPI example
  - [x] open Web UI manually from printed URL
  - [x] uninstall binary
- [x] Include expected outputs at a high level, not fragile full snapshots.
- [x] Record known non-blocking limitations, such as no Windows installer in P0.

Acceptance:

- [x] A maintainer can execute the guide without reading this spec.
- [x] The guide has a Source Manifest and records exact release tag/artifact URLs during HAT.
- [x] Failures map back to a specific P0 slice.

Verification:

- [x] Manual HAT on at least one Linux machine and one macOS machine before publishing 0.1.0.
- [x] Optional Chrome check: open the printed local Web UI URL during HAT and record the visible Overview page state.
- [x] Read-only external release preflight script exists and records unmet prerequisites.

## P1 Slices

### P1-S1. Documentation Information Architecture Pass

Priority: P1
Owner: docs
Depends on: P0-S2

Goal: make docs easy to navigate after the first release without hiding internal specs.

Tasks:

- [x] Split `docs/README.md` into clear sections:
  - [x] Start here
  - [x] User guide
  - [x] Producer integration
  - [x] CLI/API reference
  - [x] Concepts and architecture
  - [x] Development and testing
  - [x] Specs/archive
- [x] Add `docs/troubleshooting.md`.
- [x] Add `docs/versioning.md`.
- [x] Add `docs/compatibility.md` if versioning grows too large.
- [x] Move historical POC or implementation planning docs under a clearly marked project/archive section.
- [x] Cross-link quickstart, install, runtime safety, known limitations, and producer integration.

Acceptance:

- [x] New users can find install, quickstart, producer integration, safety, and troubleshooting in one click from root README.
- [x] Contributors can find testing, architecture, and generated docs rules in one click from `CONTRIBUTING.md`.

Verification:

- [x] `python3 scripts/check_docs_commands.py`
- [x] Manual link/navigation review.
- [x] Optional Chrome check: open rendered docs or a docs preview and verify the navigation path from README to install, quickstart, safety, troubleshooting, and contributor docs.

### P1-S2. Screenshots, Demo, and Web UI Walkthrough

Priority: P1
Owner: docs/Web
Depends on: P0-S8

Goal: show what the tool does before users install it.

Tasks:

- [x] Add screenshots for Overview, Objects, Object Detail, Graph, Diff, SQL/Idsets, and Report.
- [x] Add a short terminal demo transcript or animated GIF for:
  - [x] install
  - [x] fixture open
  - [x] first Web UI URL
- [x] Update `docs/web-ui-walkthrough.md` with images and exact fixture commands.
- [x] Add screenshot refresh instructions so images do not become mysterious binary artifacts.

Acceptance:

- [x] Root README includes at least one first-viewport visual signal.
- [x] Web UI walkthrough uses stable fixtures and can be regenerated.
- [x] Screenshots do not contain private data.

Verification:

- [x] Playwright screenshot generation command or documented manual screenshot procedure.
- [x] Manual visual review.
- [x] Optional Chrome check: inspect generated screenshots or the running Web UI and capture evidence for Overview, Objects, Graph, Diff, SQL/Idsets, and Report.

### P1-S3. Troubleshooting and Safety Deepening

Priority: P1
Owner: docs/security
Depends on: P0-S3

Goal: reduce support burden and make safe usage habits explicit.

Tasks:

- [x] Add troubleshooting for:
  - [x] installer unsupported target
  - [x] missing `curl`/`tar`
  - [x] `pygco` not on `PATH`
  - [x] Web UI port conflicts
  - [x] endpoint returns huge or slow dump
  - [x] `collect=true` latency risk
  - [x] reachability `truncated` or `unavailable`
  - [x] malformed dump line errors
  - [x] read-only SQL rejection
  - [x] large SQLite cleanup
- [x] Expand `docs/runtime-safety.md` with deployment patterns for internal-only endpoints.
- [x] Add a security checklist to `docs/producer-integration.md`.

Acceptance:

- [x] Common error messages in CLI/API docs link or point to actionable next steps.
- [x] Users can understand when not to expose dump routes and when to avoid `collect=true`.

Verification:

- [x] `python3 scripts/check_docs_commands.py`
- [x] Spot-check CLI errors against troubleshooting entries.

### P1-S4. Dependency and Maintenance Automation

Priority: P1
Owner: maintainer/CI
Depends on: P0-S7

Goal: keep dependencies current without surprising release behavior.

Tasks:

- [x] Add Dependabot or Renovate configuration for:
  - [x] GitHub Actions
  - [x] Cargo
  - [x] pnpm
  - [x] Python package metadata
- [x] Group low-risk patch updates.
- [x] Keep major updates separate.
- [x] Document expected maintainer response and required CI gates.

Acceptance:

- [x] Dependency PRs are automatically opened with clear grouping.
- [x] Generated lockfile changes are visible and reviewed.
- [x] Release workflows are included in dependency scanning.

Verification:

- [x] First dependency automation dry run or initial PR.

External note: Dependabot opened initial PRs after merge/push to GitHub. File-list review confirmed generated lockfile changes are visible in npm and Cargo PRs; individual PR merge decisions remain separate maintainer review actions.

### P1-S5. Release Provenance and Artifact Hardening

Priority: P1
Owner: release/security
Depends on: P0-S5

Goal: strengthen trust beyond basic checksums.

Tasks:

- [x] Evaluate artifact signing for release archives and checksums.
- [x] Evaluate GitHub artifact attestations or SLSA provenance.
- [x] Add SBOM generation if dependency consumers request it. Decision: defer until a consumer asks for it.
- [x] Add release checklist items for verifying downloaded assets from a separate machine.
- [x] Consider pinning build tool versions through `rust-toolchain.toml` and clearer Node/Python setup docs. Decision: workflow Node/pnpm/Python versions are explicit; `rust-toolchain.toml` remains optional.

Acceptance:

- [x] Release notes tell users how to verify artifacts.
- [x] Maintainers can explain what the checksum/signature/provenance does and does not prove.
- [x] Hardening does not block routine patch releases unnecessarily.

Verification:

- [x] Release dry run with signed or attested artifacts.

External note: workflow dispatch run `28587529503` completed with `tag=dry-run-attest`; `Attest release artifacts` created provenance for 8 subjects, and `gh attestation verify` passed for the Linux archive. Existing `v0.1.0-rc.3` draft release assets predate this attestation change and remain checksum-verified only.

### P1-S6. Contributor Experience and Issue Triage

Priority: P1
Owner: maintainer/community
Depends on: P0-S3

Goal: make incoming issues and PRs easier to route.

Tasks:

- [x] Add labels for area, priority, type, and status.
- [x] Add `good first issue` candidates from docs/test gaps.
- [x] Add maintainer triage notes for:
  - [x] bug reports with private dumps
  - [x] performance reports
  - [x] Web UI visual regressions
  - [x] compatibility questions
- [x] Add a lightweight release checklist issue template.

Acceptance:

- [x] A new bug report includes enough command/version/environment context to reproduce or ask one focused follow-up.
- [x] A docs contributor can run the minimum required checks without learning the full Rust/Web stack.

Verification:

- [x] Chrome DOM check of GitHub issue template chooser.

External note: `gh issue create` in the installed GitHub CLI does not expose a local dry-run flag; templates were verified through the GitHub issue creation UI after push.

### P1-S7. Performance and Benchmark Publication

Priority: P1
Owner: performance/docs
Depends on: P0-S7

Goal: make performance claims reproducible and not overfit to local runs.

Tasks:

- [x] Add scheduled or manual benchmark workflow for synthetic medium/large fixtures.
- [x] Store benchmark summaries as release artifacts or docs snapshots.
- [x] Update `docs/performance.md` with benchmark environment metadata.
- [x] Add bundle size reporting for Web UI.
- [x] Document how to run large benchmarks locally without committing generated reports.

Acceptance:

- [x] Performance docs name hardware/runner, fixture, command, and version.
- [x] Benchmark reports are not accidentally committed if `.gitignore` says they are generated artifacts.

Verification:

- [x] Manual benchmark workflow run `28581056797`.
- [x] Local benchmark command smoke against `fixtures/synthetic/medium.jsonl.gz`.
- [x] `git ls-files` has no generated benchmark reports unless intentionally tracked with a documented exception.

## Recommended Issue Breakdown

Use these as independently grabbable issue titles:

Mark these checkboxes only when the corresponding tracker issue has been created and linked from this spec. They are issue-creation tracking items, not duplicate implementation acceptance criteria.

- [x] [P0-S1: Fix repository identity and package metadata for 0.1.0](https://github.com/ivan-94/py-gc-objects-analyze/issues/13).
- [x] [P0-S2: Rewrite root README and quickstart for first-time users](https://github.com/ivan-94/py-gc-objects-analyze/issues/14).
- [x] [P0-S3: Add open source governance, security, and GitHub templates](https://github.com/ivan-94/py-gc-objects-analyze/issues/16).
- [x] [P0-S4: Implement release installer and artifact layout](https://github.com/ivan-94/py-gc-objects-analyze/issues/17).
- [x] [P0-S5: Add GitHub Release workflow for `pygco` binary artifacts](https://github.com/ivan-94/py-gc-objects-analyze/issues/18).
- [x] [P0-S6: Add PyPI release workflow for `pygco-dump`](https://github.com/ivan-94/py-gc-objects-analyze/issues/19).
- [x] [P0-S7: Expand CI into release gate](https://github.com/ivan-94/py-gc-objects-analyze/issues/20).
- [x] [P0-S8: Add clean-machine release acceptance guide](https://github.com/ivan-94/py-gc-objects-analyze/issues/22).
- [x] [P1-S1: Reorganize docs information architecture](https://github.com/ivan-94/py-gc-objects-analyze/issues/23).
- [x] [P1-S2: Add screenshots and Web UI walkthrough visuals](https://github.com/ivan-94/py-gc-objects-analyze/issues/24).
- [x] [P1-S3: Add troubleshooting and safety deepening docs](https://github.com/ivan-94/py-gc-objects-analyze/issues/25).
- [x] [P1-S4: Add dependency maintenance automation](https://github.com/ivan-94/py-gc-objects-analyze/issues/26).
- [x] [P1-S5: Add release provenance and artifact hardening](https://github.com/ivan-94/py-gc-objects-analyze/issues/28).
- [x] [P1-S6: Improve contributor triage workflow](https://github.com/ivan-94/py-gc-objects-analyze/issues/29).
- [x] [P1-S7: Publish reproducible benchmark process](https://github.com/ivan-94/py-gc-objects-analyze/issues/30).

## Recommended Implementation Order

1. P0-S1, because metadata and real URLs affect README, installer, PyPI, and workflows.
2. P0-S3, because license/security/contributing are release blockers and mostly independent.
3. P0-S2, because README defines the user contract that release automation must satisfy.
4. P0-S4, because artifact naming and installer behavior must settle before release workflow.
5. P0-S7, because CI should protect the implementation work before release automation is trusted.
6. P0-S5 and P0-S6, because binary and Python publishing are separate release lanes.
7. P0-S8, because acceptance should validate the completed release path.
8. P1 slices after the first 0.1.0 release candidate.

## HAT Criteria for 0.1.0

Before publishing 0.1.0 as a non-draft release, run human acceptance against release artifacts:

- [x] HAT-1: Install `pygco` through the release installer on Linux.
- [x] HAT-2: Install `pygco` through the release installer on macOS.
- [x] HAT-3: Manually download the release archive and verify checksum.
- [x] HAT-4: Run `pygco version`.
- [x] HAT-5: Run `pygco open fixtures/golden/tiny-v1.jsonl.gz --no-browser`.
- [x] HAT-6: Run explicit import, summary, objects, diff, and report commands against golden fixtures.
- [ ] HAT-7: Install `pygco-dump[fastapi]` from PyPI or TestPyPI in a clean virtual environment.
- [x] HAT-8: Run the FastAPI example and pull a dump.
- [x] HAT-9: Confirm README, install docs, quickstart, security docs, and known limitations match the observed behavior.
- [x] HAT-10: Confirm release notes list artifact names, checksums, compatibility versions, and known limitations.

Each HAT report must include a Source Manifest with release tag, artifact URLs, machine OS/arch, Python version, and commands run.

## Done Criteria

P0 is done when:

- [x] Root README is release-facing and accurate.
- [x] License, contributing, security, templates, package metadata, and changelog are present.
- [x] GitHub Release workflow can produce installer, archives, checksums, and draft release notes.
- [ ] PyPI workflow can publish `pygco-dump` through a rehearsed path. Requires TestPyPI/PyPI Trusted Publishing setup and rehearsal.
- [x] PR/push CI gates unit checks only; release preparation and benchmark workflows gate heavyweight artifact, docs freshness, Web E2E, and performance checks.
- [x] Clean-machine acceptance passes on the supported P0 binary targets.

P1 is done when:

- [x] Docs navigation, screenshots, troubleshooting, and safety material are strong enough for self-service users.
- [x] Dependency automation and triage workflows are active.
- [x] Release artifacts have stronger verification or provenance than basic checksums.
- [x] Performance claims are reproducible from documented benchmark commands and environment metadata.
