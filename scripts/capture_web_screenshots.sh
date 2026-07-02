#!/bin/sh
set -eu

out_dir="${1:-docs/assets/web-ui}"
db=".scratch/web-screenshots.sqlite"
log=".scratch/web-screenshots.log"
import_log=".scratch/web-screenshots-import.json"

mkdir -p "$out_dir" .scratch
out_dir="$(cd "$out_dir" && pwd)"

cargo build -p pygco-cli
(cd web/app && corepack pnpm install --frozen-lockfile)
if [ ! -x web/app/node_modules/.bin/tsc ]; then
  rm -rf web/app/node_modules
  (cd web/app && corepack pnpm install --frozen-lockfile)
fi
(cd web/app && corepack pnpm build)

PYGCO_WEB_DIST="$PWD/web/app/dist" target/debug/pygco import \
  fixtures/golden/diff-before-v1.jsonl.gz \
  fixtures/golden/diff-after-v1.jsonl.gz \
  fixtures/golden/stubs-v1.jsonl.gz \
  fixtures/golden/missing-referents-v1.jsonl.gz \
  -o "$db" --rebuild > "$import_log"

PYGCO_WEB_DIST="$PWD/web/app/dist" target/debug/pygco web "$db" --host 127.0.0.1 --port 0 --no-browser > "$log" 2>&1 &
pid=$!
cleanup() {
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

url=""
for _ in $(seq 1 80); do
  url="$(grep -Eo 'http://127[.]0[.]0[.]1:[0-9]+/' "$log" | head -n 1 || true)"
  if [ -n "$url" ]; then
    break
  fi
  sleep 0.25
done

if [ -z "$url" ]; then
  cat "$log" >&2 || true
  exit 1
fi

shot() {
  name="$1"
  path="$2"
  wait_ms="${3:-1000}"
  (cd web/app && corepack pnpm exec playwright screenshot --wait-for-timeout "$wait_ms" --full-page "${url}${path}" "$out_dir/$name.png")
}

shot overview ""
shot objects "?page=objects&snapshot=2&sort=object-id"
shot object-detail "?page=objects&snapshot=2&sort=object-id&selected=101"
shot graph "?page=graph&root=100" 3000
shot diff "?page=diff&from=1&to=2"
shot sql "?page=sql"
shot report "?page=report"

printf 'wrote screenshots to %s\n' "$out_dir"
