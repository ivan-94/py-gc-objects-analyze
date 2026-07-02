# Release Acceptance HAT: v0.1.0-rc.3

Superseded by the final `v0.1.0` HAT at `docs/hats/2026-07-02-release-acceptance-v0.1.0.md`. This report remains as release-candidate evidence and preserves the state observed before the final PyPI publish and public GitHub Release.

## Source Manifest

### Sources

- Release readiness spec: `docs/specs/2026-07-02-open-source-release-readiness-spec.md`
- Release acceptance guide: `docs/release-acceptance.md`
- Install docs: `docs/install.md`
- Quickstart: `docs/quickstart.md`
- Runtime safety: `docs/runtime-safety.md`
- Known limitations: `docs/known-limitations.md`
- Compatibility: `docs/compatibility.md`
- Release notes template: `CHANGELOG.md`
- Release workflows: `.github/workflows/release.yml`, `.github/workflows/release-acceptance.yml`
- Release scripts: `scripts/install.sh`, `scripts/package_release.sh`, `scripts/render_release_notes.py`
- GitHub Release workflow run: https://github.com/ivan-94/py-gc-objects-analyze/actions/runs/28584497200
- Attestation release dry-run workflow run: https://github.com/ivan-94/py-gc-objects-analyze/actions/runs/28587529503
- Linux release acceptance workflow run: https://github.com/ivan-94/py-gc-objects-analyze/actions/runs/28586150834
- TestPyPI publish rehearsal workflow run: https://github.com/ivan-94/py-gc-objects-analyze/actions/runs/28586504894
- TestPyPI publish rehearsal retry workflow run: https://github.com/ivan-94/py-gc-objects-analyze/actions/runs/28589579759
- TestPyPI publish rehearsal success workflow run: https://github.com/ivan-94/py-gc-objects-analyze/actions/runs/28592147240
- Lightweight unit CI confirmation run: https://github.com/ivan-94/py-gc-objects-analyze/actions/runs/28587516812
- GitHub Release tag: `v0.1.0-rc.3`
- GitHub Release URL: https://github.com/ivan-94/py-gc-objects-analyze/releases/tag/untagged-d5226fc5b2c3a3d763e8
- GitHub repository page: https://github.com/ivan-94/py-gc-objects-analyze
- Local Web UI URL inspected through Chrome: `http://127.0.0.1:3776/`

### Produced artifacts

- HAT report: `docs/hats/2026-07-02-release-acceptance-v0.1.0-rc.3.md`
- Downloaded release assets: `.scratch/rc3/`
- macOS Apple Silicon HAT output: `.scratch/hat-rc3-macos/work/`
- macOS x86_64 runtime smoke output: `.scratch/hat-rc3-macos-x86_64/`
- Linux HAT artifact output: `.scratch/hat-linux-rc3/`
- Rendered rc3 release notes: `.scratch/rc3-release-notes.md`

### Key decisions

- `v0.1.0-rc.3` remains a draft release candidate. It is valid for release artifact HAT, but the public `releases/latest/download/install.sh` path still needs one final check after publishing a non-draft release.
- The Linux HAT uses a manual `workflow_dispatch` Ubuntu runner instead of push/PR CI. This keeps routine CI to simple unit checks.
- Draft release assets were installed with `PYGCO_DOWNLOAD_BASE_URL=file://...` after authenticated download from GitHub Releases. This validates the release `install.sh` and assets without requiring public non-draft visibility.
- TestPyPI publishing is accepted for rehearsal after configuring the pending trusted publisher. Production PyPI publishing remains a separate maintainer approval step and is not performed by this HAT.

### Verification evidence

- Release workflow `28584497200` completed successfully for `v0.1.0-rc.3`.
- Release assets on `v0.1.0-rc.3`:
  - `install.sh`
  - `checksums.txt`
  - `pygco-0.1.0-rc.3-x86_64-unknown-linux-gnu.tar.gz`
  - `pygco-0.1.0-rc.3-x86_64-unknown-linux-gnu.tar.gz.sha256`
  - `pygco-0.1.0-rc.3-x86_64-apple-darwin.tar.gz`
  - `pygco-0.1.0-rc.3-x86_64-apple-darwin.tar.gz.sha256`
  - `pygco-0.1.0-rc.3-aarch64-apple-darwin.tar.gz`
  - `pygco-0.1.0-rc.3-aarch64-apple-darwin.tar.gz.sha256`
