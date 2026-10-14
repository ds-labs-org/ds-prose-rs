#!/usr/bin/env bash
# Generates site/content/catalog/ from ds-odrl-compliance-rdf.
#
#   scripts/build-catalog.sh [<corpus-checkout>]
#
# Without an argument the corpus is cloned (shallow, COMPLIANCE_REF, default
# main) into target/compliance-rdf. CI checks it out itself and passes the
# directory. The case files are linked at the commit that was read.
set -euo pipefail
cd "$(dirname "$0")/.."
repo=https://github.com/ds-labs-org/ds-odrl-compliance-rdf
dir=${1:-}
if [ -z "$dir" ]; then
  dir=target/compliance-rdf
  rm -rf "$dir"
  git clone --quiet --depth 1 --branch "${COMPLIANCE_REF:-main}" "$repo.git" "$dir"
fi
sha=$(git -C "$dir" rev-parse HEAD)
echo "ds-odrl-compliance-rdf at $sha"
cargo run --quiet --locked -p compliance-catalog -- "$dir/cases" site/content/catalog "$repo/blob/$sha/cases"
