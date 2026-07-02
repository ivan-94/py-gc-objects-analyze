# Release Acceptance HAT: v0.1.0-rc.3

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
- Linux release acceptance workflow run: https://github.com/ivan-94/py-gc-objects-analyze/actions/runs/28586150834
- GitHub Release tag: `v0.1.0-rc.3`
- GitHub Release URL: https://github.com/ivan-94/py-gc-objects-analyze/releases/tag/untagged-d5226fc5b2c3a3d763e8

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
- PyPI/TestPyPI publishing was not marked complete because `pygco-dump` is not visible on PyPI or TestPyPI and Trusted Publishing/project ownership setup is external to the repository.

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
- Chrome DOM verification was performed without screenshots against the local Web UI; the Overview navigation and page content rendered from the printed local URL.
- `python3 scripts/check_docs_commands.py`, workflow YAML parsing, and `git diff --check` passed after the workflow/report updates.

### Open questions / risks

- HAT-7 is still open: publish or rehearse `pygco-dump[fastapi]` from TestPyPI or PyPI after maintainer-owned Trusted Publishing is configured.
- The final public `curl -fsSL https://github.com/ivan-94/py-gc-objects-analyze/releases/latest/download/install.sh | sh` path must be checked after publishing a non-draft release.
- The macOS x86_64 archive was runtime-smoked under Rosetta, not on a physical Intel Mac.
- Release artifact signing or GitHub artifact attestation remains P1 hardening beyond the current checksum verification.
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
- [ ] HAT-7: Install `pygco-dump[fastapi]` from PyPI or TestPyPI in a clean virtual environment.
- [x] HAT-8: Ran the FastAPI helper from a clean local-wheel venv and pulled a gzip dump.
- [x] HAT-9: Confirmed README, install docs, quickstart, security docs, known limitations, and compatibility notes match observed behavior, except for the intentionally unverified PyPI/TestPyPI publish path.
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
```

## Result

The GitHub Release binary artifact path is accepted for `v0.1.0-rc.3` on Linux x86_64 and macOS Apple Silicon, with an additional macOS x86_64 Rosetta runtime smoke. The remaining P0 release blocker is PyPI/TestPyPI Trusted Publishing and package publication rehearsal for `pygco-dump`.
