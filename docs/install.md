# Install And Build

## Install `pygco`

The primary release install path downloads `install.sh` from the latest GitHub Release:

```bash
curl -fsSL https://github.com/ivan-94/py-gc-objects-analyze/releases/latest/download/install.sh | sh
```

The installer:

- detects OS and CPU architecture,
- downloads the matching `pygco-<version>-<target>.tar.gz`,
- verifies its `.sha256` file,
- installs the `pygco` executable to `$HOME/.local/bin` by default,
- prints the installed path and `pygco version`.

Choose another install directory with `PYGCO_INSTALL_DIR`:

```bash
curl -fsSL https://github.com/ivan-94/py-gc-objects-analyze/releases/latest/download/install.sh | PYGCO_INSTALL_DIR=/usr/local/bin sh
```

Install a specific version by downloading that version's installer:

```bash
curl -fsSL https://github.com/ivan-94/py-gc-objects-analyze/releases/download/v0.1.1/install.sh | sh
```

## Manual Binary Install

Manual install is recommended when you do not allow `curl | sh`:

```bash
version=0.1.1
target=x86_64-unknown-linux-gnu
base="https://github.com/ivan-94/py-gc-objects-analyze/releases/download/v${version}"

curl -fsSLO "${base}/pygco-${version}-${target}.tar.gz"
curl -fsSLO "${base}/pygco-${version}-${target}.tar.gz.sha256"
sha256sum -c "pygco-${version}-${target}.tar.gz.sha256"
tar -xzf "pygco-${version}-${target}.tar.gz"
mkdir -p "$HOME/.local/bin"
install -m 755 pygco "$HOME/.local/bin/pygco"
pygco version
```

On macOS, use `shasum -a 256 -c ...` if `sha256sum` is unavailable.

## Install `pygco-dump`

Install the Python dump producer in the environment that runs the target process:

```bash
python -m pip install "pygco-dump[fastapi]"
```

Supported Python versions start at Python 3.10. The `fastapi` extra is only needed for the FastAPI helper; framework-agnostic integrations can use `pygco_dump.write_gc_dump()` directly.

For local development from this repository:

```bash
python -m pip install -e "python/pygco_dump[fastapi,test]"
python -m pytest python/pygco_dump
```

## Run First Analysis

```bash
pygco open fixtures/golden/tiny-v1.jsonl.gz --no-browser
```

Open the printed local URL. By default `pygco open` keeps `analysis.sqlite`, `import.log`, and `manifest.json` under `PYGCO_HOME`, `XDG_CACHE_HOME/pygco`, or `~/.cache/pygco`.

## Upgrade

Run the latest installer again:

```bash
curl -fsSL https://github.com/ivan-94/py-gc-objects-analyze/releases/latest/download/install.sh | sh
```

Then upgrade the producer package in each Python environment that uses it:

```bash
python -m pip install --upgrade "pygco-dump[fastapi]"
```

Keep original dump files when upgrading. SQLite analysis files are rebuildable and may be regenerated when contracts change.

## Uninstall

Remove the binary from the install directory:

```bash
rm -f "$HOME/.local/bin/pygco"
```

Remove cached analysis sessions if desired:

```bash
pygco sessions list --format table
rm -rf ~/.cache/pygco/sessions/<session-id>
```

Uninstall the producer package from Python environments where it is installed:

```bash
python -m pip uninstall pygco-dump
```

## Build From Source

Prerequisites:

- Rust stable toolchain.
- Python 3.10 or newer for `pygco-dump`.
- Node.js 22 with Corepack for Web UI assets. `web/app/package.json` pins pnpm.

Release binaries should embed the real React build:

```bash
(cd web/app && corepack pnpm install --frozen-lockfile)
(cd web/app && corepack pnpm build)
PYGCO_WEB_DIST="$(pwd)/web/app/dist" cargo build --release -p pygco-cli
./target/release/pygco version
```

If `web/app/dist` is missing, the Rust build still succeeds with a minimal fallback page. Release builds must not rely on that fallback.

## Package A Local Release Archive

```bash
(cd web/app && corepack pnpm build)
cargo build --release -p pygco-cli
scripts/package_release.sh
```

The archive and `.sha256` file are written under `dist/`.

## Development Web Flow

```bash
(cd web/app && corepack pnpm dev)
cargo run -p pygco-cli -- web analysis.sqlite --dev --no-browser
```

`--dev` points the browser target at the React dev server and serves `/api` from the Rust process on `127.0.0.1:5174` by default.
