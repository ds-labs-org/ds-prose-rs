# ds-prose-rs

**PROSE: Prose Renderer for ODRL Semantic Encoding.** A
[Yew](https://yew.rs) component that reads an ODRL policy written as JSON-LD
and writes it as plain-language prose, for people who have to decide whether
to accept a policy but should not have to read JSON-LD to do it.

## Layout

| path | what it is |
|---|---|
| `prose-core/` | reads ODRL JSON-LD, builds a structured `Document`. No UI framework |
| `prose-core/src/edit/` | the edit layer: an addressable policy model, edit events, edit rules, a JSON-LD reader and writer, and the edit sentence templates |
| `prose-yew/` | `<OdrlProse json={...} />`: lays a `Document` out as semantic HTML. `<OdrlProseView>`: the same sentences for documents written with bare keys, read or edited |
| `demo/` | Trunk app: paste a policy and read it, or edit the prose itself |
| `prose-wasm/` | wasm-bindgen bridge that mounts the read-only `<OdrlProse>` into any DOM element; `prose-wasm/npm/` holds the metadata of the npm package |
| `prose-angular/` | Angular wrapper, `<ds-odrl-prose [json]>`, over `prose-wasm` (read-only in v1; no edit mode); built with ng-packagr |
| `demo-angular/` | Angular demo app; installs the two packed npm packages, like a downstream app |

```rust
use prose_yew::OdrlProse;
html! { <OdrlProse json={policy_json} /> }
```

Or without a UI: `prose_core::render(json) -> Result<Document, ProseError>`.

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
reads as "the date and time is before 2026-12-31".

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
- `EditEvent` and `EditDoc::apply`: set text, set a choice, add, remove, move,
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

### Compatibility

The enums that grow as the editor learns more of ODRL are `#[non_exhaustive]`,
so adding a variant is a minor release, not a breaking one. In `prose-core`:
`EditEvent`, `EditError`, `NewItem`, `FocusTarget`, `ListKind`, `Field`, `Step`,
`NodeRef`, `Segment`, `SlotKind`, `ChoiceKind`, `EmptyText` and `IssueTarget`.
In `prose-yew`: `FocusKind` and `DecorationAt`. Outside the defining crate a
`match` on one of them needs a wildcard arm with a sensible fallback. Units,
`rightOperandReference` and `partOf` can be edited but not added today (see
Limits); adding them is the kind of change this allows.

Left exhaustive because they are closed by nature: `Conj`, `Severity`,
`EditMode`, `ButtonContent`, `Reveal`, `LogicalOp`, `RuleList`, `EntityRole`
and `DocShape` (ODRL or this crate fixes their members), and the data enums
`ConstraintNode`, `RightOperand` and `Literal`, which already carry an
`Opaque` or raw fallback for what they do not understand.

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
  bring your own (also needed with server rendering, where Yew escapes style text).
- `show_raw_terms`, `suggestions`, `max_suggestions`.

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
- **Reading, not evaluating.** PROSE says what a policy states. It does not
  decide whether a request complies; that is a policy engine's job.
- **English only** (the edit templates too), and action phrasing assumes the action is a verb that takes
  the target as its object (`use`, `distribute`, `attribute`). Unusual actions
  get a humanised name, which can read awkwardly.
- Party and asset identifiers are shown as written; they are not resolved.

## CI

`.github/workflows/ci.yml` runs on every push and pull request, and weekly
for new advisories:

| job | gates |
|---|---|
| `gates` | `cargo fmt --check`, `clippy -D warnings`, `cargo test`, `cargo doc` with warnings denied, every feature build (host, `ssr`, `csr` on wasm32) |
| `browser` | wasm clippy, and the Edit-mode DOM tests and the demo's tab tests mounted in headless Chrome; the `wasm-bindgen-cli` version is read from `Cargo.lock` |
| `demo` | release `trunk build` under the Pages sub-path, then `scripts/smoke-demo.sh` boots it in Chrome and fails unless a policy rendered |
| `supply-chain` | `cargo deny check` against `deny.toml`: licences, advisories, sources |

`deny.toml` ignores two "unmaintained" notices that arrive only through
`yew`, each with its reason. Dependabot proposes cargo and Actions updates
weekly. Third-party Actions are pinned by commit SHA (Dependabot keeps them
current) and `cargo-deny` by version. `.github/workflows/pages.yml` deploys the demo from `main` after the
same boot check.

## npm packages

Two packages are published to npmjs under the `@ds-labs` scope:

| package | what | for |
|---|---|---|
| [`@ds-labs/prose-wasm`](prose-wasm/npm/README.md) | the wasm and its JavaScript glue; framework-neutral | any JavaScript host |
| [`@ds-labs/prose-angular`](prose-angular/README.md) | `<ds-odrl-prose [json]>`, Angular 19 or newer | Angular apps; depends on the wasm package |

An Angular app needs `npm i @ds-labs/prose-angular` and one `angular.json`
asset entry pointing at the wasm file inside `node_modules`; there is nothing
to vendor or build. Both are read-only: there is no edit mode in them.

Versions are independent of each other and of the Rust crates (the Rust
version the wasm was built from is recorded in the wasm package's
`package.json`). Releases are tags, `prose-wasm-vX.Y.Z` and
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
cargo test --workspace
cd demo && trunk serve     # Trunk 0.22.0-beta.2+, like the other ds-labs-org demos
```
