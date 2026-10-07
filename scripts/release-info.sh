#!/usr/bin/env bash
# Reads a release tag and prints "<package> <version>", or fails.
#   prose-wasm-v0.1.0     -> prose-wasm 0.1.0
#   prose-angular-v1.2.3-rc.1 -> prose-angular 1.2.3-rc.1
set -euo pipefail
tag="${1:?usage: release-info.sh <tag>}"
if [[ "$tag" =~ ^(prose-wasm|prose-angular)-v([0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?)$ ]]; then
  echo "${BASH_REMATCH[1]} ${BASH_REMATCH[2]}"
else
  echo "not a release tag: $tag (expected prose-wasm-vX.Y.Z or prose-angular-vX.Y.Z)" >&2
  exit 1
fi
