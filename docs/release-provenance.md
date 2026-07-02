# Release Provenance

P0 release verification uses checksums. P1 release workflows also publish GitHub artifact attestations for release assets.

## P0 Verification

Each release archive has:

- `pygco-<version>-<target>.tar.gz`
- `pygco-<version>-<target>.tar.gz.sha256`
- `checksums.txt`

Manual verification:

```bash
sha256sum -c "pygco-0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256"
```

On macOS:

```bash
shasum -a 256 -c "pygco-0.1.0-aarch64-apple-darwin.tar.gz.sha256"
```

## P1 Artifact Attestations

The release workflow uses GitHub artifact attestations for:

- `install.sh`
- `checksums.txt`
- `pygco-<version>-<target>.tar.gz`
- per-archive `.sha256` files

After downloading release assets, verify an attestation with:

```bash
gh attestation verify "pygco-0.1.0-x86_64-unknown-linux-gnu.tar.gz" --repo ivan-94/py-gc-objects-analyze
```

Checksums prove the downloaded archive bytes match the release checksum. Attestations add a GitHub-signed link between the artifact digest, repository, workflow identity, and build run. They do not replace local runtime HAT or guarantee that the tool is free of vulnerabilities.

## Remaining Hardening Options

Evaluate these after the first release candidate:

- SLSA provenance,
- SBOM generation,
- pinned build toolchains through `rust-toolchain.toml` and package-manager metadata.

## Maintainer Checklist

- Verify the GitHub Release was created from the intended tag.
- Download assets from the release page, not from workflow artifacts only.
- Verify checksums on a machine separate from the build runner when practical.
- Verify at least one archive attestation with `gh attestation verify`.
- Run `pygco version` from the extracted archive.
- Run the clean-machine acceptance guide before publishing a non-draft release.
