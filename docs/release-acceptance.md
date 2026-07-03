# Release Acceptance

## Purpose

Run this guide before publishing a non-draft release. Record the release tag, artifact URLs, OS/architecture, Python version, commands, outputs, and any manual browser checks.

## External Preflight

Before starting acceptance, check external release prerequisites:

```bash
scripts/release_preflight.sh
```

The preflight is read-only. It checks local tools, `origin`, GitHub CLI authentication, visible GitHub releases and workflow runs, PyPI package visibility, and required local release files. Any warning should be resolved or recorded before publishing a non-draft release.

## Optional Linux Runner Acceptance

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

## PyPI Trusted Publishing Setup

Before release acceptance can check the TestPyPI/PyPI install path, configure a trusted publisher on the package index. Follow the official [PyPI Trusted Publishers guide](https://docs.pypi.org/trusted-publishers/) and use these claims from `.github/workflows/publish-python.yml`:

| Index | Project | Owner | Repository | Workflow | Environment |
| --- | --- | --- | --- | --- | --- |
| TestPyPI | `pygco-dump` | `ivan-94` | `py-gc-objects-analyze` | `publish-python.yml` | `testpypi` |
| PyPI | `pygco-dump` | `ivan-94` | `py-gc-objects-analyze` | `publish-python.yml` | `pypi` |

`publish-python.yml` is intentionally `workflow_dispatch` only. Publish `pygco-dump` to PyPI explicitly before making the GitHub Release public; publishing a GitHub Release should not retry a package version that already exists on PyPI.

After publishing, verify `python -m pip install "pygco-dump[fastapi]"` in a clean virtual environment.

For TestPyPI install rehearsals, install runtime dependencies from PyPI first, then install the TestPyPI wheel with `--no-deps`:

```bash
python -m pip install 'fastapi>=0.115.0'
python -m pip install --index-url https://test.pypi.org/simple/ --no-deps 'pygco-dump[fastapi]'
```

Avoid using TestPyPI together with `--extra-index-url` for acceptance evidence; unrelated packages on TestPyPI can win dependency resolution before pip falls back to PyPI.

## Acceptance Checklist

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
## Release Evidence

- Release tag:
- GitHub Release URL:
- PyPI/TestPyPI URL:
- Machine:
- Python:
- Browser evidence:

### Produced artifacts

- Acceptance log:
- Screenshots:

### Key decisions

-

### Verification evidence

-

### Open questions / risks

-
```
