---
title: Angular integration
label: Overview
description: Install the ds-odrl-prose Angular component, copy its wasm file into your assets, and render an ODRL policy as sentences.
nav: angular
order: 1
---
# Angular integration

`@ds-labs/prose-angular` is an Angular component, `ds-odrl-prose`, that renders an ODRL JSON-LD policy as plain-language sentences. It runs the Rust renderer of `prose-yew` compiled to WebAssembly.

The layers are `<ds-odrl-prose [json]>`, then `ProseHandle` from `@ds-labs/prose-wasm`, then `OdrlProse` from `prose-yew`. Yew owns the children of the host element. Use the component as a leaf and do not touch what it renders.

## What you get and what you do not

- The component is read-only. There is no edit mode and no editor in the Angular package. The Yew editor (`OdrlProseView`, `ProseEditor`, `BASE_CSS`) is not exposed by `@ds-labs/prose-angular` or by `@ds-labs/prose-wasm`. For editing, use the [Yew component](/yew/).
- It is client-side only. It reads `document.baseURI` and creates elements with `document`. If your application renders on the server, guard the component with `isPlatformBrowser` or `@defer`.
- It needs Angular 19 or newer (`@angular/core >=19.0.0`, a peer dependency) because it uses the signal API: `input()`, `output()` and `signal()`.
- It ships no CSS. See [Styling](/angular/styling/).

## Install

```sh
npm install @ds-labs/prose-angular
```

This also installs `@ds-labs/prose-wasm` as a dependency. Both packages are Apache-2.0 and have the same version.

## Copy the wasm file into your assets

The build does not copy `prose_wasm_bg.wasm`, and the bundler does not either: the loader has to be given a URL. Add this entry to the `assets` array of the `build` target in `angular.json`:

{{file examples/angular-assets.json}}

The demo, `demo-angular`, builds with exactly this entry; a test compares the entry above with its `angular.json`. The component fetches `prose_wasm_bg.wasm` relative to the page's base href. To serve it from another place, provide `PROSE_WASM_URL`; see [Interface](/angular/interface/#prose-wasm-url). To read Turtle profiles, use the Turtle build and its file, `prose_wasm_turtle_bg.wasm`; see [`PROSE_WASM_MODULE`](/angular/interface/#prose-wasm-module).

The entry belongs to the `build` target only. A `test` target that renders the component needs the same entry, or the component shows its load error.

## Use the component

The component is standalone. Import it in the component that uses it.

{{source prose-angular/test/docs-examples.ts#basic}}

The class and its inputs are compiled against the package's public API by `npm run typecheck` in `prose-angular`. The component declares no `styles`, no `encapsulation`, and an empty template; it uses `OnPush` change detection.

## Pages

- [Interface](/angular/interface/): inputs, outputs, the signal, the token, the service.
- [Styling](/angular/styling/): global styles, `articleClass`, view encapsulation.
- [Validation](/angular/validation/): `strict`, `validation`, profiles, reports.
- [CSS hooks](/angular/css-hooks/): the classes, and a complete example stylesheet.