- `cd .scratch/rc3 && shasum -a 256 -c checksums.txt` passed for all three archives.
- Downloaded `install.sh` is stamped with `DEFAULT_VERSION="0.1.0-rc.3"` and still keeps the unstamped sentinel check.
- GitHub draft release notes were updated with version-stamped asset names and checksum commands for `0.1.0-rc.3`.
- Linux HAT run `28586150834` passed on Ubuntu 24.04.4 x86_64 with Python 3.12.3.
- macOS HAT ran on macOS 27.0 arm64 with Python 3.14.0.
- macOS x86_64 archive ran `pygco version`, `import`, and `summary` under Rosetta on Apple Silicon.
- FastAPI helper was installed from the local wheel in a clean venv and produced an `application/gzip` dump stream with `metadata/start` and `metadata/end` records.
- TestPyPI publish rehearsal run `28586504894` built the wheel and sdist, passed `twine check`, and tested the built wheel; upload failed with `invalid-publisher` for `repo:ivan-94/py-gc-objects-analyze:environment:testpypi`.
- TestPyPI publish rehearsal retry `28589579759` again passed package build, `twine check`, built-wheel smoke, and artifact upload; the TestPyPI upload still failed with `invalid-publisher` for matching repository/workflow/environment claims.
- TestPyPI pending trusted publisher was configured through Chrome without screenshots for project `pygco-dump`, owner `ivan-94`, repository `py-gc-objects-analyze`, workflow `publish-python.yml`, environment `testpypi`.
- TestPyPI publish rehearsal `28592147240` completed successfully on commit `fc336363a3b5c7cd498e8c946f08f877464781e8`; build, `twine check`, built-wheel smoke, and `publish-testpypi` passed, while the production PyPI job was skipped.
- `python3 -m pip index versions --index-url https://test.pypi.org/simple/ pygco-dump` found `pygco-dump (0.1.0)`.
- Clean TestPyPI install/import smoke passed in `.scratch/testpypi-venv` after installing `fastapi>=0.115.0` from PyPI and installing `pygco-dump[fastapi]` from TestPyPI with `--no-deps`.
- Direct TestPyPI install with `--extra-index-url https://pypi.org/simple` was not used as HAT evidence because dependency resolution selected an unrelated invalid TestPyPI `FASTAPI-1.0` package before falling back to PyPI.
- Lightweight CI run `28587516812` passed on commit `a81fcdf` with only `rust-unit`, `python-unit`, and `web-unit`.
- Release dry-run `28587529503` passed with `tag=dry-run-attest`; `Attest release artifacts` created provenance for 8 subjects, and `gh attestation verify` passed for `.scratch/dry-run-attest/release-linux/pygco-dry-run-attest-x86_64-unknown-linux-gnu.tar.gz`.
- Chrome DOM verification was performed without screenshots:
  - GitHub repository page rendered README install/quickstart content, `releases/latest/download/install.sh`, `pygco-dump[fastapi]`, and license/contributing/security links.
  - Rendered install, quickstart, runtime safety, troubleshooting, and contributing docs were reachable and showed expected headings/content.
  - Local Web UI rendered Overview, Objects, Object Graph, Diff, SQL, and Report pages from the printed local URL.
- `python3 scripts/check_docs_commands.py`, workflow YAML parsing, and `git diff --check` passed after the workflow/report updates.

### Open questions / risks

- Production PyPI Trusted Publishing for `pygco-dump` still needs to be configured before the non-draft 0.1.0 release.
- The final public `curl -fsSL https://github.com/ivan-94/py-gc-objects-analyze/releases/latest/download/install.sh | sh` path must be checked after publishing a non-draft release.
- The macOS x86_64 archive was runtime-smoked under Rosetta, not on a physical Intel Mac.
- Release artifact signing or GitHub artifact attestation remains P1 hardening for the already-created `v0.1.0-rc.3` draft assets; later release workflow dry run `28587529503` validated attestations for future tags.
- GitHub runner annotations note that `actions/checkout@v4` and `actions/upload-artifact@v4` are being forced to Node.js 24 because Node.js 20 is deprecated; Dependabot has opened action update PRs.

## Environment

| Target | Evidence |
| --- | --- |
| Linux x86_64 | Ubuntu 24.04.4 LTS, kernel `6.17.0-1018-azure`, Python 3.12.3, run `28586150834` |
| macOS Apple Silicon | Darwin `BobiBobi.local` arm64, macOS 27.0 build `26A5368g`, Python 3.14.0 |
| macOS x86_64 | `pygco-0.1.0-rc.3-x86_64-apple-darwin.tar.gz` executed via `arch -x86_64` under Rosetta |

## HAT Checklist

