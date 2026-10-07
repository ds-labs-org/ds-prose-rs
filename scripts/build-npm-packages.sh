#!/usr/bin/env bash
# Builds both npm packages and packs them into dist-npm/tarballs:
#   @ds-labs/prose-wasm     (wasm-pack output; scripts/package-prose-wasm.mjs)
#   @ds-labs/prose-angular  (ng-packagr; depends on the wasm package)
#
#   scripts/build-npm-packages.sh
#   PROSE_WASM_VERSION=0.2.0 PROSE_ANGULAR_VERSION=0.1.1 scripts/build-npm-packages.sh
#
# The versions default to the ones in prose-wasm/npm/package.json and
# prose-angular/package.json; the release workflow sets them from the tag.
set -euo pipefail
cd "$(dirname "$0")/.."

node scripts/package-prose-wasm.mjs "${PROSE_WASM_VERSION:-}"

cd prose-angular
if [ -n "${PROSE_ANGULAR_VERSION:-}" ]; then
  npm pkg set "version=${PROSE_ANGULAR_VERSION}"
fi
# The wasm package is not on the registry yet when this runs: install the
# build just made, in place of the declared range.
npm install --no-audit --no-fund --no-save ../dist-npm/prose-wasm
npm run build
cd ..

rm -rf dist-npm/tarballs
mkdir -p dist-npm/tarballs
(cd dist-npm/tarballs && npm pack --silent ../prose-wasm ../prose-angular)
ls -l dist-npm/tarballs
