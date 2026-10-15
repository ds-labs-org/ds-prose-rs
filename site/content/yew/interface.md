---
title: Yew interface
label: Interface
description: Every property, hook, configuration field and wording label of the prose-yew components, with types and defaults taken from the source.
nav: yew
order: 2
---
# Yew interface

The tables on this page are checked against the Rust source. A test reads the struct that backs each table and fails when a field, a type or a default differs from what is written here. The code blocks that show usage are regions of `prose-yew/tests/site_examples.rs`, which `cargo test -p prose-yew` compiles and renders on the server.

Four components exist: `OdrlProse`, `OdrlProseView`, `ProseGlossary` and `ProseProblems`. The first renders a policy given as JSON text. The second renders and edits a parsed document, and is controlled: it never changes the document, and the host applies every event it emits. Three hooks keep the host's state: `use_prose_editor`, `use_prose_editor_with` and `use_validation`.

## `OdrlProse`

A function component that renders ODRL JSON-LD as semantic HTML with `prose_core::render_with`. It emits no `<style>`. The host styles it through the `prose-*` classes, or through `class` on the root.

| Property | Type | Default | Meaning |
|---|---|---|---|
| `json` | `AttrValue` | required | The ODRL policy as JSON-LD text. |
| `class` | `Classes` | empty | Added to the root `article`, after `ds-prose`. |
| `wording` | `Option<SharedWording>` | `None` | The loaded profiles. `None` reads as before 0.4. |
| `glossary` | `bool` | `true` | Show a glossary of profile terms after each policy. It appears only when a profile explains a term the policy uses. |
| `validator` | `Option<Validator>` | `None` | Validate against the validator's profiles. Nothing is shown for it unless `violations` is set. |
| `violations` | `bool` | `false` | Add a section after the policies with the findings and the verdict. Needs `validator`. |
| `links` | `Rc<BTreeSet<String>>` | empty | IRIs the host found to dereference. A condition that refers to a constraint declared at one of them, outside the document, shows the IRI as an `a.prose-reference`; any other IRI stays plain text. The component fetches nothing. |

`SharedWording` wraps an `Rc<Wording>`. Two values are equal when they are the same `Rc`, so the component renders again only when the host hands it another `Rc`.

A policy that cannot be read renders `<article class="ds-prose"><p class="prose-error" role="alert">`. A struct literal of `OdrlProseProps` needs all six fields, because 0.4 added four of them; `html!` fills the defaults.

