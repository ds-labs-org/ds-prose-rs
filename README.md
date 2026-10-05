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
| `prose-yew/` | `<OdrlProse json={...} />`: lays a `Document` out as semantic HTML. `<OdrlProseView>`: the same sentences, read or edited |
| `demo/` | Trunk app: paste a policy and read it, or edit the prose itself |

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
values or arrays are all accepted.

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
  expects to find, so a stale event is refused rather than misapplied. `apply`
  is atomic. `focus_after` says where keyboard focus belongs afterwards.
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
| `onedit: Callback<EditEvent>` | every committed edit and structural action; not applied by the component |
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
- Yew never reads the DOM, so a slot is remounted after every commit or revert;
  that keeps the DOM and the document in step even when the host refuses the
  edit.
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
- **No JSON-LD processing.** A custom `@context` that renames ODRL terms is not
  followed. Terms it does not recognise are listed in `Document::warnings`
  (shown by the component), never silently dropped or guessed at.
- **Reading, not evaluating.** PROSE says what a policy states. It does not
  decide whether a request complies; that is a policy engine's job.
- **English only** (the edit templates too), and action phrasing assumes the action is a verb that takes
  the target as its object (`use`, `distribute`, `attribute`). Unusual actions
  get a humanised name, which can read awkwardly.
- Party and asset identifiers are shown as written; they are not resolved.

## Develop

```bash
cargo test --workspace
cd demo && trunk serve     # Trunk 0.22.0-beta.2+, like the other ds-labs-org demos
```
