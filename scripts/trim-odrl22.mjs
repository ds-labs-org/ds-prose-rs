#!/usr/bin/env node
// Trims the W3C ODRL 2.2 vocabulary (ODRL22.json, expanded JSON-LD) to the
// subset the demo's "W3C ODRL 2.2 vocabulary" preset loads:
//
// - the owl:Ontology node, with its label, description, version, licence,
//   creators and contributor;
// - every node typed odrl:Action, odrl:LeftOperand, odrl:Operator,
//   odrl:RightOperand or odrl:ConflictTerm, with only what prose-core reads
//   for a term: rdfs:label, skos:definition, skos:note, odrl:includedIn,
//   odrl:implies, skos:broader, skos:exactMatch, owl:deprecated. Its @type
//   keeps only those ODRL kinds.
//
// Values and node order are unchanged; one node per line keeps it diffable.
//
//   curl -sSfLo ODRL22.json https://www.w3.org/ns/odrl/2/ODRL22.json
//   node scripts/trim-odrl22.mjs ODRL22.json demo/profiles/odrl22-trimmed.json
import { readFileSync, writeFileSync } from 'node:fs';

const [, , from, to] = process.argv;
if (!from || !to) {
  console.error('usage: trim-odrl22.mjs ODRL22.json out.json');
  process.exit(2);
}
const ODRL = 'http://www.w3.org/ns/odrl/2/';
const KINDS = ['Action', 'LeftOperand', 'Operator', 'RightOperand', 'ConflictTerm'].map((k) => ODRL + k);
const ONTOLOGY = 'http://www.w3.org/2002/07/owl#Ontology';
const TERM_KEYS = [
  '@id', '@type',
  'http://www.w3.org/2000/01/rdf-schema#label',
  'http://www.w3.org/2004/02/skos/core#definition',
  'http://www.w3.org/2004/02/skos/core#note',
  'http://www.w3.org/ns/odrl/2/includedIn',
  'http://www.w3.org/ns/odrl/2/implies',
  'http://www.w3.org/2004/02/skos/core#broader',
  'http://www.w3.org/2004/02/skos/core#exactMatch',
  'http://www.w3.org/2002/07/owl#deprecated',
];
const ONTOLOGY_KEYS = [
  '@id', '@type',
  'http://www.w3.org/2000/01/rdf-schema#label',
  'http://purl.org/dc/terms/description',
  'http://www.w3.org/2002/07/owl#versionInfo',
  'http://purl.org/dc/terms/license',
  'http://purl.org/dc/terms/creator',
  'http://purl.org/dc/terms/contributor',
];
const pick = (node, keys) => Object.fromEntries(Object.entries(node).filter(([k]) => keys.includes(k)));

const src = JSON.parse(readFileSync(from, 'utf8'));
const out = [];
for (const node of src) {
  const types = node['@type'] ?? [];
  if (types.includes(ONTOLOGY)) {
    out.push(pick(node, ONTOLOGY_KEYS));
  } else if (types.some((t) => KINDS.includes(t))) {
    out.push({ ...pick(node, TERM_KEYS), '@type': types.filter((t) => KINDS.includes(t)) });
  }
}
if (!out.some((n) => n['http://purl.org/dc/terms/license'])) {
  console.error('no licence statement on the ontology node: refusing to write a copy without it');
  process.exit(1);
}
writeFileSync(to, '[\n' + out.map((n) => JSON.stringify(n)).join(',\n') + '\n]\n');
console.log(`${src.length} nodes -> ${out.length} in ${to}`);