{{source prose-yew/tests/site_examples.rs#reader}}

With a profile, the glossary switched off and a class:

{{source prose-yew/tests/site_examples.rs#reader_wording}}

## `OdrlProseView`

Renders a parsed `EditDoc` in read or edit mode. In edit mode the slots are `contenteditable`, the selects and the add and remove buttons appear, and focus is managed. Every committed edit leaves through `onedit` as an `EditEvent`. A structural event whose new document has not arrived within 500 ms is treated as refused: focus does not move and nothing is announced.

| Property | Type | Default | Meaning |
|---|---|---|---|
| `doc` | `Rc<EditDoc>` | required | The document. The host owns it. |
| `mode` | `EditMode` | `EditMode::Read` | Read or edit. |
| `config` | `Rc<EditConfig>` | `Rc::new(EditConfig::default())` | Everything in the `EditConfig` section below. |
| `onedit` | `Callback<EditEvent>` | no-op | Every committed edit and structural action. Not applied by the component. |
| `on_focus_slot` | `Callback<Option<SlotFocus>>` | no-op | `Some` when a slot or select gets focus, `None` when it loses it. For help text. |
| `issues` | `Rc<Vec<Issue>>` | empty | Findings the host computed, shown at their targets. An error sets `aria-invalid` and shows its message. |
| `refusal` | `Option<Issue>` | `None` | An edit the host refused, shown with the issues at its slot. Normally `error.issue(&event)` of `ProseEditorWith::refused`. |
| `decorate` | `Option<Callback<Decoration, Html>>` | `None` | Host badges after each policy heading, rule sentence and condition. |
| `class` | `Classes` | empty | Extra classes on the root, appended last. |

The root is `<article class="ds-prose prose-view {config.classes.root} [prose-edit [prose-reveal-hover]] {class}" tabindex="-1">`. `prose-edit` is present only in edit mode, and `prose-reveal-hover` only in edit mode with `Reveal::OnHoverOrFocus`. In edit mode with `base_css` on, the first child of the root is the `<style>` element holding `BASE_CSS`. Read mode never writes a `<style>`.

{{source prose-yew/tests/site_examples.rs#read_view}}

The edit layer reads bare property keys only (`permission`, not `odrl:permission`, and no full IRIs). `OdrlProse` reads all three. A document written with prefixed keys renders in `OdrlProse` and reads as no rules in `read_model`.

## `EditMode`

- `EditMode::Read` (the default): the same sentences with display text and no controls.
- `EditMode::Edit`: contenteditable slots, selects, add and remove buttons, suggestions, issues and focus management.

## `ProseGlossary`

Renders the profile terms one policy uses as `<details class="prose-glossary" open>` with a `<dl>`. `OdrlProse` uses it for you. Use it directly to place a glossary elsewhere. It renders nothing for an empty glossary, and shows at most 30 entries.

| Property | Type | Default | Meaning |
|---|---|---|---|
| `glossary` | `PolicyGlossary` | required | The terms to show. |
| `labels` | `Rc<Labels>` | `Labels::default()` | The words of the glossary. |

## `ProseProblems`

The findings of a validation, grouped by rule, each with a "Go to" button and one button for each fix. See [Validation](/yew/validation/).

| Property | Type | Default | Meaning |
|---|---|---|---|
| `report` | `Rc<Report>` | required | The report, for example from `use_validation`. |
| `onjump` | `Callback<IssueTarget>` | no-op | A "Go to" button was pressed. The host moves focus to the target, a `[data-slot]` or `[data-node]` element of the view. |
| `onfix` | `Callback<Vec<EditEvent>>` | no-op | A fix button was pressed. Apply the events as one, with `ProseEditorWith::apply_all`. They carry `expect` ids, so a fix from a report the document has moved on from is refused as stale. |
| `labels` | `Rc<Labels>` | `Labels::default()` | The words of the panel. |
| `class` | `Classes` | empty | Added to the panel root. |

## Hooks

A ready-made host. It owns the document, applies each event with the rules, and keeps an undo history capped at 50 entries.

```rust
pub fn use_prose_editor(initial: impl FnOnce() -> EditDoc, rules: Rc<EditRules>) -> ProseEditor
pub fn use_prose_editor_with(
    initial: impl FnOnce() -> EditDoc,
    rules: Rc<EditRules>,
    validator: Option<Validator>,
    policy: StrictEdit,
) -> ProseEditorWith
pub fn use_validation(doc: &Rc<EditDoc>, validator: Option<&Validator>) -> Rc<Report>
```

`use_prose_editor` is `use_prose_editor_with(initial, rules, None, StrictEdit::Flag).editor`. `use_validation` recomputes the report only when the document or the validator is another one. Without a validator it returns the advisory report of no profile, which has no findings.

{{source prose-yew/tests/site_examples.rs#editor}}

## `ProseEditor`

What `use_prose_editor` returns. Pass `doc` and `onedit` to `OdrlProseView`.

| Field | Type | Meaning |
|---|---|---|
| `doc` | `Rc<EditDoc>` | The current document. |
| `onedit` | `Callback<EditEvent>` | Applies the event with the rules. A refused event leaves the document unchanged and sets `last_error`. |
| `set_doc` | `Callback<EditDoc>` | Replace the document and clear the history. |
| `undo` | `Callback<()>` | Pop one history entry. Never gated. |
| `can_undo` | `bool` | The history is not empty. |
| `last_error` | `Option<EditError>` | Why the last event was refused. |

## `ProseEditorWith`

What `use_prose_editor_with` returns. It is `#[non_exhaustive]`: do not build it with a struct literal.

| Field | Type | Meaning |
|---|---|---|
| `editor` | `ProseEditor` | As above. |
| `apply_all` | `Callback<Vec<EditEvent>>` | Applies the events as one: all or none, one undo step. This is how a fix from a report is applied. An empty list is not an edit. |
| `refused` | `Option<(EditEvent, EditError)>` | The last refused event and why, until the next edit succeeds or the validator or the `StrictEdit` policy changes. Hand `error.issue(&event)` to `OdrlProseView::refusal`. |

## `StrictEdit`

What the reducer does with an event the loaded profiles do not allow. It is `#[non_exhaustive]`.

- `StrictEdit::Flag` (the default): commit every edit the rules allow. The validator marks a violation as an error.
- `StrictEdit::RefuseTerms`: refuse, as `EditError::OutsideProfile`, an event that writes a term the validator does not allow into a vocabulary slot. It never applies to the empty string, free text, right operands, structural events or undo. The stock view asks the validator first and does not send such a term itself, so a refusal reaches only a host that bypasses the view.

After `EditConfig::with_strict`, give the hook `Rc::new(cfg.rules.clone())` and `cfg.validator.clone()`. The reducer applies the rules it is handed, not the config's. A host that keeps `EditRules::default()` gets the defaults it set, not those `with_strict` made admissible.

## `EditConfig`

The whole configuration of an editing view. It is `Clone`, `PartialEq` and `Default`.

| Field | Type | Default | Meaning |
|---|---|---|---|
| `rules` | `EditRules` | `EditRules::default()` | Limits per list. Controls the rules forbid are not rendered, and the reducer refuses the event anyway. |
| `labels` | `Labels` | `Labels::default()` | Every word the editor writes. See Labels below. |
| `classes` | `ProseClasses` | `ProseClasses::default()` | Extra classes per kind of element. See [Styling](/yew/styling/). |
| `styles` | `ProseStyles` | `ProseStyles::default()` | Inline styles per kind of element. |
| `vocab` | `Vocabulary` | `Vocabulary::default()` | Suggestions and closed lists: 20 ODRL actions, the known left operands, seven policy kinds, 12 operators, the conflict strategies. |
| `add_button` | `ButtonContent` | `ButtonContent::SymbolAndText` | What a plus button shows. |
| `remove_button` | `ButtonContent` | `ButtonContent::Symbol` | What a minus button shows. |
| `reveal` | `Reveal` | `Reveal::Always` | When the plus and minus buttons are visible. |
| `base_css` | `bool` | `true` | Edit mode writes `<style>` with `BASE_CSS`. Turn it off to bring your own. |
| `show_raw_terms` | `bool` | `true` | Operator options read "is before (lt)" rather than "is before". |
| `suggestions` | `bool` | `true` | A suggestion list while typing in a free-text slot. Off, a slot is a plain textbox. |
| `max_suggestions` | `usize` | `8` | The cap of the suggestion list. |
| `host_plans_operator_switches` | `bool` | `false` | The host translates an operator switch the reducer would refuse, so the select offers every operator. Off, it offers only the switches the reducer accepts. |
| `wording` | `Rc<Wording>` | `Rc::new(Wording::default())` | The loaded profiles. Set it with `with_wording`. |
| `validator` | `Option<Validator>` | `None` | Set it with `with_strict`. `None` validates nothing. |

Two builders consume the config and return it:

```rust
pub fn with_wording(self, w: Rc<Wording>) -> EditConfig
pub fn with_strict(self, options: Options) -> EditConfig
```

`with_wording` uses the profiles and adds their terms to `vocab` under `labels.group_profile`, so set the labels first. `with_strict` validates against `wording` with `options`, usually `Options::strict()`, and rewrites `rules.defaults` (left operand, operator, action, policy type) so that what an add button creates is admissible. Call it after `with_wording`.

Choices that fix a name:

- `ButtonContent::Symbol`, `ButtonContent::Text` and `ButtonContent::SymbolAndText` (the default). The accessible name of a button is always the specific label, whatever it shows.
- `Reveal::Always` (the default) and `Reveal::OnHoverOrFocus`, which adds `prose-reveal-hover` to the root.

## Labels

`Labels` holds every word the editor writes: the section headings, the rule kinds, the empty states, the names of the buttons, the notes of the strict mode. Every field is public. Templates substitute `{noun}`, `{n}`, `{field}`, `{owner}`, `{action}`, `{phrase}` and `{raw}`, and in specific fields `{profile}`, `{terms}`, `{note}`, `{kept}`, `{typed}`, `{label}`, `{suggestion}`, `{errors}`, `{warnings}` and `{notes}`. When a button template has no `{owner}`, the owner chain is appended, so each button has a distinct name.

Keep the visible text of a button contained in its accessible name. A change of wording goes in a copy of `Labels::default()`:

| Field | Default | Group |
|---|---|---|
| `conditions` | `Applies only if` | Section headings |
| `conditions_in_duty` | `Applies only if` | Section headings |
| `refinements` | `Limits on the action {action}` | Section headings |
| `follow_up_duty` | `Duties to fulfil before this permission can be exercised` | Section headings |
| `follow_up_duty_elsewhere` | `Duties attached to this rule` | Section headings |
| `follow_up_remedy` | `Remedies if this prohibition is breached` | Section headings |
| `follow_up_consequence` | `Consequences if this duty is not fulfilled` | Section headings |
| `follow_up_remedy_elsewhere` | `Remedies attached to this rule` | Section headings |
| `follow_up_consequence_elsewhere` | `Consequences attached to this rule` | Section headings |
| `carried_note` | (empty) | Section headings |
| `notes_heading` | `Not rendered` | Section headings |
| `may` | `may` | Rule kinds |
| `must_not` | `must not` | Rule kinds |
| `must` | `must` | Rule kinds |
| `empty` | see Nested wording | Empty states |
| `no_rules` | `No rules yet.` | Empty states |
| `no_policies` | `No policies yet.` | Empty states |
| `add` | `Add {noun}` | Button names |
| `remove` | `Remove {noun} {n}` | Button names |
| `move_up` | `Move {noun} {n} up` | Button names |
| `move_down` | `Move {noun} {n} down` | Button names |
| `wrap` | `Group {noun} {n}` | Button names |
| `unwrap` | `Ungroup {noun} {n}` | Button names |
| `wrap_text` | `Group` | Button text |
| `unwrap_text` | `Ungroup` | Button text |
| `move_up_symbol` | `↑` | Button text |
| `move_down_symbol` | `↓` | Button text |
| `add_symbol` | `+` | Button text |
| `remove_symbol` | `−` | Button text |
| `operator_option` | `{phrase}({raw})` | Selects, slots, announcements |
| `unknown_option` | `{raw}(not in the list)` | Selects, slots, announcements |
| `slot_aria` | `{field} of {owner}` | Selects, slots, announcements |
| `rule_kind_aria` | `Kind of {owner}` | Selects, slots, announcements |
| `locked` | `Kept exactly as written;not editable here` | Selects, slots, announcements |
| `inherited` | `From the policy` | Selects, slots, announcements |
| `announce_added` | `{noun} added` | Selects, slots, announcements |
| `announce_removed` | `{noun} removed` | Selects, slots, announcements |
| `nouns` | see Nested wording | Nested wording |
| `fields` | see Nested wording | Nested wording |
| `placeholders` | see Nested wording | Nested wording |
| `group_core` | `ODRL core` | Profiles and glossary |
| `group_profile` | `From profile:{profile}` | Profiles and glossary |
| `glossary_heading` | `Terms from the profile` | Profiles and glossary |
| `kind_of` | `A kind of:{terms}` | Profiles and glossary |
| `implies` | `Implies:{terms}` | Profiles and glossary |
| `usage_note` | `Usage:{note}` | Profiles and glossary |
| `from_profile` | `From {profile}` | Profiles and glossary |
| `deprecated` | `Deprecated in the profile` | Profiles and glossary |
| `suggested_operators` | `Suggested by the profile` | Profiles and glossary |
| `other_operators` | `Other operators` | Profiles and glossary |
| `profile_values` | `Profile values(not specific to this operand)` | Profiles and glossary |
| `datatype_placeholders` | a list of six pairs: integer, decimal, boolean, date, dateTime, duration | Profiles and glossary |
| `not_in_profile` | `(not in the profile)` | Strict validation |
| `kept_previous` | `Kept "{kept}";"{typed}" is not in the profile` | Strict validation |
| `kept_empty` | `"{typed}" is not in the profile;the field stays empty` | Strict validation |
| `allowed_group` | `Allowed by the profile` | Strict validation |
| `not_allowed_group` | `Not allowed by the profile` | Strict validation |
| `not_allowed_option` | `{label}(not allowed)` | Strict validation |
| `not_allowed_term` | `"{typed}" is not an allowed {field} in {profile}.` | Strict validation |
| `did_you_mean` | `Did you mean {suggestion}?` | Strict validation |
| `problems_heading` | `Problems` | Strict validation |
| `go_to` | `Go to` | Strict validation |
| `status_conforms` | `Conforms` | Strict validation |
| `status_violations` | `Does not conform` | Strict validation |
| `status_incomplete` | `Cannot be certified:part of the vocabulary was not checked` | Strict validation |
| `counts` | `errors:{errors},warnings:{warnings},notes:{notes}` | Strict validation |

## Nested wording

`nouns`, `fields` and `placeholders` are structs of text. `empty` holds the texts shown where a list is empty, whose defaults come from `prose-core`.

| Field | Default |
|---|---|
| `Nouns.policy` | `policy` |
| `Nouns.permission` | `permission` |
| `Nouns.prohibition` | `prohibition` |
| `Nouns.obligation` | `obligation` |
| `Nouns.duty` | `duty` |
| `Nouns.remedy` | `remedy` |
| `Nouns.consequence` | `consequence` |
| `Nouns.assigner` | `assigner` |
| `Nouns.assignee` | `assignee` |
| `Nouns.target` | `target` |
| `Nouns.action` | `action` |
| `Nouns.refinement` | `limit` |
| `Nouns.condition` | `condition` |
| `Nouns.group` | `condition group` |
| `Nouns.value` | `value` |
| `Nouns.profile` | `profile` |
| `Nouns.parent` | `parent policy` |
| `FieldNames.kind` | `policy type` |
| `FieldNames.uid` | `identifier` |
| `FieldNames.conflict` | `conflict strategy` |
| `FieldNames.profile` | `profile` |
| `FieldNames.inherit_from` | `parent policy` |
| `FieldNames.reference` | `reference` |
| `FieldNames.iri` | `identifier` |
| `FieldNames.part_of` | `collection` |
| `FieldNames.name` | `action` |
| `FieldNames.left_operand` | `left operand` |
| `FieldNames.operator` | `operator` |
| `FieldNames.right_operand` | `value` |
| `FieldNames.operand_reference` | `value reference` |
| `FieldNames.unit` | `unit` |
| `FieldNames.logical_op` | `logical operator` |
| `Placeholders.kind` | `type` |
| `Placeholders.uid` | `identifier` |
| `Placeholders.party` | `party` |
| `Placeholders.asset` | `asset` |
| `Placeholders.action` | `action` |
| `Placeholders.left_operand` | `left operand` |
| `Placeholders.literal` | `value` |
| `Placeholders.unit` | `unit` |
| `Placeholders.operand_reference` | `IRI` |
| `Placeholders.part_of` | `collection` |
| `Placeholders.profile` | `profile` |
| `Placeholders.policy_ref` | `policy identifier` |
| `Placeholders.reference` | `IRI` |
| `EmptyTexts.anyone_subject` | `Anyone` |
| `EmptyTexts.anyone_object` | `anyone` |
| `EmptyTexts.the_assignee` | `The assignee` |
| `EmptyTexts.unspecified_asset` | `an unspecified asset` |
| `EmptyTexts.unspecified_action` | `perform an unspecified action on` |
| `EmptyTexts.no_assigner` | `no assigner` |
| `EmptyTexts.no_parent` | `no other policy` |
| `EmptyTexts.no_profile` | `no profile` |

## Examples

Every block above comes from `prose-yew/tests/site_examples.rs`. The remaining two are the configuration with your own CSS, which [Styling](/yew/styling/) explains, and the strict editor with the problems panel, which [Validation](/yew/validation/) explains.

The demo in `demo/` is the full host: `demo/src/edit.rs` adds undo, the strict bar, `issues` and `onjump`.
