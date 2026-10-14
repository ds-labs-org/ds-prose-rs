#!/usr/bin/env bash
# Generates site/content/catalog/ from its sources, fetched now:
#
#   ds-odrl-compliance-rdf   the compliance corpus (COMPLIANCE_REF, default main)
#   w3c/poe                  the ODRL Information Model, editors' draft (gh-pages)
#   eclipse-edc/Samples      EDC's samples (main)
#   DataspaceProtocol        the Dataspace Protocol's examples (main)
#
#   scripts/build-catalog.sh [<work-dir>]
#
# Each source is cloned shallow into <work-dir> (default target/catalog-src),
# and every page links to the commit it was read at (the W3C examples link
# to the published editors' draft). Nothing is kept in the
# repository.
set -euo pipefail
cd "$(dirname "$0")/.."
work=${1:-target/catalog-src}
rm -rf "$work"
mkdir -p "$work"

# clone <name> <repo-url> <branch>; prints the commit read.
clone() {
  git clone --quiet --depth 1 --branch "$3" "$2.git" "$work/$1"
  git -C "$work/$1" rev-parse HEAD
}

gh=https://github.com
compliance=$gh/ds-labs-org/ds-odrl-compliance-rdf
poe=$gh/w3c/poe
edc=$gh/eclipse-edc/Samples
dsp=$gh/eclipse-dataspace-protocol-base/DataspaceProtocol

c=$(clone compliance "$compliance" "${COMPLIANCE_REF:-main}")
p=$(clone poe "$poe" gh-pages)
e=$(clone edc "$edc" main)
d=$(clone dsp "$dsp" main)
echo "compliance $c, w3c/poe $p, edc $e, dsp $d"

cargo run --quiet --locked -p compliance-catalog -- site/content/catalog \
  --compliance "$work/compliance/cases" "$compliance/blob/$c/cases" \
  --w3c "$work/poe/model/index.html" "https://w3c.github.io/poe/model/" \
  --edc "$work/edc" "$edc/blob/$e" \
  --dsp "$work/dsp/artifacts/src/main/resources" "$dsp/blob/$d/artifacts/src/main/resources"
