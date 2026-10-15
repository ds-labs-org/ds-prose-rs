## Stable hooks: read-only renderer

These classes are written by `OdrlProse` (Yew), by `ProseHandle` (the wasm bridge) and by `<ds-odrl-prose>` (Angular). `OdrlProseView` in read mode writes the same names. The Rust source is `prose-yew/src/lib.rs`; the output is pinned by the snapshots in `prose-yew/tests/snapshots/`.

| Selector | Element | What it styles |
|---|---|---|
| `.ds-prose` | `article` | The root. The `class` prop, or `articleClass` in Angular, is appended here. |
| `.prose-policy` | `section` | One policy: an `h3`, an intro `p`, then the parts below. |
| `.prose-notes` | `ul` | Notes about the policy, only when there are some. |
| `.prose-rules` | `ol` | The rules of one policy. |
| `.prose-rule` | `li` | One rule. A duty, remedy or consequence nested under a rule is also a `li.prose-rule`. |
| `.prose-permission`, `.prose-prohibition`, `.prose-obligation` | `li` | The kind of the rule, next to `.prose-rule`. A duty is a `.prose-obligation`. |
| `.prose-rule-sentence` | `p` | The sentence of a rule. |
| `.prose-reference` | `a` | The IRI of a constraint the policy refers to and does not declare, in "the constraint defined at … holds". A link only when the host lists the IRI in `links` (Yew), as the site's catalog does for IRIs that answer HTTP 200; otherwise the IRI is plain text. The wasm bridge and Angular pass no links, so they do not write it. |
| `.prose-label` | `p` | A lead-in such as "Applies only if:", and the heading of the warnings block. |
| `.prose-warnings` | `aside` | The "Not rendered" block: parts of the policy the renderer did not turn into sentences. |
| `.prose-error` | `p[role="alert"]` | Replaces the policy when the JSON cannot be read. It sits inside `article.ds-prose`. The Angular component also writes it when the wasm module cannot be loaded; that one is a child of `ds-odrl-prose`, outside `article.ds-prose`, and `.ds-prose .prose-error` does not match it. Select `ds-odrl-prose > .prose-error`. |
| `.prose-glossary` | `details[open]` | Terms of the loaded profile that the policy uses, after the policy. At most 30 entries. |
| `.prose-note`, `.prose-kind-of`, `.prose-implies`, `.prose-deprecated`, `.prose-from` | `dd` | One line of a glossary entry: usage note, "A kind of", "Implies", deprecation, source profile. |
| `.prose-violations` | `section` | Validation findings and the verdict, after the policies. Written only with a validator and the findings switched on. |
| `.prose-violations-status` | `p[role="status"]` | The verdict line. Written only for a strict validation. |
| `.prose-conforms`, `.prose-incomplete`, `.prose-violations-found` | `p` | The verdict, next to `.prose-violations-status`: conforms, cannot be certified, does not conform. |
| `.prose-violations-list` | `ul` | The findings. |
| `.prose-violation` | `li` | One finding. |
| `.prose-violation-error`, `.prose-violation-warning`, `.prose-violation-info` | `li` | The severity of a finding, next to `.prose-violation`. |
| `.prose-rule-id` | `code` | The rule id inside a finding, for example `prose.datatype-lexical`. |

## Elements without a class

The read-only renderer leaves some elements bare. Reach them by structure, which ties the stylesheet to the markup.

| Selector | Element | What it styles |
|---|---|---|
| `.prose-policy > h3` | `h3` | The policy heading. |
| `.prose-policy > p` | `p` | The intro sentence. |
| `.prose-rule > ul` | `ul` | The conditions and refinements of a rule, and the rules nested in a follow-up list. They follow a `p.prose-label`, which is the only thing telling them apart. |
| `.prose-glossary dt`, `.prose-glossary dd` | `dt`, `dd` | A glossary term (a `span` with the label, then a `code` with the written form) and its plain definition. |

The editor view, `OdrlProseView`, gives conditions and follow-up lists classes. The read-only renderer does not. This is a gap in the library, not a rule.
