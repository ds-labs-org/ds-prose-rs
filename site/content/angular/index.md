---
title: Angular integration
label: Overview
description: Install the ds-odrl-prose and ds-odrl-prose-editor Angular components, copy the wasm file into your assets, and render or edit an ODRL policy as sentences.
nav: angular
order: 1
---
# Angular integration

`@ds-labs/prose-angular` has two Angular components. `ds-odrl-prose` renders an ODRL JSON-LD policy as plain-language sentences; `ds-odrl-prose-editor` lets the user edit the same policy in those sentences and hands back the JSON-LD. Both run `prose-yew` compiled to WebAssembly.

The layers are `<ds-odrl-prose [json]>`, then `ProseHandle` from `@ds-labs/prose-wasm`, then `OdrlProse` from `prose-yew`; and `<ds-odrl-prose-editor [json]>`, then `ProseEditorHandle`, then `OdrlProseView` in Edit mode. Yew owns the children of the host element. Use the component as a leaf and do not touch what it renders.

## What you get and what you do not

- `ds-odrl-prose` is read-only. To edit, use `ds-odrl-prose-editor`; see [Editing](#editing). The editor injects its own stylesheet (`BASE_CSS`), so it needs no CSS from you.
- It is client-side only. It reads `document.baseURI` and creates elements with `document`. If your application renders on the server, guard the component with `isPlatformBrowser` or `@defer`.
- It needs Angular 19 or newer (`@angular/core >=19.0.0`, a peer dependency) because it uses the signal API: `input()`, `output()` and `signal()`.
- `ds-odrl-prose` ships no CSS. See [Styling](/angular/styling/).

## Install

```sh
npm install @ds-labs/prose-angular
```

This also installs `@ds-labs/prose-wasm` as a dependency. Both packages are GPL-3.0-only and have the same version.

## Copy the wasm file into your assets

The build does not copy `prose_wasm_bg.wasm`, and the bundler does not either: the loader has to be given a URL. Add this entry to the `assets` array of the `build` target in `angular.json`:

{{file examples/angular-assets.json}}

The demo, `demo-angular`, builds with exactly this entry; a test compares the entry above with its `angular.json`. The component fetches `prose_wasm_bg.wasm` relative to the page's base href. To serve it from another place, provide `PROSE_WASM_URL`; see [Interface](/angular/interface/#prose-wasm-url). To read Turtle profiles, use the Turtle build and its file, `prose_wasm_turtle_bg.wasm`; see [`PROSE_WASM_MODULE`](/angular/interface/#prose-wasm-module).

The entry belongs to the `build` target only. A `test` target that renders the component needs the same entry, or the component shows its load error.

## Use the component

The component is standalone. Import it in the component that uses it.

{{source prose-angular/test/docs-examples.ts#basic}}

The class and its inputs are compiled against the package's public API by `npm run typecheck` in `prose-angular`. The component declares no `styles`, no `encapsulation`, and an empty template; it uses `OnPush` change detection.

## Editing

`ds-odrl-prose-editor` is controlled. `json` is the document, and `jsonChange` emits the JSON-LD the editor writes after each committed edit. Bind back exactly the text it emitted: the editor recognises its own echo and keeps the undo history and the focus. A different text replaces the document and clears the history. Reformatting the emitted text before binding it back counts as a different text.

{{source prose-angular/test/docs-examples.ts#editor}}

The editor reads and writes the bare keys of ODRL JSON-LD (`permission`, `constraint`, ...). If your documents use prefixed keys (`odrl:permission`), convert them before binding and after reading. `limits` sets how many items each list may hold, here one policy and no assigner or assignee. All inputs, outputs and the `report` signal are in [Interface](/angular/interface/#editor-inputs).

## Pages

- [Interface](/angular/interface/): inputs, outputs, the signal, the token, the service.
- [Styling](/angular/styling/): global styles, `articleClass`, view encapsulation.
- [Validation](/angular/validation/): `strict`, `validation`, profiles, reports.
- [CSS hooks](/angular/css-hooks/): the classes, and a complete example stylesheet.
