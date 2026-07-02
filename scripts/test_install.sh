#!/bin/sh
set -eu

version="0.0.0-test"
target="$(PYGCO_VERSION="$version" sh scripts/install.sh --print-target)"
tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT INT TERM

release_dir="$tmp_dir/release"
payload_dir="$tmp_dir/payload"
install_dir="$tmp_dir/bin"
mkdir -p "$release_dir" "$payload_dir" "$install_dir"

cat > "$payload_dir/pygco" <<'SCRIPT'
#!/bin/sh
if [ "${1:-}" = "version" ]; then
  printf '0.0.0-test\n'
else
  printf 'fake pygco\n'
fi
SCRIPT
chmod 755 "$payload_dir/pygco"

archive="$release_dir/pygco-${version}-${target}.tar.gz"
tar -C "$payload_dir" -czf "$archive" pygco

if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$archive" > "$archive.sha256"
elif command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$archive" > "$archive.sha256"
else
  printf 'missing checksum tool: install sha256sum or shasum\n' >&2
  exit 1
fi

PYGCO_VERSION="$version" \
PYGCO_INSTALL_DIR="$install_dir" \
PYGCO_DOWNLOAD_BASE_URL="file://$release_dir" \
  sh scripts/install.sh >/tmp/pygco-install-test.log

"$install_dir/pygco" version | grep "0.0.0-test" >/dev/null
grep "installed pygco" /tmp/pygco-install-test.log >/dev/null
