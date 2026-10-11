# Profile fixtures

Test input for `prose_core::profile`. They live here, not in
`tests/fixtures/`, so the frozen v0.1 golden set is unchanged.

| file | what it is | source |
|---|---|---|
| `research.jsonld` | a synthetic research-data profile, compacted JSON-LD | written for these tests |
| `research-expanded.json` | the same 118 triples, expanded by rdflib 6.1.1, nodes and values put back in document order | generated from `research.jsonld` |
| `research.ttl` | the same 118 triples in Turtle, same order | generated from `research.jsonld`; isomorphic with rdflib |
| `research-policy.json` | an Agreement that uses the profile, plus the undeclared `ex:distill` | written for these tests |
| `odrl22-excerpt.json` | 13 nodes of the ODRL 2.2 vocabulary, unchanged | W3C, see below |
| `hostile/*` | inputs that are not profiles, or are written to hurt | written for these tests |

`prose:` in `research.*` is the ds-prose suggestion extension
(`https://ds-labs-org.github.io/ds-prose-rs/profile#`), a ds-prose
convention, not ODRL.

## W3C notice

`odrl22-excerpt.json` is an excerpt of
<https://www.w3.org/ns/odrl/2/ODRL22.json>, the ODRL Version 2.2 vocabulary.
Copyright © World Wide Web Consortium (W3C®). Published under the W3C
Software and Document License,
<https://www.w3.org/copyright/software-license-2023/>; the vocabulary's own
licence statement is kept in the excerpt (`dct:license` on its ontology
node).
