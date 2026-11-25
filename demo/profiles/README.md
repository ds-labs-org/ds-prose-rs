# Demo profile presets

Profiles the demo's Profile field offers as presets. They are compiled into
the demo only (`include_str!` in `demo/src/profile_field.rs`), never into
the library crates or the npm packages.

| file | what it is | source |
|---|---|---|
| `odrl22-trimmed.json` | the ODRL 2.2 vocabulary, trimmed to what the reader uses | W3C, see below |

The research profile presets (JSON-LD and Turtle) are the test fixtures in
`prose-core/tests/profiles/`, used as they are.

## How `odrl22-trimmed.json` was made

From <https://www.w3.org/ns/odrl/2/ODRL22.json> as fetched on 2026-10-11
(264,724 bytes, Last-Modified 2025-09-03, sha256
`a897cbb82c08f1d16e4aec20a3acfe7cd91595a0642597965d5ce5f40f530637`):

```bash
curl -sSfLo ODRL22.json https://www.w3.org/ns/odrl/2/ODRL22.json
node scripts/trim-odrl22.mjs ODRL22.json demo/profiles/odrl22-trimmed.json
```

The script keeps 123 of the 276 nodes:

- the `owl:Ontology` node, with its label, description, version, licence
  statement (`dct:license`), creators and contributor;
- every node typed `odrl:Action` (72), `odrl:LeftOperand` (34),
  `odrl:Operator` (12), `odrl:RightOperand` (1) or `odrl:ConflictTerm` (3),
  with `rdfs:label`, `skos:definition`, `skos:note`, `odrl:includedIn`,
  `odrl:implies`, `skos:broader`, `skos:exactMatch` and `owl:deprecated`.
  Their `@type` keeps only those ODRL kinds.

Left out: classes, properties, SKOS collections, party scopes,
`skos:scopeNote`, `rdfs:isDefinedBy`, domains and ranges. Values and node
order are unchanged.

## W3C notice

`odrl22-trimmed.json` is an excerpt of the ODRL Version 2.2 vocabulary,
<https://www.w3.org/ns/odrl/2/ODRL22.json>. Copyright © World Wide Web
Consortium (W3C®). Published under the W3C Software and Document License,
<https://www.w3.org/copyright/software-license-2023/>; the vocabulary's own
licence statement is kept in the excerpt (`dct:license` on its ontology
node).
