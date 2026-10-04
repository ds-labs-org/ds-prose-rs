# ds-prose-rs

**PROSE: Prose Renderer for ODRL Semantic Encoding.** A
[Yew](https://yew.rs) component that reads an ODRL policy written as JSON-LD
and writes it as plain-language prose, for people who have to decide whether
to accept a policy but should not have to read JSON-LD to do it.

## Layout

| path | what it is |
|---|---|
| `prose-core/` | reads ODRL JSON-LD, builds a structured `Document`. No UI framework |
| `prose-yew/` | `<OdrlProse json={...} />`: lays a `Document` out as semantic HTML |
| `demo/` | Trunk app: paste a policy, read it |

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

## Limits

- **No JSON-LD processing.** A custom `@context` that renames ODRL terms is not
  followed. Terms it does not recognise are listed in `Document::warnings`
  (shown by the component), never silently dropped or guessed at.
- **Reading, not evaluating.** PROSE says what a policy states. It does not
  decide whether a request complies; that is a policy engine's job.
- **English only**, and action phrasing assumes the action is a verb that takes
  the target as its object (`use`, `distribute`, `attribute`). Unusual actions
  get a humanised name, which can read awkwardly.
- Party and asset identifiers are shown as written; they are not resolved.

## Develop

```bash
cargo test --workspace
cd demo && trunk serve     # Trunk 0.22.0-beta.2+, like the other ds-labs-org demos
```
