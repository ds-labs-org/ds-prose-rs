# @ds-labs/prose-angular

An Angular component that renders an [ODRL](https://www.w3.org/TR/odrl-model/)
policy, written as JSON-LD, as plain-language prose. It wraps the
`<OdrlProse>` Yew component of
[ds-prose-rs](https://github.com/ds-labs-org/ds-prose-rs), compiled to
WebAssembly (shipped in the `@ds-labs/prose-wasm` dependency). Read-only: no
edit mode.

```
<ds-odrl-prose [json]>  ->  ProseHandle (@ds-labs/prose-wasm)  ->  <OdrlProse> (prose-yew)
```

## Install

```bash
npm i @ds-labs/prose-angular
```

Angular 19 or newer. Client-side only (see Notes).

## Set up

The wasm file is a static asset your build must serve. Add one entry to the
`assets` of your application target in `angular.json`:

```json
{ "glob": "prose_wasm_bg.wasm", "input": "node_modules/@ds-labs/prose-wasm", "output": "/" }
```

That is the whole setup. The wasm is fetched once, on first render.

## Use

```ts
import { Component } from '@angular/core';
import { OdrlProseComponent } from '@ds-labs/prose-angular';

@Component({
  selector: 'app-policy',
  imports: [OdrlProseComponent],
  template: `<ds-odrl-prose [json]="policy" articleClass="my-prose" (loadFailed)="onFailed($event)" />`,
})
export class PolicyComponent {
  policy = '{ "@type": "Set", "permission": [{ "action": "use", "target": "urn:asset:1" }] }';
  onFailed(error: unknown) { /* report it */ }
}
```

| | | |
|---|---|---|
| `json` | input, required | the ODRL policy as JSON-LD text |
| `articleClass` | input | extra class(es) for the rendered `<article class="ds-prose">` |
| `loadFailed` | output | emits the error when the wasm cannot be loaded |
| `PROSE_WASM_URL` | injection token | where the browser fetches the wasm from, relative to the page's base href (default `prose_wasm_bg.wasm`) |

Invalid JSON shows an error message in place of the prose; it recovers on the
next valid input. Style the result through the `prose-*` classes
(`.ds-prose`, `.prose-policy`, `.prose-rule`, `.prose-permission`,
`.prose-prohibition`, `.prose-obligation`, `.prose-error`, ...) or the
`articleClass` input.

If the wasm cannot be loaded (a wrong `PROSE_WASM_URL`, a 404, a network
failure) the element shows an error (`<p class="prose-error" role="alert">`),
logs it with `console.error`, and emits it from `loadFailed`. A failed load is
not remembered: the next change of an input tries again.

## Notes

- The component owns its host element's children (Yew keeps its own virtual
  DOM there). Use it as a leaf.
- It renders client-side only. If your app uses SSR, guard it with
  `isPlatformBrowser` or `@defer`.
- The package version is independent of the Rust crates. The ds-prose-rs
  version of the wasm is recorded in `@ds-labs/prose-wasm`'s `package.json`
  under `dsProse.prose-yew`.

## Replacing a vendored copy

If you copied this project into your repository and built it yourself, you can
delete the copy, the vendoring script, the `tsconfig` path to it and the lint
and formatter ignores for it, install the package above, and keep only the one
`angular.json` asset line.

Licence: Apache-2.0.

## Developing this package

```bash
scripts/build-npm-packages.sh      # from the repository root: both packages, packed
cd prose-angular && npm test && npm run typecheck
```
