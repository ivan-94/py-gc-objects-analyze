# Troubleshooting

Use this page when install, collection, import, analysis, or the local Web UI does not behave as expected.

## Installer: Unsupported Target

The release installer supports the P0 release target matrix only:

- `x86_64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`

Check the target detected by the installer:

```bash
curl -fsSL https://github.com/ivan-94/py-gc-objects-analyze/releases/latest/download/install.sh | sh -s -- --print-target
```

If the target is unsupported, build from source:

```bash
cargo build --release -p pygco-cli
```

## Installer: Missing curl, tar, or Checksum Tool

The installer requires `curl`, `tar`, and either `sha256sum` or `shasum`.

On macOS, `shasum` is available by default. On minimal Linux images, install the distribution package that provides `curl`, `tar`, and `sha256sum`.

## pygco Is Not on PATH

The installer defaults to:

```text
$HOME/.local/bin/pygco
```

Confirm the binary exists:

```bash
ls -l "$HOME/.local/bin/pygco"
"$HOME/.local/bin/pygco" version
```

Add the directory to `PATH` in your shell profile:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Or install into another directory:

```bash
curl -fsSL https://github.com/ivan-94/py-gc-objects-analyze/releases/latest/download/install.sh | PYGCO_INSTALL_DIR=/usr/local/bin sh
```

## Web UI Port Conflicts

`pygco open` and `pygco web` use `--port 0` by default, which asks the OS for a free port. If you need a fixed port:

```bash
pygco open dump.jsonl.gz --host 127.0.0.1 --port 3791 --no-browser
```

If the port is already in use, pick another port or omit `--port`.

## Dump Endpoint Is Huge or Slow

GC dumps can be large. Start with conservative parameters:

```bash
curl -o heap.jsonl.gz "http://service/debug/gc-heap-dump?collect=false&include_repr=false"
```

Avoid enabling `include_repr` unless you need it. Object `repr` can be slow, huge, or sensitive.

## collect=true Adds Latency

`collect=true` triggers Python GC before dumping and can affect request latency. Keep `collect=false` for production-adjacent debugging unless you intentionally need a post-GC snapshot.

## Reachability Is truncated or unavailable

`truncated` means the bounded graph walk hit a configured depth, node, or fanout limit. Re-run import with explicit reachability options if you need a wider estimate.

`unavailable` means the dump did not include referents, so reachable-size estimates cannot be computed.

## Malformed Dump Line Errors

Import errors should include the line number and reason. Check:

- the file is gzip JSONL,
- the first record is a supported metadata/start record,
- object records include the expected dump format version,
- the dump was not truncated while downloading.

Run a small fixture first to separate tool setup from dump quality:

```bash
pygco import fixtures/golden/tiny-v1.jsonl.gz -o .scratch/tiny.sqlite --rebuild
```

## Read-only SQL Rejection

The SQL workbench accepts read-only `SELECT` and `WITH` queries. Mutating statements such as `DELETE`, `INSERT`, `UPDATE`, and `CREATE` are rejected.

Use:

```sql
select object_id, type, shallow_size
from objects
limit 20;
```

## Large SQLite Cleanup

SQLite analysis databases are rebuildable local artifacts. Clean up explicit databases and cached `pygco open` sessions when you are done:

```bash
rm -f analysis.sqlite
pygco sessions list
```

Cached sessions live under `PYGCO_HOME`, `XDG_CACHE_HOME/pygco`, or `~/.cache/pygco`.
