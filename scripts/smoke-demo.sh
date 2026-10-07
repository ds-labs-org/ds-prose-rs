#!/usr/bin/env bash
# Boot check for the built demo: serve demo/dist under the same sub-path the
# Pages deploy uses, load it in headless Chrome, and fail unless the wasm
# actually rendered a policy. A wasm that fails to boot leaves a blank page
# and is silent otherwise.
#
#   scripts/smoke-demo.sh [dist-dir] [base-path]
#   CHROME=/path/to/chrome overrides the browser lookup.
#   SMOKE_TITLE overrides the text the page must show (default: the Trunk
#   demo's heading); the Angular demo sets it to its own.
set -euo pipefail

dist="${1:-demo/dist}"
base="${2:-/ds-prose-rs/}"
port="${SMOKE_PORT:-8741}"
title="${SMOKE_TITLE:-ds-prose-rs demo}"

chrome="${CHROME:-}"
if [ -z "$chrome" ]; then
  for c in google-chrome chromium chromium-browser; do
    if command -v "$c" >/dev/null 2>&1; then chrome="$c"; break; fi
  done
fi
if [ -z "$chrome" ]; then
  echo "no Chrome or Chromium found; set CHROME" >&2
  exit 2
fi

root="$(mktemp -d)"
profile="$(mktemp -d)"
server=""
cleanup() {
  [ -n "$server" ] && kill "$server" 2>/dev/null || true
  rm -rf "$root" "$profile"
}
trap cleanup EXIT

mkdir -p "$root$base"
cp -r "$dist"/. "$root$base"
(cd "$root" && exec python3 -m http.server "$port" --bind 127.0.0.1 >/dev/null 2>&1) &
server=$!

for _ in $(seq 1 20); do
  curl -sf -o /dev/null "http://127.0.0.1:$port$base" && break
  sleep 0.5
done

dom="$("$chrome" --headless=new --disable-gpu --no-sandbox \
  --user-data-dir="$profile" --virtual-time-budget=8000 \
  --dump-dom "http://127.0.0.1:$port$base" 2>/dev/null)"

fail=0
# Without a doctype the page renders in quirks mode, which no host test sees.
if ! grep -qi '^<!doctype html>' <<<"$dom"; then
  echo "boot check FAILED: no <!DOCTYPE html> (page would render in quirks mode)" >&2
  fail=1
fi
for needle in "$title" 'prose-policy'; do
  if ! grep -q "$needle" <<<"$dom"; then
    echo "boot check FAILED: '$needle' not in the rendered page" >&2
    fail=1
  fi
done
[ "$fail" -eq 0 ] && echo "boot check ok: the demo rendered a policy"
exit "$fail"
