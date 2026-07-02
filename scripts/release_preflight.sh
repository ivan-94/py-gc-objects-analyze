#!/bin/sh
set -eu

repo="${1:-ivan-94/py-gc-objects-analyze}"
tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT INT TERM

section() {
  printf '\n== %s ==\n' "$1"
}

have() {
  command -v "$1" >/dev/null 2>&1
}

status=0

section "local tools"
for cmd in git gh cargo python3; do
  if have "$cmd"; then
    printf 'ok: %s\n' "$cmd"
  else
    printf 'missing: %s\n' "$cmd"
    status=1
  fi
done

section "git remote"
if git remote get-url origin >/dev/null 2>&1; then
  git remote -v
else
  printf 'missing origin remote\n'
  status=1
fi

if git ls-remote --heads origin >"$tmp_dir/heads" 2>"$tmp_dir/git.err"; then
  if [ -s "$tmp_dir/heads" ]; then
    sed -n '1,20p' "$tmp_dir/heads"
  else
    printf 'warning: origin has no visible heads\n'
    status=1
  fi
else
  printf 'cannot read origin heads:\n'
  sed -n '1,20p' "$tmp_dir/git.err"
  status=1
fi

section "github cli"
if gh auth status >"$tmp_dir/gh-auth" 2>&1; then
  sed -n '1,20p' "$tmp_dir/gh-auth"
else
  sed -n '1,20p' "$tmp_dir/gh-auth"
  status=1
fi

if gh repo view "$repo" --json nameWithOwner,url,isPrivate,defaultBranchRef >"$tmp_dir/repo.json" 2>"$tmp_dir/gh.err"; then
  cat "$tmp_dir/repo.json"
  printf '\n'
  if ! grep -q '"defaultBranchRef":{"name":"[^"]' "$tmp_dir/repo.json"; then
    printf 'warning: repository default branch is missing or empty\n'
    status=1
  fi
else
  sed -n '1,20p' "$tmp_dir/gh.err"
  status=1
fi

section "github release state"
if gh release list --repo "$repo" --limit 10 >"$tmp_dir/releases" 2>"$tmp_dir/gh.err"; then
  if [ -s "$tmp_dir/releases" ]; then
    cat "$tmp_dir/releases"
  else
    printf 'no releases found\n'
  fi
else
  sed -n '1,20p' "$tmp_dir/gh.err"
  status=1
fi

section "github workflow state"
if gh run list --repo "$repo" --limit 10 >"$tmp_dir/runs" 2>"$tmp_dir/gh.err"; then
  if [ -s "$tmp_dir/runs" ]; then
    cat "$tmp_dir/runs"
  else
    printf 'no workflow runs found\n'
  fi
else
  sed -n '1,20p' "$tmp_dir/gh.err"
  status=1
fi

section "pypi state"
if python3 -m pip index versions pygco-dump >"$tmp_dir/pypi" 2>"$tmp_dir/pypi.err"; then
  sed -n '1,20p' "$tmp_dir/pypi"
else
  printf 'pygco-dump not visible on PyPI or pip index failed:\n'
  sed -n '1,20p' "$tmp_dir/pypi.err"
fi

section "local release files"
for path in scripts/install.sh scripts/package_release.sh scripts/test_install.sh .github/workflows/release.yml .github/workflows/publish-python.yml docs/release-acceptance.md; do
  if [ -f "$path" ]; then
    printf 'ok: %s\n' "$path"
  else
    printf 'missing: %s\n' "$path"
    status=1
  fi
done

exit "$status"
