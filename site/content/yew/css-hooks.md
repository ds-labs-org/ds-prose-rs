---
title: Yew CSS hooks
label: CSS hooks
description: Class names, attributes and custom properties of the prose-yew output that a stylesheet can rely on, and a complete themed example stylesheet.
nav: yew
order: 5
---
# CSS hooks (Yew)

This page lists what a stylesheet can select in the output of `prose-yew`, split into Stable and Internal, and ends with a complete example stylesheet in light and dark.

The repository states no CSS stability policy. The README lists some classes, the Rust doc comments list others, and the snapshots in `prose-yew/tests/snapshots/` pin the read-only markup. The split below is a recommendation based on how much each hook is documented, pinned by a test, or used by the two demos. Stable means: select it. Internal means: it carries layout or wiring and may change in a minor release.

The tables are checked against the code. A test collects every `prose-*` string literal in `prose-yew/src/` and fails when one is missing from this page or when this page names one the code does not write.

{{include hooks-renderer}}

## Stable hooks: editor view

`OdrlProseView` writes these in addition to the names above. In read mode the same markup is written without the controls, the slots, the selects, the `<style>` element, the add bars and the live region. Source: `prose-yew/src/edit/view.rs`, `slot.rs`, `choice.rs`.

| Selector | Element | What it styles |
|---|---|---|
| `.prose-view` | `article[tabindex="-1"]` | The root of `OdrlProseView`, next to `.ds-prose`, in both modes. |
| `.prose-edit` | root | Edit mode only. `BASE_CSS` is scoped to `.ds-prose.prose-edit`. |
| `.prose-reveal-hover` | root | Edit mode with `Reveal::OnHoverOrFocus`: the add, remove, reorder and group buttons are dimmed until hover or focus. |
| `.prose-heading-row` | `div` | Edit mode: wraps the policy heading and its toolbar. |
| `.prose-heading` | `h3` | The policy heading. |
| `.prose-intro` | `p` | The policy intro. |
| `.prose-conditions` | `div` | A label, a `ul.prose-list` and, in edit mode, an add bar. Used for limits and conditions. |
| `.prose-list` | `ul` | A list of conditions or follow-up rules. |
| `.prose-condition` | `li` | A condition. |
| `.prose-logical` | `li` | A condition that has children (an and, or, xone group), next to `.prose-condition`. |
| `.prose-follow-up` | `div` | A block of duties, remedies or consequences. |
| `.prose-carried` | `div` | A follow-up block in a position ODRL does not give it, next to `.prose-follow-up`. |
| `.prose-term` | `span[title]` | A displayed term. With a profile that explains it, it also has `data-iri` and `aria-describedby`. |
| `.prose-item` | `span` | One entry in a list of actions, parties or assets. |
| `.prose-inherited` | `span[title]` | A value inherited from the parent policy. |
| `.prose-text` | `span` | Written only when the host sets `classes.text` or `styles.text`. |
| `.prose-locked-note` | `span` | The note on a part the editor keeps verbatim. |
| `.prose-locked` | `code` | The JSON of that part. |
| `.prose-empty` | `p` or `span` | The text of an empty state. |
| `.prose-decoration` | `span` | A badge the host returned from the `decorate` callback. |
| `.prose-add-bar` | `div` | Edit mode: the row of add buttons and list issues. |
| `.prose-toolbar` | `span` | Edit mode: the move, remove and group buttons of one item. |
| `.prose-add` | `button[data-add]` | Add button. `data-add` holds the list path. |
| `.prose-remove` | `button` | Remove button. |
| `.prose-reorder` | `button` | Move up and move down. |
| `.prose-wrap` | `button` | Group and ungroup. |
| `.prose-live` | `div[role="status"]` | The announcer, visually hidden by `BASE_CSS`. With `base_css` off, hide it yourself. |
| `.prose-slot-wrap` | `span` | Positioning context for a slot, its message and its listbox. |
| `.prose-slot` | `span[contenteditable]` | An editable slot. Has `data-slot` and `data-placeholder`. |
| `.prose-slot-empty` | slot | The slot has no text. |
| `.prose-slot-invalid` | slot | Error state. A slot with `aria-invalid="true"` is styled the same. |
| `.prose-slot-warning` | slot | Warning state. |
| `.prose-slot-whitespace` | slot | The text has leading, trailing or non-breaking whitespace. |
| `.prose-slot-note` | `small` | The "not in the profile" note under a closed slot. |
| `.prose-slot-message` | `span` | Why typed text was refused, positioned above the slot. Also has `.prose-issue` and `.prose-issue-error`. |
| `.prose-suggestions` | `ul[role="listbox"]` | The open suggestion list. |
| `.prose-suggestion` | `li[role="option"]` | One option. |
| `.prose-suggestion-active` | option | The active option. |
| `.prose-suggestion-nested` | option | A value narrower than a suggested one. Indented through `--prose-depth`. |
| `.prose-suggestion-group` | `li[role="group"]` | A heading and its options, written when some suggestion has a group. |
| `.prose-suggestion-group-label` | `span` | The group heading. |
| `.prose-select` | `select` | A closed choice such as an operator. |
| `.prose-rule-kind` | `select` | The may, must not, must select, next to `.prose-select`. |
| `.prose-issue` | `span` | An issue message the host passed in `issues` or `refusal`. |
| `.prose-issue-error`, `.prose-issue-warning`, `.prose-issue-info` | `span` | The severity of an issue, next to `.prose-issue`. |

