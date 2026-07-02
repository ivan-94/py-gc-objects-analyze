# Release Acceptance

## Source Manifest

- Release readiness spec: `docs/specs/2026-07-02-open-source-release-readiness-spec.md`
- Install docs: `docs/install.md`
- Quickstart: `docs/quickstart.md`
- Runtime safety: `docs/runtime-safety.md`
- Known limitations: `docs/known-limitations.md`
- Release scripts: `scripts/install.sh`, `scripts/package_release.sh`
- GitHub release workflow: `.github/workflows/release.yml`
- PyPI publish workflow: `.github/workflows/publish-python.yml`

## Purpose

Run this guide before publishing a non-draft 0.1.0 release. Record the release tag, artifact URLs, OS/architecture, Python version, commands, outputs, and screenshots or Chrome evidence when a browser is used.

## External Preflight

Before starting HAT, check external release prerequisites:

```bash
scripts/release_preflight.sh
```

The preflight is read-only. It checks local tools, `origin`, GitHub CLI authentication, visible GitHub releases and workflow runs, PyPI package visibility, and required local release files. Any warning should be resolved or recorded in the HAT evidence before publishing a non-draft release.

## Optional Linux Runner HAT

When a local Linux machine is not available, run the manual `release-acceptance` workflow against a draft or published release tag:

```bash
gh workflow run release-acceptance.yml -f tag=v0.1.0
```

The workflow downloads GitHub Release assets, verifies `checksums.txt`, installs through the release `install.sh`, and runs the fixture import, summary, objects, diff, report, and `open --no-browser` smoke commands on an Ubuntu runner. It is intentionally `workflow_dispatch` only and does not run on push or pull request CI.

## Optional Release Workflow Dry Run

Use the release workflow dry run when you want to exercise release builds and packaging without creating or modifying a GitHub Release:

```bash
gh workflow run release.yml -f tag=dry-run -f dry_run=true
```

The dry run still builds the Web UI, release binaries, archives, checksums, installer, and release notes. It skips only the final `gh release create` step.

The release workflow also creates GitHub artifact attestations for release assets. To rehearse that path before a real tag, use a distinct dry-run tag value and verify at least one downloaded workflow artifact:

```bash
gh workflow run release.yml -f tag=dry-run-attest -f dry_run=true
gh run watch <run-id> --exit-status
gh attestation verify "pygco-dry-run-attest-x86_64-unknown-linux-gnu.tar.gz" --repo ivan-94/py-gc-objects-analyze
```

The 2026-07-02 attestation dry run `28587529503` passed on commit `a81fcdf` and verified the Linux archive against `release.yml@refs/heads/main`.

## PyPI Trusted Publishing Setup

Before HAT can check the TestPyPI/PyPI install path, configure a trusted publisher on the package index. Follow the official [PyPI Trusted Publishers guide](https://docs.pypi.org/trusted-publishers/) and use these claims from `.github/workflows/publish-python.yml`:

| Index | Project | Owner | Repository | Workflow | Environment |
| --- | --- | --- | --- | --- | --- |
| TestPyPI | `pygco-dump` | `ivan-94` | `py-gc-objects-analyze` | `publish-python.yml` | `testpypi` |
| PyPI | `pygco-dump` | `ivan-94` | `py-gc-objects-analyze` | `publish-python.yml` | `pypi` |

The 2026-07-02 TestPyPI rehearsal run `28586504894` built the wheel and sdist successfully, passed `twine check`, and tested the built wheel. The upload failed with `invalid-publisher` for `repo:ivan-94/py-gc-objects-analyze:environment:testpypi`, which means the matching TestPyPI trusted publisher was not configured yet.

Retry run `28589579759` produced the same result: build, `twine check`, built-wheel smoke, and artifact upload passed; TestPyPI still rejected the trusted publishing exchange with `invalid-publisher` for repository `ivan-94/py-gc-objects-analyze`, workflow `publish-python.yml@refs/heads/main`, and environment `testpypi`.

After configuring the TestPyPI pending trusted publisher, run `28592147240` published `pygco-dump 0.1.0` to TestPyPI successfully. The production PyPI publisher is still a separate maintainer setup step before the non-draft release.

`publish-python.yml` is intentionally `workflow_dispatch` only. Publish `pygco-dump` to PyPI explicitly before making the GitHub Release public; publishing a GitHub Release should not retry a package version that already exists on PyPI.

Production PyPI run `28596078256` published `pygco-dump 0.1.0` successfully. `python -m pip install "pygco-dump[fastapi]"` passed in a clean virtual environment after the publish.

For TestPyPI install rehearsals, install runtime dependencies from PyPI first, then install the TestPyPI wheel with `--no-deps`:

```bash
python -m pip install 'fastapi>=0.115.0'
python -m pip install --index-url https://test.pypi.org/simple/ --no-deps 'pygco-dump[fastapi]'
```

Avoid using TestPyPI together with `--extra-index-url` for acceptance evidence; unrelated packages on TestPyPI can win dependency resolution before pip falls back to PyPI.

## HAT Checklist

- [ ] Install `pygco` through the release installer on Linux.
- [ ] Install `pygco` through the release installer on macOS.
- [ ] Manually download a release archive and verify its checksum.
- [ ] Run `pygco version`.
- [ ] Run `pygco open fixtures/golden/tiny-v1.jsonl.gz --no-browser`.
- [ ] Open the printed local Web UI URL and confirm the Overview page renders.
- [ ] Run explicit import:

  ```bash
  pygco import fixtures/golden/diff-before-v1.jsonl.gz fixtures/golden/diff-after-v1.jsonl.gz -o analysis.sqlite --rebuild
  ```

- [ ] Run CLI smoke commands:

  ```bash
  pygco summary analysis.sqlite
  pygco objects analysis.sqlite --limit 5 --format table
  pygco diff analysis.sqlite --from 1 --to 2
  pygco report analysis.sqlite --format markdown
  ```

- [ ] Install `pygco-dump[fastapi]` from TestPyPI or PyPI in a clean virtual environment.
- [ ] Run the FastAPI example and pull a dump.
- [ ] Confirm README, install docs, quickstart, security docs, known limitations, and release notes match observed behavior.
- [ ] Uninstall `pygco` from the test install directory.

## Evidence Template

```markdown
## Source Manifest

### Sources

- Release tag:
- GitHub Release URL:
- PyPI/TestPyPI URL:
- Machine:
- Python:
- Browser/Chrome evidence:

### Produced artifacts

- HAT log:
- Screenshots:

### Key decisions

-

### Verification evidence

-

### Open questions / risks

-
```
