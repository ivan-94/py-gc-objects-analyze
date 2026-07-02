#!/bin/sh
set -eu

REPO="ivan-94/py-gc-objects-analyze"
DEFAULT_VERSION="__PYGCO_VERSION__"

usage() {
  cat <<'USAGE'
Install pygco from GitHub Releases.

Usage:
  install.sh
  install.sh --print-target

Environment:
  PYGCO_VERSION             Release version without leading v. Defaults to the version baked into the release install.sh.
  PYGCO_INSTALL_DIR        Install directory. Defaults to $HOME/.local/bin.
  PYGCO_DOWNLOAD_BASE_URL  Override release asset base URL for tests or mirrors.
USAGE
}

target_triple() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      case "$arch" in
        x86_64 | amd64) printf '%s\n' "x86_64-unknown-linux-gnu" ;;
        *) printf 'unsupported Linux architecture: %s\n' "$arch" >&2; return 1 ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64) printf '%s\n' "x86_64-apple-darwin" ;;
        arm64 | aarch64) printf '%s\n' "aarch64-apple-darwin" ;;
        *) printf 'unsupported macOS architecture: %s\n' "$arch" >&2; return 1 ;;
      esac
      ;;
    *)
      printf 'unsupported operating system: %s\n' "$os" >&2
      return 1
      ;;
  esac
}

sha256_file() {
  file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
  else
    printf 'missing checksum tool: install sha256sum or shasum\n' >&2
    return 1
  fi
}

download() {
  url="$1"
  output="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$output"
  else
    printf 'missing required command: curl\n' >&2
    return 1
  fi
}

case "${1:-}" in
  --help | -h)
    usage
    exit 0
    ;;
  --print-target)
    target_triple
    exit 0
    ;;
  "")
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac

if ! command -v tar >/dev/null 2>&1; then
  printf 'missing required command: tar\n' >&2
  exit 1
fi

if [ "${PYGCO_VERSION:-}" ]; then
  version="$PYGCO_VERSION"
else
  version="$DEFAULT_VERSION"
fi

if [ "$version" = "__PYGCO_VERSION__" ]; then
  printf 'install.sh was not stamped with a release version; set PYGCO_VERSION explicitly.\n' >&2
  exit 1
fi

target="$(target_triple)"
asset="pygco-${version}-${target}.tar.gz"

if [ "${PYGCO_DOWNLOAD_BASE_URL:-}" ]; then
  base="${PYGCO_DOWNLOAD_BASE_URL%/}"
elif [ "${PYGCO_VERSION:-}" ]; then
  base="https://github.com/${REPO}/releases/download/v${version}"
else
  base="https://github.com/${REPO}/releases/latest/download"
fi

install_dir="${PYGCO_INSTALL_DIR:-"$HOME/.local/bin"}"
tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT INT TERM

archive="$tmp_dir/$asset"
checksum_file="$tmp_dir/$asset.sha256"

download "$base/$asset" "$archive"
download "$base/$asset.sha256" "$checksum_file"

expected="$(awk '{print $1}' "$checksum_file")"
actual="$(sha256_file "$archive")"

if [ "$expected" != "$actual" ]; then
  printf 'checksum mismatch for %s\nexpected: %s\nactual:   %s\n' "$asset" "$expected" "$actual" >&2
  exit 1
fi

tar -xzf "$archive" -C "$tmp_dir"
if [ ! -f "$tmp_dir/pygco" ]; then
  printf 'archive did not contain pygco executable\n' >&2
  exit 1
fi

mkdir -p "$install_dir"
cp "$tmp_dir/pygco" "$install_dir/pygco"
chmod 755 "$install_dir/pygco"

printf 'installed pygco to %s\n' "$install_dir/pygco"
"$install_dir/pygco" version
