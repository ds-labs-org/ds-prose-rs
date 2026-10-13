---
title: Read an ODRL policy as sentences
description: ds-prose-rs reads an ODRL JSON-LD policy and renders it as plain-language sentences, in Yew and in Angular.
nav: home
stylesheet: examples/prose.css
---
# Read an ODRL policy as sentences

**PROSE: Prose Rendering for ODRL Semantic Representation.** ds-prose-rs reads an ODRL JSON-LD policy and writes it as plain-language sentences that say who may, must not or must do what, and under which conditions. It is a Rust library with a Yew component, a WebAssembly bridge, and an Angular component. It does not decide whether an action is allowed, and it ships no stylesheet for the rendered sentences.

## A policy and its sentences

The policy below is a fixture of the repository. The sentences under it are the renderer's output for that policy, stored in `site/content/examples/offer.html`. A Rust test renders the policy again on every `cargo test` and fails if the stored output is stale. The page styles them with the example stylesheet from the CSS hooks pages.

### Input

{{file examples/offer.jsonld}}

### Rendered

{{rendered examples/offer.html}}

## Where to go

- [Yew demo](/demo/): the component in the browser, with the editor and the strict validation.
- [Angular demo](/demo-angular/): the read-only Angular component.
- [Yew documentation](/yew/): install, interface, styling, validation, CSS hooks.
- [Angular documentation](/angular/): install, interface, styling, validation, CSS hooks.

## Install

```sh
cargo add prose-yew
```

```sh
npm install @ds-labs/prose-angular
```

## Where it runs

| Package | Version | What it exposes |
|---|---|---|
| `prose-yew` (crate) | 0.4.0 | `OdrlProse`, `OdrlProseView`, `ProseGlossary`, `ProseProblems`, the editor hooks and `BASE_CSS`. |
| `@ds-labs/prose-wasm` (npm) | 0.4.0 | `ProseHandle` for the read-only renderer, `ProseValidator`, `validate`, `rules`, and the `ds-prose-validate` command. No editor. |
| `@ds-labs/prose-angular` (npm) | 0.4.0 | The `ds-odrl-prose` component and `ProseValidatorService`. Read-only, client-side only. |
