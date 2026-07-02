#!/bin/sh
set -eu

version="${1:-}"
target="${2:-}"

if [ -z "$version" ]; then
  version="$(awk -F' = ' '/^version = / {gsub(/"/, "", $2); print $2; exit}' Cargo.toml)"
fi

if [ -z "$target" ]; then
  target="$(rustc -vV | awk '/^host:/ {print $2}')"
fi

binary="target/${target}/release/pygco"
if [ ! -x "$binary" ]; then
  binary="target/release/pygco"
fi

if [ ! -x "$binary" ]; then
  printf 'missing release binary for target %s; run cargo build --release -p pygco-cli first\n' "$target" >&2
  exit 1
fi

if [ ! -f web/app/dist/index.html ]; then
  printf 'missing web/app/dist/index.html; build the Web UI before packaging release binaries\n' >&2
  exit 1
fi

dist_dir="dist"
work_dir="${dist_dir}/pygco-${version}-${target}"
archive="${dist_dir}/pygco-${version}-${target}.tar.gz"

rm -rf "$work_dir"
mkdir -p "$work_dir" "$dist_dir"
cp "$binary" "$work_dir/pygco"
cp LICENSE "$work_dir/LICENSE"
cp README.md "$work_dir/README.md"

tar -C "$work_dir" -czf "$archive" pygco LICENSE README.md

if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$archive" > "${archive}.sha256"
elif command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$archive" > "${archive}.sha256"
else
  printf 'missing checksum tool: install sha256sum or shasum\n' >&2
  exit 1
fi

printf 'created %s\n' "$archive"
