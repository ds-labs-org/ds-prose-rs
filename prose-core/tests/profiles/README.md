# Profile fixtures

Test input for `prose_core::profile`. They live here, not in
`tests/fixtures/`, so the frozen v0.1 golden set is unchanged.

| file | what it is | source |
|---|---|---|
| `research.jsonld` | a synthetic research-data profile, compacted JSON-LD | written for these tests |
| `research-expanded.json` | the same 118 triples, expanded by rdflib 6.1.1, nodes and values put back in document order | generated from `research.jsonld` |
| `research.ttl` | the same 118 triples in Turtle, same order | generated from `research.jsonld`; isomorphic with rdflib |
| `research-rdflib.ttl` | the same 118 triples in Turtle, in rdflib's own order | `rdflib.Graph().parse("research.jsonld").serialize(format="turtle")`, rdflib 6.1.1 |
| `research-closed.jsonld` | `research.jsonld` plus `prose:closed true` on `ex:dataSubjectCount` and `ex:researchPurpose` | written for these tests |
| `research-closed.ttl` | its Turtle twin | `research.ttl` with the same two triples added |
| `plural-operand.jsonld` | a small profile with `prose:plural true` on two left operands, a singular one and two operator labels | written for these tests |
| `plural-operand.ttl` | its Turtle twin | written for these tests |
| `research-imports.jsonld` | a profile that imports `research` and is a profile of `base`, which no test loads | written for these tests |
| `other.jsonld` | a second small profile, for policies that mix profiles | written for these tests |
| `research-policy.json` | an Agreement that uses the profile, plus the undeclared `ex:distill` | written for these tests |
| `odrl22-excerpt.json` | 13 nodes of the ODRL 2.2 vocabulary, unchanged | W3C, see below |
| `hostile/*` | inputs that are not profiles, or are written to hurt | written for these tests |

`prose:` in `research.*` is the ds-prose suggestion extension
(`https://ds-labs-org.github.io/ds-prose-rs/profile#`), a ds-prose
convention, not ODRL. `prose:closed` makes a left operand's hints binding
in strict validation only. `prose:plural` says a left operand's label is
a plural noun phrase, so its operators agree in the plural.

The policies strict validation is tested on are in `../strict/`; each file
in `../strict/hostile/` holds one policy that triggers exactly the rule it
is named after, with the profiles and options it is checked under.

## W3C notice

`odrl22-excerpt.json` is an excerpt of
<https://www.w3.org/ns/odrl/2/ODRL22.json>, the ODRL Version 2.2 vocabulary.
Copyright © World Wide Web Consortium (W3C®). Published under the W3C
Software and Document License,
<https://www.w3.org/copyright/software-license-2023/>; the vocabulary's own
licence statement is kept in the excerpt (`dct:license` on its ontology
node).
`../golden_profile/agreement_odrl22.txt` quotes labels and definitions
from the excerpt; the same notice applies to them.