- [x] HAT-1: Installed `pygco` through the release installer on Linux.
- [x] HAT-2: Installed `pygco` through the release installer on macOS Apple Silicon.
- [x] HAT-3: Manually downloaded release archives and verified checksums.
- [x] HAT-4: Ran `pygco version`.
- [x] HAT-5: Ran `pygco open fixtures/golden/tiny-v1.jsonl.gz --no-browser`.
- [x] HAT-6: Ran explicit import, summary, objects, diff, and report commands against golden fixtures.
- [x] HAT-7: Install `pygco-dump[fastapi]` from PyPI or TestPyPI in a clean virtual environment.
- [x] HAT-8: Ran the FastAPI helper from a clean local-wheel venv and pulled a gzip dump.
- [x] HAT-9: Confirmed README, install docs, quickstart, security docs, known limitations, and compatibility notes match observed behavior, with TestPyPI publish rehearsal accepted and production PyPI still pending.
- [x] HAT-10: Confirmed release notes list versioned artifact names, checksum commands, compatibility versions, and known limitations.

## Command Evidence

```bash
gh release view v0.1.0-rc.3 --repo ivan-94/py-gc-objects-analyze --json tagName,isDraft,url,assets
(cd .scratch/rc3 && shasum -a 256 -c checksums.txt)
PYGCO_DOWNLOAD_BASE_URL="file://$PWD/.scratch/rc3" PYGCO_INSTALL_DIR="$PWD/.scratch/hat-rc3-macos/bin" .scratch/rc3/install.sh
.scratch/hat-rc3-macos/bin/pygco version
.scratch/hat-rc3-macos/bin/pygco import fixtures/golden/diff-before-v1.jsonl.gz fixtures/golden/diff-after-v1.jsonl.gz -o .scratch/hat-rc3-macos/work/analysis.sqlite --rebuild
.scratch/hat-rc3-macos/bin/pygco summary .scratch/hat-rc3-macos/work/analysis.sqlite
.scratch/hat-rc3-macos/bin/pygco objects .scratch/hat-rc3-macos/work/analysis.sqlite --limit 5 --format table
.scratch/hat-rc3-macos/bin/pygco diff .scratch/hat-rc3-macos/work/analysis.sqlite --from 1 --to 2
.scratch/hat-rc3-macos/bin/pygco report .scratch/hat-rc3-macos/work/analysis.sqlite --format markdown
timeout 5s .scratch/hat-rc3-macos/bin/pygco open fixtures/golden/tiny-v1.jsonl.gz --no-browser --port 0 --cleanup-on-exit
arch -x86_64 .scratch/hat-rc3-macos-x86_64/pygco version
arch -x86_64 .scratch/hat-rc3-macos-x86_64/pygco import fixtures/golden/tiny-v1.jsonl.gz -o .scratch/hat-rc3-macos-x86_64/tiny.sqlite --rebuild
.scratch/hat-rc3-macos/venv/bin/python -c 'from pygco_dump.fastapi import gc_heap_dump_route; assert callable(gc_heap_dump_route)'
gh workflow run release-acceptance.yml -f tag=v0.1.0-rc.3 --ref main
gh run watch 28586150834 --exit-status
gh run download 28586150834 --name release-acceptance-linux --dir .scratch/hat-linux-rc3
gh workflow run release.yml -f tag=dry-run-attest -f dry_run=true --ref main
gh run watch 28587529503 --exit-status
gh attestation verify .scratch/dry-run-attest/release-linux/pygco-dry-run-attest-x86_64-unknown-linux-gnu.tar.gz --repo ivan-94/py-gc-objects-analyze --format json
gh workflow run publish-python.yml -f target=testpypi --ref main
gh run watch 28592147240 --exit-status
python3 -m pip index versions --index-url https://test.pypi.org/simple/ pygco-dump
rm -rf .scratch/testpypi-venv
python3 -m venv .scratch/testpypi-venv
. .scratch/testpypi-venv/bin/activate
python -m pip install --upgrade pip
python -m pip install 'fastapi>=0.115.0'
python -m pip install --index-url https://test.pypi.org/simple/ --no-deps 'pygco-dump[fastapi]'
python -c 'from pygco_dump import write_gc_dump; from pygco_dump.fastapi import gc_heap_dump_route; assert callable(write_gc_dump); assert callable(gc_heap_dump_route)'
```

## Result

The GitHub Release binary artifact path is accepted for `v0.1.0-rc.3` on Linux x86_64 and macOS Apple Silicon, with an additional macOS x86_64 Rosetta runtime smoke. The `pygco-dump` TestPyPI publication rehearsal and clean install/import path are accepted. Remaining pre-release work is production PyPI Trusted Publishing/publish approval and the final public `releases/latest` installer check after a non-draft release is published.
