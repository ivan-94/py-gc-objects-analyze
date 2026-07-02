# Release Provenance

P0 release verification uses checksums. Stronger signing and provenance are P1 hardening items.

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

## P1 Hardening Options

Evaluate these after the first release candidate:

- signing `checksums.txt`,
- GitHub artifact attestations,
- SLSA provenance,
- SBOM generation,
- pinned build toolchains through `rust-toolchain.toml` and package-manager metadata.

## Maintainer Checklist

- Verify the GitHub Release was created from the intended tag.
- Download assets from the release page, not from workflow artifacts only.
- Verify checksums on a machine separate from the build runner when practical.
- Run `pygco version` from the extracted archive.
- Run the clean-machine acceptance guide before publishing a non-draft release.
