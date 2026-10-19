# ds-prose-rs

**PROSE: Prose Renderer for ODRL Semantic Encoding.** A
[Yew](https://yew.rs) component that reads an ODRL policy written as JSON-LD
and writes it as plain-language prose, for people who have to decide whether
to accept a policy but should not have to read JSON-LD to do it.

## Documentation and demos

The site is at <https://ds-labs-org.github.io/ds-prose-rs/>. Each part of it is a page of its own.

| page | what it holds |
|---|---|
| [Home](https://ds-labs-org.github.io/ds-prose-rs/) | an ODRL policy and the sentences the renderer writes for it, and where each package runs |
| [Yew demo](https://ds-labs-org.github.io/ds-prose-rs/demo/) | paste a policy and read it, or edit the prose; a Profile field; strict validation; a switch between unstyled and the example stylesheet |
| [Angular demo](https://ds-labs-org.github.io/ds-prose-rs/demo-angular/) | the read-only Angular component with `strict`, `profile` and `validationChange`, and the same switch |
| [Yew docs](https://ds-labs-org.github.io/ds-prose-rs/yew/) | `prose-yew`: [interface](https://ds-labs-org.github.io/ds-prose-rs/yew/interface/), [styling](https://ds-labs-org.github.io/ds-prose-rs/yew/styling/), [validation](https://ds-labs-org.github.io/ds-prose-rs/yew/validation/), [CSS hooks](https://ds-labs-org.github.io/ds-prose-rs/yew/css-hooks/) |
| [Angular docs](https://ds-labs-org.github.io/ds-prose-rs/angular/) | `@ds-labs/prose-angular`: [interface](https://ds-labs-org.github.io/ds-prose-rs/angular/interface/), [styling](https://ds-labs-org.github.io/ds-prose-rs/angular/styling/), [validation](https://ds-labs-org.github.io/ds-prose-rs/angular/validation/), [CSS hooks](https://ds-labs-org.github.io/ds-prose-rs/angular/css-hooks/) |

The two demos and the docs share one top navigation. The stylesheets the
Styling pages print are files in `site/content/examples/`, and both demos load
those files, so the docs and the demos cannot drift apart.

## Layout

| path | what it is |
|---|---|
| `prose-core/` | reads ODRL JSON-LD, builds a structured `Document`. No UI framework |
| `prose-core/src/edit/` | the edit layer: an addressable policy model, edit events, edit rules, a JSON-LD reader and writer, and the edit sentence templates |
| `prose-core/src/validate/` | advisory and strict validation of a policy against the loaded profiles: the rule catalogue, the verdict and the `ds-prose/validation-report@1` JSON (see Strict validation) |
| `prose-core/src/profile/` | reads an ODRL profile (JSON-LD, or Turtle with the `turtle` feature) into a `ProfileVocab`; `Wording` carries the loaded profiles to the reader and the editor |
| `prose-yew/` | `<OdrlProse json={...} />`: lays a `Document` out as semantic HTML, with a glossary of profile terms. `<OdrlProseView>`: the same sentences for documents written with bare keys, read or edited |
| `demo/` | Trunk app: paste a policy and read it, or edit the prose itself; a Profile field loads a profile by paste, preset, file or URL |
| `demo/profiles/` | profile presets compiled into the demo only (the trimmed W3C ODRL 2.2 vocabulary) |
| `prose-wasm/` | wasm-bindgen bridge that mounts the read-only `<OdrlProse>` into any DOM element, with `setProfile`, plus `validate` and the `ds-prose-validate` command; `prose-wasm/npm/` holds the metadata of the npm package |
| `prose-angular/` | Angular wrapper, `<ds-odrl-prose [json] [profile]>`, over `prose-wasm` (read-only; no edit mode), with strict validation inputs; built with ng-packagr |
| `demo-angular/` | Angular demo app; installs the two packed npm packages, like a downstream app |
| `catalog/` | `compliance-catalog`: builds the site's [catalog](https://ds-labs-org.github.io/ds-prose-rs/catalog/) of real-world policies: the [ds-odrl-compliance-rdf](https://github.com/ds-labs-org/ds-odrl-compliance-rdf) corpus (translated from Turtle), the W3C ODRL Information Model examples, the EDC samples and the Dataspace Protocol examples, with a coverage table. `scripts/build-catalog.sh` fetches the sources at build time; nothing of them is kept here |

```rust
use prose_yew::OdrlProse;
html! { <OdrlProse json={policy_json} /> }
```

Or without a UI: `prose_core::render(json) -> Result<Document, ProseError>`.
With a profile: `prose_core::render_with(json, &wording) -> Result<Reading, ProseError>`,
where `Reading` holds the `Document` and a glossary per policy (see Profiles).

## What it reads

Sets, offers and agreements (also under `@graph` or in an array); permissions,
prohibitions and obligations; duties, remedies and consequences nested under
them; assigner, assignee and target, inherited from the policy when a rule
states none; actions, including refinements; atomic constraints and the logical
`and` / `or` / `xone` / `andSequence`; `profile`, `conflict`, `inheritFrom`.
Compact IRIs (`odrl:use`), full ODRL IRIs, `@list`, `@value`/`@type`, and single
values or arrays are all accepted. So are ODRL property keys written as compact
or full IRIs (`odrl:permission`, `http://www.w3.org/ns/odrl/2/action`), which
is how a JSON-LD processor compacts ODRL against a context that declares the
`odrl` prefix but not the terms (EDC's management API does). A bare key wins
over its prefixed twin, on any kind of object (policy, rule, constraint,
action, party); the twin is reported, never merged. Of two prefixed spellings
of one term, the first in the document is read. `odrl:` keys are not read as
ODRL when any `@context` in the document, including a term's own (type- or
property-scoped) context or a nested node's, binds `odrl` to another IRI.
Literal values (`@value`), extension payloads and `@context` are never
rewritten. This applies to `render` and `OdrlProse` (and so the Angular
bridge); the edit layer (`read_model`, and so `OdrlProseView`) still reads
bare keys only, because its writer keeps key spelling, so a document written
with prefixed keys renders in `OdrlProse` but reads there as no rules.

Example: `{"leftOperand":"dateTime","operator":"lt","rightOperand":{"@value":"2026-12-31","@type":"xsd:date"}}`
reads as "the date and time are before the start of 2026-12-31".

"The date and time" takes a plural verb: it is read as two nouns. The other
operands stay singular, and so does an operand a profile labels itself. A date
without a time of day is a whole day, so `lt` and `gteq` bound it at its start
and `lteq` and `gt` at its end. A date without a zone leaves the zone open
(XSD reads it as a day in an unstated zone); the renderer does not pick one
and never rewrites the date. A value with a time of day is an instant and
reads as before ("the date and time are before 2026-12-31T10:00:00Z"). `eq`
and `neq` are unchanged. Only the date and time is a point in time. A time
interval, a delay period and an elapsed time are quantities: `lt` reads "is
less than", `lteq` "is at most", `gt` "is greater than" and `gteq` "is at
least", whether the right operand is a duration or a date. A profile's
operator label agrees with the subject too: "Earlier than" reads "the date
and time are earlier than". A relabelled operand stays singular, unless its
profile declares `prose:plural true` (see The `prose:` extension).
A rule sentence names its assigner by role: "as permitted by the
assigner X", "as prohibited by the assigners X and Y".

A constraint can be declared once, as a node of its own with an identifier,
and referred to by that identifier (`"constraint": [{"@id": "urn:c:1"}]`, or
the bare string), from a rule, a refinement or a logical constraint (ODRL
Information Model, Example 19). A constraint is declared by a top-level or
`@graph` node, or by any constraint object with an identifier, wherever it
sits; identifiers are compared as IRIs, so `ex:c1` finds the constraint
declared as `http://ex.com/c1` when the document binds `ex`. A reference to
a constraint the document declares reads as that constraint. A reference to
one it does not declare reads as "the constraint defined at urn:c:1 holds",
and the `Condition` carries the (expanded) IRI in `reference`; in a target's
or a party's refinement, which reads inside the rule's sentence, the `Rule`
carries it in `references`. References are followed at most 1000 times per
document, and a reading nests at most 256 levels deep, references included. The reader fetches nothing: `OdrlProse`
shows the IRI as a link only when the host lists it in `links`, the IRIs it
found to dereference (the site's catalog checks each over HTTP at build time
and lists those that answer 200).

## Profiles

An ODRL profile ([ODRL IM §3.3](https://www.w3.org/TR/odrl-model/#profile-mechanism))
declares the actions, operands and values a community adds to ODRL. Loaded,
it gives the prose that community's words; without one, nothing changes.

```rust
use prose_core::profile::{ProfileVocab, Wording};

let profile = ProfileVocab::parse(&profile_text)?;      // ProfileError says why not
let wording = Wording::new().with_profile(profile);
let reading = prose_core::render_with(policy_json, &wording)?;
// reading.document: the Document, worded with the profile's labels
// reading.glossary[i]: the terms policy i uses, with their definitions
```

What a profile changes:

- **Its own terms read with its labels.** "may fine-tune" rather than "may
  fine tune", "the number of data subjects is at least 50", "is any of
  clinical research or academic research". A left operand gets "the", an
  operator label that is not already a verb phrase gets "is", and an
  IRI-valued right operand reads as its label.
- **ODRL's own terms keep this crate's phrasing.** A profile that labels
  `odrl:distribute` "never distribute" adds a glossary entry for it, and the
  sentence still says "distribute". A profile can explain the core
  vocabulary but cannot change what a sentence says about it.
- **Terms are matched by expanded IRI**, never by local name: `ex:purpose`
  and `odrl:purpose` are two terms. A compact term is expanded with the
  policy's own `@context` first, then the profile's prefixes. Compact
  values inside a policy's `@context` (`"fineTune": "ex:fineTune"`) are
  expanded while a budget lasts, 64 KiB plus 16 times the bytes of the
  document's contexts; past it a value stays as written, so many short
  terms bound to one long namespace cannot fill the memory.
- **A glossary per policy.** `Reading::glossary[i]` lists every term of
  policy `i` that a loaded profile explains, once, in reading order: label,
  definition, usage note, what it is a kind of, what it implies, which
  profile it comes from, whether it is deprecated. `OdrlProse` shows it as
  a `<details class="prose-glossary">` after each policy (at most 30
  entries), unless `glossary={false}`.
- **Advisory warnings, only with a profile loaded.** A declared profile that
  is not loaded; a term in a loaded profile's namespace that the profile
  does not declare (a likely typo); a deprecated term. A term from another
  namespace stays silent, because a policy may mix profiles. ODRL's "MUST
  stop processing" for unknown profile terms binds an enforcing processor;
  a renderer warns instead.

A profile widens and annotates. It never closes an input: an undeclared term
still reads, still edits, and the reducer never refuses an edit because of a
profile.

### Formats

| format | read by | notes |
|---|---|---|
| JSON-LD, compacted (`{"@context": ..., "@graph": [...]}` or one node) | always | no JSON-LD processor: string prefixes, `@vocab`, `@base`, `@language` and simple term definitions (`{"@id": ..., "@type": "@id"}`) in an inline `@context` are followed; a remote, scoped or `@import` context is not, with one warning naming it |
| JSON-LD, expanded (a top-level array, like W3C's `ODRL22.json` or DPV's `.jsonld`) | always | |
| Turtle | the `turtle` cargo feature (off by default; on in the demo and in the npm package's Turtle build, `@ds-labs/prose-wasm/turtle`) | parsed by [oxttl](https://crates.io/crates/oxttl) into the same triples as JSON-LD, so a Turtle profile and its JSON-LD twin read as equal `ProfileVocab`s, in whatever order a tool wrote them |
| N-Triples | with the `turtle` feature | N-Triples is a subset of Turtle, so it is read as Turtle |
| RDF/XML, others | no | RDF/XML is refused with `ProfileError::UnsupportedFormat`; with the `turtle` feature any other non-JSON text is tried as Turtle and fails with `ProfileError::Turtle`, without it with `UnsupportedFormat` |

`ProfileVocab::parse` tells the syntax from the first characters: `{`, or
`[` followed by a JSON value, is JSON-LD; anything else is Turtle when the
feature is on. `ProfileVocab::parse_turtle` skips the guess, for a host that
has a `text/turtle` content type. A relative IRI (`<#train>`, `"@id": "#train"`)
is made absolute by the document's own `@base`, else by the base a host
passes to `ProfileVocab::parse_with_base` or `parse_turtle_with_base` (the
URL the profile was fetched from; the demo does this). Without either, the
node or value is skipped with a warning, in both syntaxes; a Turtle text
whose terms are all relative is refused as `ProfileError::Turtle`, naming
the first relative IRI. An unbound `odrl`, `rdf`, `rdfs`, `owl`,
`skos`, `dct`, `dcterms`, `xsd`, `prof` or `prose` prefix falls back to its
well-known namespace, with one warning each; an author's own binding always
wins. The W3C context `http://www.w3.org/ns/odrl.jsonld` binds the ones it
defines (`odrl`, `rdf`, `rdfs`, `owl`, `skos`, `dct`, `xsd`, and also
`vcard`, `foaf`, `schema`, `cc`) without a warning; `dcterms`, `prof` and
`prose` still fall back with one.

What is read from each term: its kinds (`rdf:type` `odrl:Action`,
`LeftOperand`, `Operator`, `RightOperand`, `ConflictTerm`, plus policy,
rule, asset and party classes by `rdfs:subClassOf`, and party functions,
asset relations and logical operands by `rdfs:subPropertyOf`); a label
(`rdfs:label`, else `skos:prefLabel`, else `dct:title`, else the humanised
local name); a definition (`skos:definition`, else `rdfs:comment`, else
`dct:description`); a usage note; `odrl:includedIn`, `skos:broader`,
`odrl:implies`, `skos:exactMatch` and `owl:deprecated`. Labels are picked by
language (`Wording::with_languages`), one preference at a time, most
preferred first: the exact tag, then the tag shortened subtag by subtag
(RFC 4647 lookup: `de-CH` reads `de`), then another region of the same
language; then `en`, then untagged, then the first. Two `ProfileVocab`s
are equal when they hold the same vocabulary as a graph: the order of
terms, of language variants and of IRI lists does not count. The
profile's own IRI, title, version and `owl:imports` are recorded; imports
are never fetched.

Errors name the likely cause and the fix, because the usual mistake is
loading the wrong document: `LooksLikeHtml` (a server's landing page),
`ContextOnly` (a document that is only a `@context`, such as the Dataspace
Protocol's `odrl-profile.jsonld`), `JsonSchema` (a JSON Schema that checks
policies, not a vocabulary), `LooksLikePolicy`, `NoTerms`, `Json`, `Turtle`,
`UnsupportedFormat`, `TooLarge` and `LimitExceeded`.

### The `prose:` extension (a ds-prose convention, not ODRL)

ODRL has no machine-readable link from a left operand to the operators and
values that suit it; profiles say it in `skos:note` prose, which this crate
never parses. A profile author who wants narrowed suggestions in the editor
can add properties to a left operand, in the namespace
`https://ds-labs-org.github.io/ds-prose-rs/profile#`:

| property | meaning |
|---|---|
| `prose:operator` | operators to offer first for this left operand |
| `prose:rightOperand` | suggested values; their `skos:broader` or `rdfs:subClassOf` descendants within the profile are offered under them, indented |
| `prose:datatype` | an xsd datatype IRI: the value slot's placeholder, and an advisory lexical check (integer, decimal, boolean, date, dateTime, duration) |
| `prose:unit` | unit suggestions |
| `prose:closed` | `true` makes the four properties above binding on this left operand, in strict validation only (see Strict validation) |
| `prose:plural` | `true` says the left operand's label is a plural noun phrase ("Time windows"), so its operators agree in the plural: "the time windows are earlier than" |

```json
{ "@context": { "odrl": "http://www.w3.org/ns/odrl/2/",
                "prose": "https://ds-labs-org.github.io/ds-prose-rs/profile#",
                "ex": "https://example.org/odrl/profile/research/" },
  "@id": "ex:researchPurpose", "@type": "odrl:LeftOperand",
  "prose:operator": [ { "@id": "odrl:eq" }, { "@id": "odrl:isAnyOf" } ],
  "prose:rightOperand": [ { "@id": "ex:academic" } ] }
```

The extension only orders and groups suggestions. Every other operator and
any free value stay available, and nothing a profile says through it is
enforced, unless the left operand also says `prose:closed true` and the host
asks for strict validation (see Strict validation). It is read on left
operands only. `prose:plural` is read on a term the profile declares outside
ODRL's namespace; ODRL's own terms keep their built-in number ("the date and
time" is plural). It is a boolean like `prose:closed`: `true` or `1`, and
anything else counts as absent. A profile without it still
fills the editor's lists with all its actions, operands, operators, values
and conflict terms, just not narrowed per operand. Other tools ignore these
triples, as RDF allows.

### Security and limits

A profile is third-party input, often fetched from a URL the user typed.

- **Bounded cost.** `ProfileLimits` (defaults): `max_bytes` 4 MiB, checked
  before any parsing; `max_terms` 20,000; `max_depth` 64, walked with an
  explicit stack, never by recursion (oxttl keeps its own stack on the
  heap); `max_values_per_key` 256; `max_expanded_bytes` 32 MiB, counted
  after prefixes and term definitions are expanded, so a short prefix bound
  to a long namespace cannot turn a small text into gigabytes (a value past
  `max_values_per_key` is not kept but still counts its subject and
  property, and `@vocab` and `@base` count once where they are set). Past a limit
  the profile is refused with `LimitExceeded`; a half-read vocabulary is
  never returned. `includedIn` and `broader` cycles end with a warning.
  At most 100 warnings are kept, then one line says more were left out.
  `ProfileVocab::parse_with` takes other limits.
- **Plain text only.** Labels, definitions and notes have HTML tags
  stripped, entities decoded, control characters and every default-ignorable
  code point (bidi controls, zero-width, tag and variation characters,
  fillers) removed, whitespace collapsed, and length capped
  (`max_label_chars` 200, `max_text_chars` 4,000). The components render
  them as text nodes.
- **Clean IRIs only.** An IRI that holds whitespace, an invisible character
  or one of `<>"{}|\^` and the backquote (none of which an IRI may hold) is
  skipped with a warning, so an unlabelled term's display label, made from
  its IRI, cannot carry a bidi override either. Warnings quote what they are
  about with invisible characters written as `\u{..}`.
- **No network access.** `prose-core`, `prose-yew` and the npm packages
  never fetch: the host fetches the profile and passes its text. Remote
  contexts and `owl:imports` are not followed. Only the demo fetches, when
  the user asks it to load a URL (CORS, no credentials, 15 s, 4 MiB).
- **No spoofing of core terms** (see above): a profile can relabel its own
  terms only.

## Strict validation

`prose_core::validate` checks a policy against the profiles a `Wording`
holds. It has two modes.

- **Advisory** (the default, and what `edit::check_profile` has always said):
  a profile widens and annotates, and nothing it says about a policy is an
  error. `check_profile` is this mode, byte for byte.
- **Strict** reads the vocabulary as closed: ODRL plus the loaded profiles is
  every term there is. A term that resolves nowhere is an error, and so is a
  term used where its kind does not belong. Strict is opt-in; it never
  changes what `render` or the editor do unless the host asks for it.

```rust
use prose_core::validate::{Options, Verdict, validate_json};

let report = validate_json(policy_json, &wording, &Options::strict());
match report.verdict {
    Verdict::Conforms => {}
    Verdict::Violations => { /* report.findings, in document order */ }
    Verdict::Incomplete => { /* report.gaps: what could not be checked */ }
    _ => {}
}
```

For many policies against the same profiles, build a `Validator` once
(`Validator::new(Arc<Wording>, Options)`): it indexes the profiles at
construction instead of on every call. `Validator::validate` takes an
`EditDoc`, `validate_json` a text, and neither panics on hostile input; a
text that is not a policy is an `input.unreadable` finding.

### The verdict

| verdict | when |
|---|---|
| `conforms` | no error and nothing left unchecked |
| `violations` | at least one error that does not rest on something unchecked |
| `incomplete` | no such error, and a gap: something could not be checked |

Incomplete is never a pass and never a guessed error. The vocabulary is
incomplete when the policy declares a `profile` that is not loaded, when a
loaded profile builds (`owl:imports`, `prof:isProfileOf`) on one that is not,
or when a node could not be checked (a locked or opaque node, a remote
`inheritFrom` parent). While the vocabulary is incomplete,
`profile.unknown-term`, `profile.wrong-kind` and `profile.not-declared` are
not reported: a missing profile must not turn every term into a false error.
Nothing is fetched; declared and imported profiles are the host's to load.
With no profile at all, strict is meaningful: the vocabulary is ODRL alone,
`ex:x` is an error, and a policy that declares a profile is incomplete.

Which ODRL terms count as defined without a profile is `Options::odrl_vocabulary`:
the Core plus the Common Vocabulary (the default), or the Core only.
`profile::odrl_terms()` is the table of names and kinds (no definitions).

### Rules

Every finding names its rule. Ids are frozen once released. Each rule
carries its tier and the authority behind it (`RuleInfo`, `RULES`), so a
reader can tell what the standard requires from what is a ds-prose
convention:

- **standard**: the ODRL Information Model (IM) states it as a requirement;
- **standard-derived**: the IM states a requirement about vocabulary, and the
  rule is its reading under a closed world, which the IM does not itself call
  for;
- **convention**: a ds-prose convention, not a standard;
- **input**: about the input or the reader, not the policy;
- **heuristic**: a likely mistake no standard rules out; opt-in only.

What the IM says that these rules lean on, quoted from the W3C Recommendation: "An ODRL Policy must either: Only use
terms defined in the ODRL Core Vocabulary, or Use an ODRL Profile that
declares the supported vocabulary used by expressions in the Policy", and in
the latter case the `profile` property MUST indicate the profile's IRI
(IM §2.1). A
processor MUST understand the profile a policy identifies, and MUST stop if it
does not recognise it (IM §3.2). From these, the `profile.*` rules follow
only once the vocabulary is taken as closed.

Levels read advisory, then strict. "Closed" is a left operand a profile
marks `prose:closed` (below). "Refusable" means the editor may refuse an edit
that makes this finding, and only under `StrictEdit::RefuseTerms`.

| id | what | levels | basis | refusable |
|---|---|---|---|---|
| <a id="rule-profile-not-loaded"></a>`profile.not-loaded` | a declared `profile` is not loaded (ODRL core always counts as loaded); a gap | warning, error | standard-derived | no |
| <a id="rule-profile-import-not-loaded"></a>`profile.import-not-loaded` | a loaded profile builds on one that is not loaded; a gap | off, warning | standard-derived | no |
| <a id="rule-profile-undeclared-in-namespace"></a>`profile.undeclared-in-namespace` | a term in a loaded profile's namespace that no loaded profile declares | warning, error | standard-derived | yes |
| <a id="rule-profile-unknown-term"></a>`profile.unknown-term` | an action, left operand, operator, unit, conflict value or policy type that neither ODRL nor a loaded profile defines | off, error | standard-derived | yes |
| <a id="rule-profile-wrong-kind"></a>`profile.wrong-kind` | a term used where its kind does not belong, such as a left operand written as an action | off, error | standard-derived | yes |
| <a id="rule-profile-not-declared"></a>`profile.not-declared` | a term from a loaded profile the policy does not list in `profile` | off, error | standard-derived | no (a fix is offered) |
| <a id="rule-profile-conflict-term"></a>`profile.conflict-term` | a conflict value that is not `perm`, `prohibit`, `invalid` or a conflict term of a loaded profile (the IM says SHOULD) | off, error | standard-derived | yes |
| <a id="rule-profile-unbound-prefix"></a>`profile.unbound-prefix` | a compact IRI that only a profile's own prefixes expand; a JSON-LD processor would read another IRI | off, error | standard-derived | yes |
| <a id="rule-profile-deprecated-term"></a>`profile.deprecated-term` | a term the profile marks `owl:deprecated`; still defined, so a warning (an error with `deny_deprecated`) | warning, warning | convention | no |
| <a id="rule-odrl-policy-uid"></a>`odrl.policy-uid` | a policy without a uid (IM §2.1) | off, error | standard | no |
| <a id="rule-odrl-no-rule"></a>`odrl.no-rule` | a policy without a permission, prohibition or obligation (IM §2.1) | off, error | standard | no |
| <a id="rule-odrl-offer-assigner"></a>`odrl.offer-assigner` | an Offer rule without an assigner (IM §2.1.2, §2.7.1) | off, error | standard | no |
| <a id="rule-odrl-agreement-parties"></a>`odrl.agreement-parties` | an Agreement rule without an assigner or an assignee (IM §2.1.3, §2.7.1) | off, error | standard | no |
| <a id="rule-odrl-rule-target"></a>`odrl.rule-target` | a permission or prohibition without a target (IM §2.6.1, §2.6.2, §2.7.1) | off, error | standard | no |
| <a id="rule-odrl-rule-action"></a>`odrl.rule-action` | a rule without an action (IM §2.6, §2.7.1) | off, error | standard | no |
| <a id="rule-odrl-constraint-shape"></a>`odrl.constraint-shape` | a constraint without a left operand, an operator, or exactly one of `rightOperand` and `rightOperandReference` (IM §2.5.1), or a logical constraint with several of `and`, `or`, `xone`, `andSequence` (IM §2.5.2) | off, error | standard | no |
| <a id="rule-odrl-set-operator-cardinality"></a>`odrl.set-operator-cardinality` | several right operand values with a comparison operator (eq, neq, gt, gteq, lt, lteq); isA, hasPart, isPartOf, isAnyOf, isAllOf and isNoneOf are set-based in ODRL22.ttl and take a list (IM §2.5.1) | off, error | standard | no |
| <a id="rule-odrl-consequence-nesting"></a>`odrl.consequence-nesting` | a consequence on a consequence, or a remedy that has a consequence (IM §2.6.3, §2.6.7) | off, error | standard | no |
| <a id="rule-odrl-inheritance-cycle"></a>`odrl.inheritance-cycle` | policies in the document that inherit from each other in a circle (IM §2.9) | off, error | standard | no |
| <a id="rule-prose-operator-not-allowed"></a>`prose.operator-not-allowed` | an operator outside the left operand's `prose:operator` | info, info; closed: error | convention | closed only |
| <a id="rule-prose-value-not-allowed"></a>`prose.value-not-allowed` | an IRI value outside `prose:rightOperand` and the values narrower than it | off, info; closed: error | convention | no |
| <a id="rule-prose-iri-expected"></a>`prose.iri-expected` | a plain string that names an allowed value where an IRI is expected | off, off; closed: error | convention | no (a fix is offered) |
| <a id="rule-prose-unit-not-allowed"></a>`prose.unit-not-allowed` | a unit outside `prose:unit` | off, info; closed: error | convention | closed only |
| <a id="rule-prose-datatype-lexical"></a>`prose.datatype-lexical` | a literal outside the lexical space of `prose:datatype` | info, info; closed: error | convention | no |
| <a id="rule-prose-datatype-conflict"></a>`prose.datatype-conflict` | a literal typed with another datatype than `prose:datatype` | off, warning; closed: error | convention | no |
| <a id="rule-prose-ill-typed-literal"></a>`prose.ill-typed-literal` | a typed literal outside its own datatype's lexical space (XSD 1.1 Part 2; RDF 1.1 Concepts §3.3) | off, error | convention | no |
| <a id="rule-input-unreadable"></a>`input.unreadable` | the input is not a policy; always an error | error, error | input | no |
| <a id="rule-input-unchecked"></a>`input.unchecked` | something the validator could not check; a gap (an error with `fail_closed`) | off, warning | input | no |
| <a id="rule-input-reader-note"></a>`input.reader-note` | a note the reader made while reading | off, info | input | no |
| <a id="rule-heuristic-ordered-on-unordered"></a>`heuristic.ordered-on-unordered` | `gt`, `gteq`, `lt` or `lteq` on an IRI or boolean value; the IM states no requirement | off, off | heuristic | no |

Tiers. Strict turns on `profile.*`, `prose.*` and `input.*`. The `odrl.*`
tier (the IM's own structural requirements) is **off by default**, even in
strict, and `Options::structural(true)` switches it on: a Dataspace Protocol
catalog offer is often written without the assigner or target the IM asks
for, so the tier must not block a profile user by default. The heuristic rule
is switched on only with `Options::level`. `Options::level(rule, level)`
sets any rule's level, over everything else (`input.unreadable` stays an
error); `fail_closed(true)` makes `input.unchecked` an error and
`deny_deprecated(true)` does the same for `profile.deprecated-term`.

The severity of a rule is resolved in this order: a per-rule level; off if
the rule is newer than the pinned ruleset; off if it is an `odrl.*` rule and
`structural` is not set; the catalogue's level for the mode; for `prose.*`,
the closed column when the left operand is closed.

**Ruleset pinning.** `RULESET` is the ruleset this build implements (1), and
each rule records `since_ruleset`. `Options::ruleset(n)` checks only the rules
of ruleset `n` and earlier, so a CI that pins `n` does not turn red when a
later release adds a rule to a policy that did not change. Rule ids are never
renamed or reused.

### `prose:closed`

The `prose:` hints keep their meaning, "suggested, never exclusive", in strict
mode too. A left operand becomes closed only through an explicit marker in the
`prose:` namespace, `prose:closed true`, one per left operand
(`OperandHints::closed`):

```json
{ "@id": "ex:researchPurpose", "@type": "odrl:LeftOperand",
  "prose:closed": true,
  "prose:operator": [ { "@id": "odrl:eq" } ],
  "prose:rightOperand": [ { "@id": "ex:academic" } ] }
```

On a closed operand the operator, values (with their narrower values), unit
and datatype the profile names are binding, and the `prose.*` rules above
are errors there. On an operand that is not closed, the same rules stay at
the informational level they have in advisory mode. This is a ds-prose
convention. ODRL has no such marker, and other tools ignore the triple.

### The report

`Report::to_json` is `ds-prose/validation-report@1`, the same bytes from the
Rust crates, the wasm package, the Angular wrapper and the command line. It is
deterministic: no timestamps, findings in document order, keys in a fixed
order. A change to `@1` may only add keys; removing or renaming one is `@2`.

```json
{ "schema": "ds-prose/validation-report@1",
  "tool": {"name": "ds-prose-rs", "version": "0.4.0"},
  "mode": "strict",
  "tiers": ["profile", "prose", "input"],
  "ruleset": 1,
  "verdict": "violations",
  "vocabulary": {"odrl": "common",
                 "profiles": [{"iri": "...", "title": "...", "version": "..."}]},
  "gaps": [{"rule": "profile.not-loaded", "detail": "..."}],
  "counts": {"error": 1, "warning": 0, "info": 0},
  "findings": [{
    "rule": "profile.unknown-term",
    "tier": "profile",
    "basis": "standard-derived",
    "severity": "error",
    "message": "...",
    "target": {"kind": "slot", "path": "policy[0].permission[0].action[0]#name"},
    "pointer": null,
    "term": {"written": "foo:bar", "iri": "https://.../bar"},
    "profile": null,
    "expected": {"kinds": ["action"], "operators": [], "values": [], "units": [],
                 "datatype": null, "closed": false},
    "fixes": [{"label": "...", "safe": true, "replacement": "..."}],
    "fingerprint": "16 hex digits"
  }] }
```

`expected.values` lists at most 8 values so a report grows with its findings;
when the profile allows more, `expected.valuesTotal` (an added key, absent
otherwise) gives how many. `gaps` lists each distinct gap once.

- `verdict` is `conforms`, `violations` or `incomplete`; `gaps` lists the
  findings that make a verdict incomplete, and they are also in `findings`.
- `target.kind` is `doc`, `node`, `slot` or `list`; `path` is the edit layer's
  path (empty for `doc`). `pointer` is reserved for an RFC 6901 pointer into
  the input and is `null` today.
- `fingerprint` is FNV-1a 64 over the rule id, the path and the term as
  written (the message when there is no term). Reformatting the file or
  reordering its keys does not change it, so it works as a baseline key.
- `fixes[].replacement` is a compact IRI only when the policy binds the
  prefix, otherwise the full IRI. In Rust a `Fix` also carries its
  `EditEvent`s, with `expect` ids, so a fix applied to a document that moved
  on fails as stale.
- `RuleInfo` and `RULES` (and `rules()` in the wasm package) give the same
  catalogue as data, for a UI that lists the rules.

### Command line

`@ds-labs/prose-wasm` installs `ds-prose-validate`, a Node bin that runs the
same engine (the wasm module), for CI:

```bash
npx ds-prose-validate --strict --profile research.ttl policy.json
npx ds-prose-validate --strict --structural --format json --profile p.jsonld a.json b.json
```

Options: `--profile` (repeatable), `--strict`, `--structural`,
`--fail-closed`, `--deny-deprecated`, `--odrl common|core`, `--ruleset <n>`,
`--rule <id>=<off|info|warning|error>` (repeatable), `--format text|json`,
`--allow-incomplete`. `--structural`, `--fail-closed`,
`--deny-deprecated` and `--odrl` need `--strict` (without it the command
exits 2 as a usage error). A policy of `-` is read from standard input (once).
Input must be UTF-8 (a leading byte order mark is ignored). Without
`--strict` it is the advisory reading, where nothing is an error unless
`--rule` or `--ruleset` says so: both work with or without `--strict`. In the
text output, control characters from a policy are shown as escapes. An
uncaught fault of the tool exits 2, never 1.

| exit code | meaning |
|---|---|
| 0 | every policy conforms |
| 1 | at least one policy has violations |
| 2 | usage error, or a policy or profile that cannot be read |
| 3 | no violations, but incomplete |

`--allow-incomplete` maps 3 to 0. When several apply, 2 outranks 1, and 1
outranks 3. A profile that does not parse is exit 2, never a finding.
`--format json` prints an array with a `{file, report, error}` per policy;
each report is the validator's own text, unchanged. There is no native
binary; the command is a Node script.

### In the editor

`EditConfig::with_strict(options)` makes the editor validate the document
(`EditConfig::validator`, a `Validator`) and marks each finding in place, with
a problems list; the closed comboboxes offer only the terms the validator
admits, and `rules.defaults` is rewritten so an add button creates an
admissible node. That rewrite lives in `cfg.rules`, and the reducer applies
the rules it is handed, not the config's: after `with_strict`, give
`use_prose_editor_with` (or `use_prose_editor`) `Rc::new(cfg.rules.clone())`,
and `cfg.validator.clone()` for the validator. A host that keeps building its
own `EditRules::default()`, as in the 0.3 pattern, gets the defaults it set,
which are admissible only while they are ODRL terms.

```rust
let cfg = Rc::new(EditConfig::default().with_wording(wording).with_strict(Options::strict()));
let editor = use_prose_editor_with(
    || read_model(POLICY).unwrap_or_default(),
    Rc::new(cfg.rules.clone()),
    cfg.validator.clone(),
    StrictEdit::Flag,
);
html! {
    <OdrlProseView doc={editor.editor.doc.clone()} mode={EditMode::Edit} config={cfg}
                   onedit={editor.editor.onedit.clone()}
                   refusal={editor.refused.as_ref().and_then(|(ev, err)| err.issue(ev))} />
}
```

The policy is the reducer's. By default (`StrictEdit::Flag`) it commits every
edit the rules allow, and the validator marks a violation as an error. The
view is the same under both: a closed combobox (a vocabulary slot, or a value
slot of a closed left operand) takes only what the validator admits. Enter on
text that names nothing admissible keeps the draft in the slot, with the
reason and the nearest term; Escape reverts it; and when focus leaves the
slot the draft is given up, the slot goes back to its value and a polite live
region says so. So text typed into a closed combobox can be lost, in `Flag`
mode too. The empty text and the text of any other slot commit as usual.
`StrictEdit::RefuseTerms` is opt-in: through
`EditDoc::apply_gated(ev, rules, Some(&validator))` and
`use_prose_editor_with`, the reducer refuses, as `EditError::OutsideProfile`,
a term written into a vocabulary slot that a refusable rule reports as an
error there. The stock view itself never sends such a term: a vocabulary
slot asks the validator before it commits, with suggestions on or off and
however large the document, and does not send it. A refusal reaches only a
host that bypasses the view, and it is dropped when the validator or the
policy changes. Refusal is slot-local (it looks at the written value only, never
at what depends on it), never applies to the empty string, to free text, to
right operands, to structural events or to undo, and a document that already
violates loads and stays editable. What certifies a policy is
`Report::conforms()`, not the refusals; the demo gates its export on it.

### In the browser and in Angular

`@ds-labs/prose-wasm` exports `validate(policy, profiles, options?)`,
`rules()` and `ProseValidator`, returning the report as JSON text; its
`ProseHandle` gets `setValidation` and `report()`. `<ds-odrl-prose>` has the
`strict`, `validation` and `showViolations` inputs, the `validationChange`
and `validationError` outputs and a `report` signal, and
`ProseValidatorService` validates without rendering. See the package READMEs.

## Edit mode

`prose_core::edit` turns the read-only reading into something an editor can
drive. It knows ODRL and nothing about any particular engine or wire format.

- `EditDoc`: policies as trees of numbered nodes (`PolicyNode`, `RuleNode`,
  `Entity`, `ActionNode`, `ConstraintNode`). Ids are stable across edits.
  Values are kept raw (`odrl:isAnyOf`, never humanised), because that is what
  a consumer compares.
- Paths: `NodePath`, `SlotPath` and `ListPath` address a node, one editable
  piece of text, or a list, with `Display` and `FromStr`. The segment names are
  ODRL's, for example `policy[0].permission[1].duty[0].consequence[0]#rightOperand[2]`.
- `EditEvent` and `EditDoc::apply`: set text, set a choice, set an IRI (a
  right-operand value picked as `{"@id": ...}`), add, remove, move,
  change a rule's kind, wrap and unwrap a constraint. Each event names the id it
  expects to find, so a stale event is refused rather than misapplied. For
  `Add` that id is the owner of the list (`EditDoc::list_owner_id` gives it),
  and `expect` is `None` exactly for the top-level policies list, which has no
  owner node; a missing or surplus expectation is `EditError::Invalid`. An
  `Add` therefore catches an owner that was removed, moved or replaced, but not
  a sibling inserted or removed in the same list, because items are addressed
  by index. `apply` is atomic. `focus_after` says where keyboard focus belongs
  afterwards.
- `EditRules`: how many items each list takes (`Limit`), which operators take
  several values, what a new node starts with. The reducer enforces it and an
  editor reads it, so an editor cannot offer what the reducer refuses.
- `read_model` and `write_jsonld`: JSON-LD in and out. Every node remembers the
  JSON it was read from (`Origin`) and the writer merges into it property by
  property. What you did not edit is written back as it was: key order, key
  spelling, unknown keys, `@list` wrappers. `set_property` is the same merge for
  a host that builds the model from its own encoding.
- `sentences`: the edit templates, as segments (text, editable slots, choices,
  lists to add to) rather than finished strings.
- With a profile: `sentences_with(doc, &wording)` puts the profile's labels in
  each slot's `display` (its `raw` value is unchanged, so what is written back
  is too); `check_profile(doc, &wording)` returns advisory `Issue`s (an
  undeclared term in a loaded profile's namespace, a deprecated term, a
  declared profile that is not loaded (the first 20 by name, then one issue
  counting the rest), an operator outside the `prose:`
  suggestions, a value that fails the `prose:datatype`); `left_operand_at`
  says which left operand a right-operand, unit or reference slot belongs
  to; `prefixes(doc)` reads the prefixes the document binds. The reducer
  and `EditRules` do not look at the profile.

### Compatibility

The enums that grow as the editor learns more of ODRL are `#[non_exhaustive]`,
so adding a variant is a minor release, not a breaking one. In `prose-core`:
`EditEvent`, `EditError`, `NewItem`, `FocusTarget`, `ListKind`, `Field`, `Step`,
`NodeRef`, `Segment`, `SlotKind`, `ChoiceKind`, `EmptyText`, `IssueTarget`,
`profile::TermKind`, `profile::ProfileError`, `StrictEdit`, `wording::ValueShape` (also `edit::ValueShape`) and, in `validate`, `Mode`, `Verdict`, `Tier`, `Basis`, `RuleId` and `RuleInfo`. In `prose-yew`: `FocusKind`
and `DecorationAt`. Outside the defining crate a `match` on one of them needs
a wildcard arm with a sensible fallback. Units,
`rightOperandReference` and `partOf` can be edited but not added today (see
Limits); adding them is the kind of change this allows.

Left exhaustive because they are closed by nature: `Conj`, `Severity`,
`EditMode`, `ButtonContent`, `Reveal`, `LogicalOp`, `RuleList`, `EntityRole`
and `DocShape` (ODRL or this crate fixes their members), and the data enums
`ConstraintNode`, `RightOperand` and `Literal`, which already carry an
`Opaque` or raw fallback for what they do not understand.

The profile types can grow too: `ProfileVocab`, `Wording`, `Prefixes` and
`LangText` are opaque (private fields, methods only), and `Term`,
`OperandHints`, `ProfileSummary`, `ProfileLimits`, `Reading`,
`PolicyGlossary` and `Gloss` are `#[non_exhaustive]` structs: read their
fields, and start from `ProfileLimits::default()` to change a limit. So is
`edit::ChoiceSlot`: build one with `ChoiceSlot::new` and set its fields.

Changed after 0.4.0 (wording): the rendered text changes, on purpose. "the
date and time is before X" reads "are before X"; a date-only right operand
reads "are before the start of X" (see above); a rule sentence reads "as
permitted by the assigner X". The v0.1 goldens, the profile goldens, the SSR
snapshots and the site example change with it. `wording::operator_for` and
`wording::operator_with` now agree in number with the left operand ("are
before" for `dateTime`); `wording::operator_for_date` and
`wording::operator_with_date` are new. A duration is not an instant:
`timeInterval` with `lt` read "is before" since v0.1 and now reads "is less
than", as do `delayPeriod` and the other ordering operators (`lteq` "is at
most", `gt` "is greater than", `gteq` "is at least"). A profile operator
label agrees with a plural subject ("the date and time are earlier than").
`ChoiceSlot` is now `#[non_exhaustive]` and gained `value_shape` (a
`ValueShape`); build one with `ChoiceSlot::new` and set the fields. `ValueShape::of` is new.
`wording::operator_phrase(&OperatorQuery)` is the one entry point for
operator wording: `OperatorQuery::new(left, op)` with `.value(ValueShape)` and
`.wording(&Wording, &Prefixes)`. `wording::operator_for`, `operator_for_date`,
`operator_with` and `operator_with_date` are deprecated since 0.5.0 and give
the same text. (The entry point is not called `operator`, which is already
the phrase for a term alone.) `OperandHints::plural` is new (`prose:plural`).

Changed after 0.4.0 (additive except for struct literals): `Condition`
gained `reference` (the IRI of a constraint referred to and not declared),
`Rule` gained `references` (the same, for the refinements of its target and
parties) and `OdrlProseProps` gained `links`; add `reference: None`,
`references: vec![]` and `links: Default::default()` to a literal. `html!`
and `yew::props!` are unaffected. The v0.1 and profile goldens print the
new fields (`reference: None`, `references: []`); their text is unchanged.

Changed in 0.4 (profiles; additive except for struct literals):

- New: the `profile` module, `render_with` and `render_value_with` (they
  return a `Reading`), `wording::gloss` and `wording::operator_with`, and in
  `edit` `sentences_with`, `check_profile`, `left_operand_at`, `prefixes`
  and the `EditEvent::SetIri` variant. In `prose-yew`: `SharedWording`, `ProseGlossary`,
  `Vocabulary::with_profiles` and `EditConfig::with_wording`. In the npm
  packages: `ProseHandle.setProfile`, and on `<ds-odrl-prose>` the `profile`
  input and the `profileLoaded` and `profileError` outputs.
- New, strict validation (opt-in; advisory output is unchanged): in
  `prose-core`, the `validate` module (`Options`, `Validator`, `validate`,
  `validate_json`, `Report`, `Finding`, `RULES`), `profile::odrl_terms`,
  `OperandHints::closed`, `EditDoc::apply_gated`, `StrictEdit` and
  `EditError::OutsideProfile`. In `prose-yew`: `EditConfig::with_strict`,
  `use_prose_editor_with`, `use_validation`, `OdrlProseProps::validator` and
  `violations`. In the npm packages: `validate`, `rules`, `ProseValidator`,
  `setValidation`, `report()` and `setProfiles`; on `<ds-odrl-prose>` the
  `strict`, `validation` and `showViolations` inputs, the `validationChange`
  and `validationError` outputs, the `report` signal and
  `ProseValidatorService`; and the `ds-prose-validate` command.
- **`EditConfig.validator` is a breaking change for a struct literal.**
  `EditConfig` gained the field `validator: Option<Validator>`, so a literal
  of it needs `validator: None`. Code that builds it with
  `..Default::default()` or with the `with_*` methods is unaffected. This
  rides the 0.4 release, with `wording`. `Labels` also gained the strict
  fields (`not_in_profile`, `kept_previous`, `kept_empty`, `allowed_group`,
  `not_allowed_group`, `not_allowed_option`, `not_allowed_term`,
  `did_you_mean`, `problems_heading`, `go_to`, `status_conforms`,
  `status_violations`, `status_incomplete` and `counts`), with the same
  remedy. So did two props: `OdrlProseProps` gained `validator` and
  `violations` (add `validator: None, violations: false` to a literal), and
  `OdrlProseViewProps` gained `refusal` (add `refusal: None`); `html!` and
  `yew::props!` are unaffected. Under strict validation the reducer applies the
  `EditRules` it is handed, so a host passes `cfg.rules` after `with_strict`
  (see In the editor).
- **A new error variant.** `EditError::OutsideProfile` is only produced by
  `apply_gated` under `StrictEdit::RefuseTerms`; `EditError` is
  `#[non_exhaustive]`, so a `match` with a wildcard arm is unaffected.
  `EditRules` and `EditDoc::apply` are unchanged.
- **No profile reads exactly as before.** `render`, `render_value` and
  `sentences` are `render_with`, `render_value_with` and `sentences_with`
  with `Wording::default()`; the v0.1 goldens and the SSR snapshots are
  byte-identical.
- Struct literals need the new fields: `OdrlProseProps` gained `wording` and
  `glossary`; `EditConfig` gained `wording`; `Suggestion` gained `group`
  (`Suggestion::new` sets `None`); `Labels` gained `group_core`,
  `group_profile`, `glossary_heading`, `kind_of`, `implies`, `usage_note`,
  `from_profile`, `deprecated`, `suggested_operators`, `other_operators`,
  `profile_values` and `datatype_placeholders`. Code that builds the props
  with `html!`, or `EditConfig` and `Labels` with `..Default::default()`, is
  unaffected. `Suggestion` has no `Default`: build it with
  `Suggestion::new`, or add `group: None` to the literal.
- `prose-core` has an optional `turtle` feature (oxttl and oxrdf; on
  wasm32 also getrandom with `js`). It is off by default, so a host that
  does not ask for it gets no new dependency. `prose-wasm` and the demo turn
  it on.
- The profile reaches Yew components as a prop (`OdrlProseProps::wording`,
  `EditConfig::wording`), not as a Yew context: a prop is explicit, keys
  the memo, and renders the same under SSR. A context was considered and
  left out.

Changed in 0.3 (breaking, after the tagged 0.2.0; a consumer pinned to the
`v0.2.0` tag is unaffected until it moves the pin):

- `EditEvent::Add` gained the `expect: Option<NodeId>` field described above.
  Build one with the id from `EditDoc::list_owner_id`.
- The enums listed above are now `#[non_exhaustive]`: a `match` on one needs a
  wildcard arm.
- `ChoiceSlot` gained the pub field `values: usize` (the number of right
  operands of the constraint an operator select belongs to), so a struct
  literal of it needs the field. Likewise `Labels` gained
  `follow_up_remedy_elsewhere` and `follow_up_consequence_elsewhere`, and
  `EditConfig` gained `host_plans_operator_switches`; code that builds them
  with `..Default::default()` is unaffected.
- The operator select offers only the switches the reducer accepts, unless
  `EditConfig::host_plans_operator_switches` is set (see `operator_switch_allowed`).
- A structural event's pending focus move and announcement expire at the
  user's next input or after 500 ms, whichever comes first, so a host that
  applies the event within a slow, multi-task render keeps them, and a
  refused event cannot fire later on an unrelated change of the document. The
  listeners behind this are removed as soon as the event is answered or
  expires, and when the view unmounts.
- Editing or removing a right-operand value keeps the `@language`,
  `@index` and other keys of the value objects around it, and a value the model
  holds as a string is written as a string. Moving policies keeps empty
  `@graph` wrappers, and nested `@graph` wrappers inside a top-level `@graph`
  object keep their `@context`.

### Using it from Yew

`prose-yew` renders the model through `OdrlProseView`. The component is
controlled: it never changes the document, it reports every edit as an
`EditEvent` and the host applies it. `use_prose_editor` is a host that does so
for you, with an undo history.

```rust
use std::rc::Rc;
use prose_yew::prose_core::edit::{EditRules, read_model};
use prose_yew::{EditConfig, EditMode, OdrlProseView, use_prose_editor};

#[function_component(Editor)]
fn editor() -> Html {
    let rules = use_memo((), |_| EditRules::default());
    let config = use_memo((), |_| EditConfig::default());
    let editor = use_prose_editor(|| read_model(POLICY).unwrap_or_default(), rules);
    html! {
        <OdrlProseView doc={editor.doc.clone()} mode={EditMode::Edit}
                       config={config} onedit={editor.onedit.clone()} />
    }
}
```

Props of `OdrlProseView`:

| prop | what it does |
|---|---|
| `doc: Rc<EditDoc>` | the document; the host owns it |
| `mode: EditMode` | `Read` (same sentences, no controls) or `Edit` |
| `config: Rc<EditConfig>` | everything below |
| `onedit: Callback<EditEvent>` | every committed edit and structural action; not applied by the component. Apply it before the user's next input and within 500 ms (synchronously, as `use_prose_editor` does; a slow render that spans several browser tasks is fine): a structural event whose result has not arrived by then is treated as refused, so no focus moves and nothing is announced, even if the document changes later for another reason. A host that validates asynchronously should apply the event when it is done, and expect to lose focus management and the announcement for it |
| `on_focus_slot: Callback<Option<SlotFocus>>` | `Some` when a slot or select gets focus, `None` when it loses it, for help text |
| `issues: Rc<Vec<Issue>>` | findings the host computed, shown inline at their targets (`aria-invalid` plus the message for errors) |
| `decorate: Option<Callback<Decoration, Html>>` | host badges after each policy heading, rule sentence and condition |
| `class: Classes` | extra classes on the root |

`EditConfig` (default: the ODRL JSON-LD preset):

- `rules: EditRules`: limits per list, so a host that needs "exactly one
  action per rule" sets `rule_action = Limit::exactly(1)`. A control the rules
  do not allow is not rendered, and the reducer refuses the event anyway.
- `labels`: every word the editor writes, including nouns, field names,
  placeholders and the accessible-name templates (`{noun}`, `{n}`, `{field}`,
  `{owner}`, `{action}`, `{phrase}`, `{raw}`).
- `classes: ProseClasses`, `styles: ProseStyles`: one entry per element kind,
  added next to the built-in `prose-*` class and as an inline style, so a host
  can put its design system's classes (for example a PatternFly button) on the
  `+` and `-` buttons without forking the crate.
- `vocab: Vocabulary`: suggestions for actions, left operands, policy types,
  parties and assets, and the closed lists of operators and conflict strategies.
- `add_button`, `remove_button`: `Symbol`, `Text` or `SymbolAndText`. The
  accessible name is always the specific one ("Remove condition 2 of
  permission 1 of policy 1").
- `reveal`: `Always`, or `OnHoverOrFocus` to dim the controls until needed.
- `base_css`: Edit mode ships `BASE_CSS` in a `<style>` element; turn it off to
  bring your own. Server rendering keeps `BASE_CSS` as it is with Yew 0.23.
- `wording: Rc<Wording>`: the loaded profiles. Set it with
  `EditConfig::with_wording`, which also adds the profiles' terms to `vocab`.
- `show_raw_terms`, `suggestions`, `max_suggestions`.

### Profiles in the components

`<OdrlProse>` takes the profile as a prop:

| prop | what it does |
|---|---|
| `json: AttrValue` | the policy as JSON-LD text |
| `class: Classes` | extra classes on the root |
| `wording: Option<SharedWording>` | the loaded profiles; `None` reads as before. `SharedWording` compares by pointer, so the component renders again only when the host hands it another one |
| `glossary: bool` | the glossary after each policy (default `true`) |

```rust
let wording = SharedWording(Rc::new(Wording::new().with_profile(profile)));
html! { <OdrlProse json={policy_json} wording={Some(wording)} /> }
```

`<OdrlProseView>` reads it from `EditConfig`:

```rust
let config = use_memo(wording_ptr, |_| EditConfig::default().with_wording(wording.clone()));
let issues = use_memo((doc_ptr, wording_ptr), |_| check_profile(&doc, &wording));
```

With a profile, in Edit mode:

- The action, left operand and policy type suggestions list the profile's
  terms first, under "From profile: {title}", ordered along `includedIn`
  and without deprecated terms, each with its definition as the hint; the
  built-in terms follow under "ODRL core". Typing also matches the hint
  text, ranked last, so typing "model" finds an action defined as training
  a model.
- A right-operand slot suggests the `prose:rightOperand` values of its left
  operand (with their narrower values indented), or else every profile
  right operand under "Profile values (not specific to this operand)",
  unless the left operand's `prose:datatype` is a literal type (an integer,
  a duration) that those IRIs cannot fit. The slot stays free text. Its placeholder comes from `prose:datatype`
  (`Labels::datatype_placeholders`), and a unit slot suggests
  `prose:unit`.
- The operator select adds the profile's operators. When the left operand
  names `prose:operator`s, the options split into two `<optgroup>`s,
  "Suggested by the profile" and "Other operators" (left out when nothing
  is left for it); nothing is hidden, and a current value outside the list
  stays selectable. A value written another way than its option (in full,
  or as a CURIE the profile resolves) selects that option. The conflict
  select adds the profile's conflict terms.
- A picked profile term is written as a compact IRI only when the policy's
  own `@context` binds its namespace, else as the full IRI; a picked
  right-operand value is written as `{"@id": ...}` (`EditEvent::SetIri`),
  whatever the slot held before. No prefix is invented.
- In Read mode, a profile term carries `title="{label}: {definition}
  ({raw})"` and `data-iri`.

### How editing behaves

- Free text is a `<span contenteditable="plaintext-only">` (falling back to
  `"true"` where the browser lacks `plaintext-only`) showing the raw value the
  consumer compares. It commits on blur or Enter, Escape reverts, and Enter
  never inserts a line break. Paste is inserted as plain text.
- Text is never trimmed. A leading, trailing or non-breaking space is kept
  (non-breaking spaces typed in a slot become plain spaces) and marked
  visibly.
- A slot is remounted after every commit or revert; that keeps the DOM and the
  document in step even when the host refuses the edit. The one place the
  component reads the DOM is a slot being typed in: if the host changes that
  slot's value meanwhile, the slot keeps the user's text instead of the new
  value, and the text is committed on blur, overwriting the host's change.
- The operator `<select>` hides the operators the reducer would refuse (a
  single-value operator on a constraint holding several values, from a set
  operator). `Labels` has `follow_up_remedy_elsewhere` and
  `follow_up_consequence_elsewhere` for a remedy outside a prohibition and a
  consequence outside a duty.
- Operator, logical operator, conflict strategy and a top-level rule's kind are
  `<select>`s. A current value that is not among the options stays selectable.
- Vocabulary suggestions are an ARIA listbox: type to filter, arrow keys and
  Enter to pick, Alt+ArrowDown to open.
- After an add, remove, move, regroup or kind change, focus moves where
  `focus_after` says and a polite live region announces the change.

### Markup

`<article class="ds-prose prose-view prose-edit">` holds `section.prose-policy`
elements with `ol.prose-rules` of `li.prose-rule` (plus `prose-permission`,
`prose-prohibition` or `prose-obligation`), `p.prose-rule-sentence`, and nested
`.prose-conditions` lists. Every policy, rule and condition container carries
`data-node="<NodePath>"`; slots carry `data-slot="<SlotPath>"`; add buttons
`data-add="<ListPath>"`. Further classes: `prose-term`, `prose-slot`
(`-empty`, `-invalid`, `-warning`, `-whitespace`), `prose-inherited`,
`prose-select`, `prose-list`, `prose-item`, `prose-add`, `prose-remove`,
`prose-reorder`, `prose-wrap`, `prose-toolbar`, `prose-add-bar`,
`prose-label`, `prose-condition`, `prose-logical`, `prose-follow-up`,
`prose-locked`, `prose-issue` (`-error`, `-warning`, `-info`),
`prose-warnings`, `prose-decoration`, `prose-suggestions`, `prose-suggestion`,
`prose-live`, `prose-empty`.

### Verification

`prose-yew/tests/ssr.rs` renders the component on the host (snapshots of the
v0.1 `OdrlProse` output, and the markup contract of both modes).
`prose-yew/tests/edit_dom.rs` mounts it in headless Chrome and drives it with
synthetic events (commit, Enter, Escape, paste, composition, + and -, focus,
suggestions, selects):

```bash
CHROMEDRIVER=$(command -v chromedriver) \
  cargo test -p prose-yew --target wasm32-unknown-unknown --features csr --test edit_dom
```

Only Chromium is tested. Synthetic events do not insert text, so real typing,
IME composition and clipboard permissions need a manual check; Firefox without
`plaintext-only` relies on the paste, drop and Enter handlers that are tested
only through the `"plaintext-only"` path.

## Limits

- **Edit templates differ from `render`.** The edit layer shows every slot, so
  it never elides an empty one, and its wording is not identical to
  `render`'s. `render` and `OdrlProse` are unchanged and golden-tested.
- **No JSON-LD processing on write.** The writer merges into the JSON it read;
  it does not expand or compact, and it does not follow a custom `@context`.
- **Some things can be edited but not added:** units, `rightOperandReference`
  and `partOf`. A constraint with several logical keys, a rule that is not an
  object or IRI string, and the like are kept exactly as written and shown as
  locked.
- **Browser behaviour is verified in Chromium only** (see Verification).
- **`prose-core` turns on `serde_json`'s `preserve_order` feature** (since 0.2;
  the edit layer needs document key order to write back what it read). Cargo
  unifies features across the build, so a host that depends on `prose-core`
  gets `preserve_order` for its own `serde_json` too: `serde_json::Map` keeps
  insertion order instead of sorting keys, and the host's own serialised JSON
  key order changes accordingly. A host that needs sorted keys must sort them
  itself.
- **Minimum supported Rust is 1.88** (let-chains); `rust-version` says so.
- **No JSON-LD processing.** A custom `@context` that renames ODRL terms is not
  followed. Terms it does not recognise are listed in `Document::warnings`
  (shown by the component), never silently dropped or guessed at.
- **Strict validation is a reading of the vocabulary, not a proof of
  meaning.** It does not fetch a declared profile, an `owl:imports` target,
  a remote `@context` or a remote `inheritFrom` parent; each is a gap and the
  verdict is `incomplete`. It does not decide whether a constraint is
  satisfied (that needs runtime claims, and belongs to a policy engine). The
  `profile.*` rules are this crate's reading of IM §2.1 and §3.2 under a
  closed world, and the `prose.*` rules a ds-prose convention; the report's
  `basis` says which. Report pointers (RFC 6901), SARIF output and a
  `--baseline` for the command are not there yet.
- **Reading, not evaluating.** PROSE says what a policy states. It does not
  decide whether a request complies; that is a policy engine's job.
- **English only** (the edit templates too), and action phrasing assumes the action is a verb that takes
  the target as its object (`use`, `distribute`, `attribute`). Unusual actions
  get a humanised name, which can read awkwardly.
- Party and asset identifiers are shown as written; they are not resolved.
- **Profiles: read, not reasoned over.** No JSON-LD processor, no OWL or
  SHACL reasoning beyond bounded transitive `includedIn`, `broader` and
  `subClassOf` within the loaded profiles, and no fetching (see Profiles).
  Turtle needs the `turtle` feature; RDF/XML is not read. `skos:note` and
  `skos:definition` prose is shown, never parsed into restrictions. The
  read-only `OdrlProse` gets the profile's labels and a glossary, but no
  per-term `title` (only `OdrlProseView`'s Read mode has those). Party
  functions and asset relations a profile declares are read, not offered
  in dropdowns. Only the label language is picked; the sentence templates
  stay English.

## CI

`.github/workflows/ci.yml` runs on every push and pull request, and weekly
for new advisories:

| job | gates |
|---|---|
| `gates` | `cargo fmt --check`, `clippy -D warnings`, `cargo test`, `cargo doc` with warnings denied, `prose-core` tested without and with `turtle`, the ignored time-budget test of strict validation run in release (`validate_perf`), every feature build (host, `ssr`, `csr` and `turtle` on wasm32) |
| `browser` | wasm clippy, and the Edit-mode DOM tests, the demo's tab and profile tests and the `prose-wasm` tests mounted in headless Chrome; the `wasm-bindgen-cli` version is read from `Cargo.lock` |
| `demo` | release `trunk build` under the Pages sub-path `/ds-prose-rs/demo/`, then `scripts/smoke-demo.sh` boots it in Chrome and fails unless a policy rendered and the Profile field is there |
| `angular` | `scripts/build-npm-packages.sh` builds and packs both npm packages, `scripts/check-npm-packages.mjs` checks the tarballs, the `ds-prose-validate` tests (`node --test scripts/test/*.test.mjs`, and every exit code through the bin of the packed tarball), `npm publish --dry-run`, the wrapper's typecheck and node tests, then `demo-angular` is built from the tarballs only under `/ds-prose-rs/demo-angular/` and booted in Chrome. The job then builds the Yew demo, runs the site tests (`node --test "site/test/*.test.mjs"`), assembles `site/dist` with `node site/build.mjs` and runs `site/test/layout-check.mjs` on it: every page and both demos at 1280 and 400 px, light and dark, with the demos' stylesheet switch, theme and validation controls |
| `supply-chain` | `cargo deny check` against `deny.toml`: licences, advisories, sources |

`deny.toml` ignores two "unmaintained" notices that arrive only through
`yew`, each with its reason. Dependabot proposes cargo and Actions updates
weekly. Third-party Actions are pinned by commit SHA (Dependabot keeps them
current) and `cargo-deny` by version. `.github/workflows/pages.yml` deploys the site from `main` after the same
boot checks of both stamped demos and the layout check.

## npm packages

Two packages are published to npmjs under the `@ds-labs` scope:

| package | what | for |
|---|---|---|
| [`@ds-labs/prose-wasm`](prose-wasm/npm/README.md) | the wasm and its JavaScript glue; framework-neutral | any JavaScript host |
| [`@ds-labs/prose-angular`](prose-angular/README.md) | `<ds-odrl-prose [json] [profile]>`, Angular 19 or newer | Angular apps; depends on the wasm package |

`@ds-labs/prose-wasm` also installs the `ds-prose-validate` command (see
Strict validation), and both packages can validate a policy against profiles;
the rendering itself is read-only.

An Angular app needs `npm i @ds-labs/prose-angular` and one `angular.json`
asset entry pointing at the wasm file inside `node_modules`; there is nothing
to vendor or build. There is no edit mode in either.

Both take a profile as text: `ProseHandle.setProfile(text | null)` returns
the profile's summary as JSON and throws on a text that is not a profile,
keeping the previous one; the Angular component has a `profile` input and
`profileLoaded` and `profileError` outputs. A profile is JSON-LD; Turtle is
opt-in, because the Turtle reader costs about 30 KiB over the wire. The wasm
package ships two builds of the same API: `@ds-labs/prose-wasm` with
`prose_wasm_bg.wasm` (JSON-LD), and `@ds-labs/prose-wasm/turtle` with
`prose_wasm_turtle_bg.wasm` (JSON-LD and Turtle; the `ds-prose-validate`
command uses it). The Angular component takes the Turtle build through the
`PROSE_WASM_MODULE` token. The packages never fetch a profile; the host does.

Both builds use the `wasm-release` profile of the workspace `Cargo.toml`
(`opt-level = "z"`, LTO, one codegen unit, symbols stripped), as does the
demo's release build; native builds keep `release`. The wasm grew in 0.4 for
profiles (wasm-pack, `wasm-opt` off; the earlier rows are the `release`
profile, measured on 2026-10-11):

| `prose_wasm_bg.wasm` | bytes | gzip -9 |
|---|---|---|
| 0.3.1 | 488,862 | 174,680 |
| 0.4.0 without `turtle` | 774,473 | 260,984 |
| 0.4.0 with `turtle`, before strict validation | 957,764 | 323,480 |
| 0.4.0 with `turtle` and strict validation (as built by `scripts/build-npm-packages.sh`, 2026-10-12) | 1,226,818 | 409,877 |
| the same, `release` profile, 2026-10-14 | 1,398,514 | 419,457 |
| `prose_wasm_bg.wasm`, `wasm-release` profile, without `turtle` (2026-10-14) | 603,775 | 223,926 |
| `prose_wasm_turtle_bg.wasm`, `wasm-release` profile, with `turtle` (2026-10-14) | 715,793 | 261,071 |

The two npm packages and the Rust crates share one version number (0.4.0
at the time of writing), so a number says which reader and renderer you have.
A release bumps them together; the Rust version the wasm was built from is
also recorded in the wasm package's `package.json`. Releases are tags, `prose-wasm-vX.Y.Z` and
`prose-angular-vX.Y.Z`, published by `.github/workflows/release-npm.yml` with
npm trusted publishing (OIDC, no token in this repository). The workflow
*stages* each release (`npm stage publish`); it goes live only when you approve
it with your 2FA (on npmjs.com, or `npm stage approve <stage-id>`). Publish
`prose-wasm` first when `prose-angular` needs a newer one.

```bash
scripts/build-npm-packages.sh        # build and pack both into dist-npm/tarballs
node scripts/check-npm-packages.mjs  # check the tarballs
```

## Develop

```bash
cargo test --workspace      # prose-core with turtle (prose-wasm turns it on by default)
cargo test -p prose-core   # prose-core alone, without turtle
cargo test -p prose-core --release --test validate_perf -- --ignored  # a time budget; ignored in debug
node --test scripts/test/*.test.mjs   # the ds-prose-validate command, with a fake engine
cd demo && trunk serve     # Trunk 0.22.0-beta.2+, like the other ds-labs-org demos
```

The site (see `site/README.md`) builds from the two demos. The demos are built
under the sub-paths they are served from, and `SITE_BASE` is the Pages prefix:

```bash
(cd demo && trunk build --release --public-url /ds-prose-rs/demo/)
scripts/build-npm-packages.sh
(cd demo-angular && npm ci && npm install --no-save ../dist-npm/tarballs/*.tgz \
  && npx ng build --base-href /ds-prose-rs/demo-angular/)
node --test "site/test/*.test.mjs"                  # generator, contract and browser tests
SITE_BASE=/ds-prose-rs/ node site/build.mjs         # assembles site/dist, stamps the navigation
node site/test/layout-check.mjs site/dist /ds-prose-rs/   # CHROME=... if there is no google-chrome; SHOT_DIR=... for screenshots
```

`trunk serve` shows the demo without the navigation: the stamp markers in
`demo/index.html` are comments until `site/build.mjs` replaces them.

The demo's Profile field reads one profile at a time: paste it, pick a
preset (the research profile of the tests, as JSON-LD or Turtle, or the
W3C ODRL 2.2 vocabulary trimmed to its terms, see `demo/profiles/`), choose
a file, or load a URL. Applying is explicit, and both tabs use it.
