# 安装与构建

## Source Manifest

- Root project overview: `README.md`
- CLI contract: `docs/cli.md`
- Web build architecture: `docs/architecture.md`
- Python package metadata: `python/pygco_dump/pyproject.toml`
- Release checklist: `task.md`

## Prerequisites

- Rust stable toolchain.
- Python 3.12 or newer for local development and `pygco-dump`.
- Node.js 22 with Corepack for building the Web UI assets. `web/app/package.json`
  pins the pnpm version.

## Build `pygco`

Release binaries should embed the real React build:

```bash
cd web/app
corepack pnpm install --frozen-lockfile
corepack pnpm build
cd ../..
cargo build --release -p pygco-cli
./target/release/pygco version
```

If `web/app/dist` is missing, the Rust build still succeeds with a minimal fallback page. Release builds should not rely on that fallback.

## Install `pygco-dump`

From a built wheel:

```bash
python -m pip install "python/pygco_dump/dist/pygco_dump-0.1.0-py3-none-any.whl[fastapi]"
```

From the source tree during development:

```bash
python -m pip install -e "python/pygco_dump[fastapi,test]"
```

After publishing to a package index, the intended install command is:

```bash
python -m pip install "pygco-dump[fastapi]"
```

## Run First Analysis

```bash
./target/release/pygco open fixtures/golden/tiny-v1.jsonl.gz --no-browser
```

Then open the printed local URL. By default `pygco open` keeps `.pygco/sessions/<timestamp>/` after exit so the generated `analysis.sqlite` and `import.log` remain inspectable.

## Development Web Flow

```bash
cd web/app
corepack pnpm dev
cd ../..
cargo run -p pygco-cli -- web analysis.sqlite --dev --no-browser
```

`--dev` points the browser target at the React dev server and serves `/api` from the Rust process on `127.0.0.1:5174` by default.
