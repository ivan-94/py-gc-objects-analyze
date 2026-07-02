# Release Acceptance HAT: v0.1.0

## Source Manifest

### Sources

- Release readiness spec: `docs/specs/2026-07-02-open-source-release-readiness-spec.md`
- Release acceptance guide: `docs/release-acceptance.md`
- Publish workflow: `.github/workflows/publish-python.yml`
- Release workflow: `.github/workflows/release.yml`
- Installer: `scripts/install.sh`
- TestPyPI HAT baseline: `docs/hats/2026-07-02-release-acceptance-v0.1.0-rc.3.md`
- Production PyPI publish run: https://github.com/ivan-94/py-gc-objects-analyze/actions/runs/28596078256
- Final tag unit CI run: https://github.com/ivan-94/py-gc-objects-analyze/actions/runs/28596330932
- Final release workflow run: https://github.com/ivan-94/py-gc-objects-analyze/actions/runs/28596419592
- GitHub Release: https://github.com/ivan-94/py-gc-objects-analyze/releases/tag/v0.1.0
- PyPI project: https://pypi.org/project/pygco-dump/0.1.0/

### Produced artifacts

- Final HAT report: `docs/hats/2026-07-02-release-acceptance-v0.1.0.md`
- Latest installer test output: `.scratch/latest-pipe-install/`
- Production PyPI install venv: `.scratch/pypi-venv/`

### Key decisions

- `publish-python.yml` is `workflow_dispatch` only. `pygco-dump` is published explicitly before making the GitHub Release public, so a release publication event does not retry an already-published PyPI version.
- `v0.1.0` is the public non-draft latest release. The `v0.1.0-rc.3` draft remains historical release-candidate evidence.

### Verification evidence

- Production PyPI publish run `28596078256` succeeded for `pygco-dump 0.1.0`; build, `twine check`, built-wheel smoke, and `publish-pypi` passed.
- `python3 -m pip index versions pygco-dump` returned `pygco-dump (0.1.0)`.
- Clean production PyPI install/import passed:
  - `python3 -m venv .scratch/pypi-venv`
  - `python -m pip install 'pygco-dump[fastapi]'`
  - imported `write_gc_dump` and `gc_heap_dump_route`.
- Unit-only CI run `28596330932` passed on commit `d9b809dd66c1ff24ed4e7cf3a5a044a32fe984f1` with `rust-unit`, `python-unit`, and `web-unit`.
- Release workflow run `28596419592` succeeded for tag `v0.1.0`; Linux and macOS artifacts built, smoke-tested, packaged, attested, and attached to a draft release.
- `v0.1.0` was published as non-draft and marked latest.
- GitHub Release assets for `v0.1.0` include:
  - `install.sh`
  - `checksums.txt`
  - `pygco-0.1.0-x86_64-unknown-linux-gnu.tar.gz`
  - `pygco-0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256`
  - `pygco-0.1.0-x86_64-apple-darwin.tar.gz`
  - `pygco-0.1.0-x86_64-apple-darwin.tar.gz.sha256`
  - `pygco-0.1.0-aarch64-apple-darwin.tar.gz`
  - `pygco-0.1.0-aarch64-apple-darwin.tar.gz.sha256`
- `curl -fsSL https://github.com/ivan-94/py-gc-objects-analyze/releases/latest/download/install.sh | PYGCO_INSTALL_DIR="$PWD/.scratch/latest-pipe-install/bin" sh` installed `pygco`.
- Installed latest `pygco` ran `version`, fixture import, `summary`, and `open fixtures/golden/tiny-v1.jsonl.gz --no-browser --port 0 --cleanup-on-exit`.

### Open questions / risks

- The macOS x86_64 archive has release workflow build coverage and previous Rosetta runtime smoke from `v0.1.0-rc.3`; physical Intel Mac verification remains useful but is not a P0 blocker.
- GitHub Actions still emits Node.js 20 deprecation annotations for third-party actions. Dependabot action-update PRs should address this separately.

## HAT Checklist

- [x] Install `pygco` through the public latest release installer on macOS Apple Silicon.
- [x] Verify the latest release archive checksum for macOS Apple Silicon.
- [x] Run `pygco version` from the latest installed binary.
- [x] Run fixture import and `summary` from the latest installed binary.
- [x] Run `pygco open fixtures/golden/tiny-v1.jsonl.gz --no-browser --port 0 --cleanup-on-exit` and observe a loopback Web UI URL.
- [x] Install `pygco-dump[fastapi]` from production PyPI in a clean virtual environment.
- [x] Confirm GitHub Release `v0.1.0` is public, non-draft, and latest.
- [x] Confirm routine PR/push CI remains unit-only.

## Result

`v0.1.0` is accepted for public open source delivery. The GitHub Release latest installer path and production PyPI package install path are both verified.
