# @ds-labs/prose-wasm

Renders an [ODRL](https://www.w3.org/TR/odrl-model/) policy, written as
JSON-LD, as plain-language prose. This is the `<OdrlProse>` Yew component of
[ds-prose-rs](https://github.com/ds-labs-org/ds-prose-rs), compiled to
WebAssembly and mounted into a DOM element you give it. Read-only: there is
no edit mode in this package.

Framework-neutral. For Angular use
[`@ds-labs/prose-angular`](https://www.npmjs.com/package/@ds-labs/prose-angular),
which wraps this.

```js
import init, { ProseHandle } from '@ds-labs/prose-wasm';

await init({ module_or_path: new URL('/prose_wasm_bg.wasm', document.baseURI) });
const handle = new ProseHandle(document.getElementById('policy'), policyJsonText, 'my-class');
handle.update(otherPolicyJsonText, 'my-class'); // re-render
handle.destroy();                               // unmount
```

## Profiles

`setProfile(text)` loads the ODRL profile the prose is worded with: its
labels replace local names in the sentences, and a glossary of the profile
terms follows each policy. The text is JSON-LD or Turtle, as you fetched it
(the package does no network access).

```js
const summary = JSON.parse(handle.setProfile(profileText));
// { iri, title, version, terms, counts: { action: 3, leftOperand: 3, ... }, warnings: [...] }
handle.setProfile(null); // back to the built-in wording; returns "null"
```

It throws when the text is not a readable profile (an HTML landing page, a
document that holds only a `@context`, a JSON Schema, a policy...); the
message names the likely cause, and the previous profile stays. `update`
keeps the loaded profile.

Yew owns the element's children: use it as a leaf and do not touch what it
renders. Style it through the `prose-*` classes (`.ds-prose`, `.prose-policy`,
`.prose-rule`, ...) or the class you pass.

## Serving the wasm

The package ships `prose_wasm_bg.wasm` next to the JavaScript. Your bundler
does not copy it for you: serve it as a static asset and pass its URL to
`init`. With the Angular CLI, one `angular.json` asset entry does it:

```json
{ "glob": "prose_wasm_bg.wasm", "input": "node_modules/@ds-labs/prose-wasm", "output": "/" }
```

## What is in the wasm

`package.json` records the `ds-prose-rs` workspace version the wasm was built
from under `dsProse.prose-yew`. The package version is its own (semver).
Licence: Apache-2.0.