Two more selectors have no class. A closed slot has `data-closed="true"`. A rule also carries `prose-` plus the name of its list (`permission`, `prohibition`, `obligation`, `duty`, `remedy` or `consequence`), built at run time. Select the first three through the kind classes above, and do not select the other three.

## Stable hooks: problems panel

`ProseProblems` writes these. The panel is usually placed next to the view and outside `.ds-prose`, so its rules are not scoped under `.ds-prose`. Source: `prose-yew/src/edit/problems.rs`; the markup is pinned by `strict_problems.html`.

| Selector | Element | What it styles |
|---|---|---|
| `.prose-problems` | `section` | The panel root. The `class` prop is appended here. |
| `.prose-problems-heading` | `h3` | The heading. It takes focus after a fix removed the focused button. |
| `.prose-problems-status` | `p[role="status"]` | The verdict, for a strict report only, with one of `.prose-conforms`, `.prose-incomplete`, `.prose-violations-found`. |
| `.prose-problems-counts` | `p` | The counts of errors, warnings and notes. |
| `.prose-problem-groups` | `ol` | The groups, one per rule. |
| `.prose-problem-group` | `li` | One rule: an `h4` with the rule id and title, then its findings. |
| `.prose-problem-list` | `ol` | The findings of one rule. |
| `.prose-problem` | `li` | One finding. |
| `.prose-problem-error`, `.prose-problem-warning`, `.prose-problem-info` | `li` | The severity, next to `.prose-problem`. |
| `.prose-problem-gap` | `li` | The finding is a gap: something could not be checked. |
| `.prose-problem-message` | `span` | The message. |
| `.prose-problem-go` | `button` | Go to the target. The host moves focus. |
| `.prose-problem-fix` | `button[data-safe]` | One per available fix. `data-safe="false"` marks a fix that is not lossless. |

## Internal hooks

| Selector | Element | Why it is internal |
|---|---|---|
| `.prose-slot-edit` | `span` | Inner wrapper of a slot. `BASE_CSS` has no rule for it. |
| `--prose-depth` | custom property | Set inline by the library on each nested suggestion. Do not set it. |

Also internal: the element ids (`prose-sugg-<slot>`, `prose-note-<slot>`, `prose-message-<slot>`, `prose-issue-<key>-<i>`, `prose-def-<slot>`), the text of every `aria-label` and `title`, `tabindex="-1"` on containers, the order of siblings, the comment markers in server-rendered output, and the text of `BASE_CSS` itself. The contract is that the editor ships a default stylesheet you can turn off.

## Data attributes and roles

`data-node` marks a policy, rule or condition container and holds its path. `data-slot` marks a slot or select and holds its path. `data-add` marks an add button and holds the path of the list it adds to; `data-item="group"` marks the button that adds a group. `data-closed="true"` marks a closed slot, which takes only the values the profile lists. `data-placeholder` holds the text an empty slot shows. `data-iri` holds the full IRI of a profile term in read mode. `data-safe` on a fix button says whether the fix is lossless. The paths are the strings of `NodePath`, `SlotPath` and `ListPath` from `prose-core`, and `ProseProblems::onjump` hands them to a host that moves focus. A slot is a `combobox` when suggestions are on and a `textbox` when they are off.

## Custom properties

| Property | Read by | Fallback | Set by |
|---|---|---|---|
| `--prose-error` | `BASE_CSS`: the invalid underline and the error issue text | `#c9190b` | The host, on any ancestor or on `.ds-prose`. |
| `--prose-warning` | `BASE_CSS`: the warning underline and the warning issue text | `#b45309` | The host. |

The read-only renderer reads no custom properties. `--prose-error` does not colour `.prose-error`, `.prose-violation-error` or `.prose-problem-error`; style those classes yourself, as the example does. No other custom property exists in the Rust, TypeScript or CSS sources of the repository.

## Example stylesheet: rendered policy

`prose.css` is a complete starting point for the rendered policy, the glossary and the validation findings. It defines its own tokens (`--ds-*`) in light, in dark by system preference and in dark by `data-theme="dark"` on `<html>`, and sets `--prose-error` and `--prose-warning` from them. Rename the tokens to yours and keep the selectors. The same file is served at `examples/prose.css`, and the home page of this site loads it. A test renders the real output with it in Chrome and checks the colours.

{{file examples/prose.css}}

## Example stylesheet: editor and problems panel

`prose-editor.css` recolours the editor and the problems panel with the same tokens. Load it after `prose.css`. It keeps `base_css` on. See [Styling](/yew/styling/) for why every selector starts with `:root`.

{{file examples/prose-editor.css}}
