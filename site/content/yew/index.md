---
title: Yew component
label: Overview
description: Install the prose-yew crate and render or edit an ODRL policy as plain-language sentences in a Yew application.
nav: yew
order: 1
---
# Yew component

`prose-yew` is a Yew component crate. `OdrlProse` renders an ODRL JSON-LD policy as plain-language sentences. `OdrlProseView` renders a parsed document and, in edit mode, lets the reader change it in place: the sentences are the form. The crate also has a strict-validation mode with a problems panel.

The crate targets Yew 0.23, Rust edition 2024, and Rust 1.88 or newer. The [Yew demo](/demo/) runs it in the browser, and its source is `demo/` in the repository.

## Install

```toml
[dependencies]
prose-yew = "0.4"
yew = { version = "0.23", features = ["csr"] }
```

`prose-yew` has two features and none is on by default.

| Feature | Enables | Use it for |
|---|---|---|
| `csr` | `yew/csr` | Client-side rendering in the browser. |
| `ssr` | `yew/ssr` | Server-side rendering. |

The crate does not enable `csr` itself. The application that mounts the component must. Edit mode needs the browser: `contenteditable`, focus and selection are DOM calls.

## What the crate exports

The root of the crate re-exports everything you need.

- Components: `OdrlProse`, `OdrlProseView`, `ProseGlossary`, `ProseProblems`, with the property types `OdrlProseProps`, `OdrlProseViewProps`, `GlossaryProps`, `ProseProblemsProps`.
- Hooks: `use_prose_editor`, `use_prose_editor_with`, `use_validation`, with the state they return, `ProseEditor` and `ProseEditorWith`.
- Configuration: `EditConfig`, `EditMode`, `Labels`, `Nouns`, `FieldNames`, `Placeholders`, `EmptyTexts`, `ProseClasses`, `ProseStyles`, `Vocabulary`, `Suggestion`, `Choice`, `ButtonContent`, `Reveal`, `StrictEdit`, `filter_suggestions`.
- Events and hints for hosts: `SlotFocus`, `FocusKind`, `Decoration`, `DecorationAt`, `SharedWording`.
- Styling: `BASE_CSS`, the stylesheet that edit mode writes by default.
- `prose_core`, the crate underneath, so a host reaches `prose_yew::prose_core::edit`, `prose_yew::prose_core::profile` and `prose_yew::prose_core::validate` without another dependency.

The [Interface](/yew/interface/) page lists every property, field and default.

## Render a policy

`OdrlProse` takes the policy as JSON text and writes `article.ds-prose` with one `section.prose-policy` per policy.

{{source prose-yew/tests/site_examples.rs#reader}}

`POLICY` is a `&str` or an `AttrValue` holding JSON-LD. The output has no styling of its own; [Styling](/yew/styling/) says how to add it.

## Edit a policy

`use_prose_editor` owns the document and the undo history. `OdrlProseView` is controlled: it emits every edit as an event, and the host applies it.

{{source prose-yew/tests/site_examples.rs#editor}}

## Compiled examples

Each block on these pages is a region of `prose-yew/tests/site_examples.rs`. `cargo test -p prose-yew` compiles the file and renders every component on the server, so a snippet that stops compiling fails the build. What the tests check in the browser, in headless Chrome, is `prose-yew/tests/edit_dom.rs`.

## Pages

- [Interface](/yew/interface/): properties, hooks, `EditConfig`, `Labels`.
- [Styling](/yew/styling/): how to inject CSS for the rendering and for the editor.
- [Validation](/yew/validation/): findings, the strict editor, the problems panel.
- [CSS hooks](/yew/css-hooks/): classes, attributes, custom properties, and a complete example stylesheet.
