# @ds-labs/prose-angular

An Angular component that renders an [ODRL](https://www.w3.org/TR/odrl-model/)
policy, written as JSON-LD, as plain-language prose. It wraps the
`<OdrlProse>` Yew component of
[ds-prose-rs](https://github.com/ds-labs-org/ds-prose-rs), compiled to
WebAssembly (shipped in the `@ds-labs/prose-wasm` dependency).
`ds-odrl-prose` is read-only; `ds-odrl-prose-editor` edits the same policy in
its sentences (see Editing).

```
<ds-odrl-prose [json]>         ->  ProseHandle (@ds-labs/prose-wasm)        ->  <OdrlProse> (prose-yew)
<ds-odrl-prose-editor [json]>  ->  ProseEditorHandle (@ds-labs/prose-wasm)  ->  <OdrlProseView> in Edit mode (prose-yew)
```

## Documentation and demo

The [Angular docs](https://ds-labs-org.github.io/ds-prose-rs/angular/) cover the interface, styling, validation and the CSS hooks. The [Angular demo](https://ds-labs-org.github.io/ds-prose-rs/demo-angular/) runs the component with `strict`, `profile` and `validationChange`.

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
| `profile` | input | the ODRL profile the prose is worded with, as JSON-LD text, or Turtle with the Turtle build (`null` or blank: none) |
| `profiles` | input | several profiles, as text each; a list wins over `profile` (an empty list clears them) |
| `shapes` | input | the dataspace's companion SHACL shapes as text, JSON-LD or Turtle with the Turtle build (`null` or blank: none); a value a shape's `sh:in` does not list is a finding with `expected.source` `"shapes"` |
| `overlays` | input | the dataspace's operator overlays as text each (`prose:` hints about terms the profile declares; JSON-LD, or Turtle with the Turtle build); blank texts are dropped. Shapes win over an overlay; a finding an overlay closed has `expected.source` `"overlay"` |
| `valueSources` | input | values the host took from its registry or catalog (participant DIDs, credential types, asset ids, purposes), keyed by left operand IRI or by party role (`assigner`, `assignee`, `target`): a list of values (`string` or `{ value, label, group }`), which only suggests, or `{ values, closed }`. They sit on top of the profile, overlays and shapes; a finding a closed source caused has `expected.source` `"host"`, and a value it no longer lists stays in the policy |
| `strict` | input | validate with a closed vocabulary: ODRL plus the loaded profiles (default `false`) |
| `validation` | input | the full `ValidateOptions`; takes precedence over `strict` |
| `showViolations` | input | also show the findings and the verdict under the prose (needs `strict` or `validation`) |
| `validationChange` | output | emits the `ValidationReport` each time its verdict or findings change |
| `validationError` | output | emits the error when `validation` is not readable options; the previous options stay |
| `report` | signal | the latest `ValidationReport`, or `null` while validation is off |
| `loadFailed` | output | emits the error when the wasm cannot be loaded |
| `profileLoaded` | output | emits a `ProfileSummary` (`iri`, `title`, `version`, `terms`, `counts`, `warnings`) for each profile loaded |
| `profileError` | output | emits the error when `profile` is not a readable profile; the previous profile stays |
| `overlaysLoaded` | output | emits an `OverlaySummary[]` (`terms`, `warnings`, one per overlay) when `overlays` changed to overlays that were read |
| `overlaysError` | output | emits the error when an overlay is not readable; the previous overlays stay |
| `valueSourcesLoaded` | output | emits a `Record<string, ValueSourceSummary>` (`values`, `closed`, `warnings`, one per key) when `valueSources` changed to sources that were read |
| `valueSourcesError` | output | emits the error when `valueSources` cannot be read; the previous sources stay |
| `shapesLoaded` | output | emits a `ShapesSummary` (`constraints`, `actions`, `leftOperands`, `warnings`) when `shapes` changed to shapes that were read |
| `shapesError` | output | emits the error when `shapes` is not readable shapes; the previous shapes stay |
| `PROSE_WASM_URL` | injection token | where the browser fetches the wasm from, relative to the page's base href (default `prose_wasm_bg.wasm`) |
| `PROSE_WASM_MODULE` | injection token | which build of `@ds-labs/prose-wasm` runs: by default JSON-LD profiles only; `() => import('@ds-labs/prose-wasm/turtle')`, with `PROSE_WASM_URL` set to `prose_wasm_turtle_bg.wasm`, reads Turtle as well |

Invalid JSON shows an error message in place of the prose; it recovers on the
next valid input. Style the result through the `prose-*` classes
(`.ds-prose`, `.prose-policy`, `.prose-rule`, `.prose-permission`,
`.prose-prohibition`, `.prose-obligation`, `.prose-error`, ...) or the
`articleClass` input.

With a profile, the sentences use its labels ("may fine-tune" rather than
"may fine tune") and a glossary of its terms follows each policy. The
component does no network access: fetch the profile yourself and pass its
text. It is read again only when the text changes. ODRL's own terms keep
their built-in wording whatever a profile calls them, and a profile over
4 MiB, or past the reader's other limits, is refused (`profileError`). See
the [ds-prose-rs README](https://github.com/ds-labs-org/ds-prose-rs#profiles)
for what a profile changes and which formats are read.


## Editing

```ts
import { Component, signal } from '@angular/core';
import { OdrlProseEditorComponent, type EditLimits } from '@ds-labs/prose-angular';

@Component({
  selector: 'app-editor',
  imports: [OdrlProseEditorComponent],
  template: `<ds-odrl-prose-editor [json]="policy()" [limits]="limits" (jsonChange)="policy.set($event)" />`,
})
export class EditorComponent {
  policy = signal('{ "@type": "Set", "permission": [{ "action": "use", "target": "urn:asset:1" }] }');
  limits: EditLimits = { policyAssigner: { max: 0 }, policyAssignee: { max: 0 } };
}
```

`json` is controlled: `jsonChange` emits the JSON-LD the editor writes after
each committed edit, and you bind back exactly that text. The editor
recognises its own echo and keeps the undo history and the focus; any other
text replaces the document and clears the history, so do not reformat the
emitted text. The editor reads and writes the bare keys of ODRL JSON-LD
(`permission`, not `odrl:permission`). It injects its own stylesheet.

| | | |
|---|---|---|
| `json` | input, required | the ODRL policy as JSON-LD text; unreadable text emits `jsonError` |
| `articleClass`, `profile`, `profiles`, `strict`, `validation` | inputs | as for `ds-odrl-prose`; with validation on, the editor shows inline markers and a problems panel with fix buttons |
| `limits` | input | how many items each list may hold, by name (`{ min?, max? }`); a given entry replaces the default for that list |
| `termInput` | input | `'free'` (the default) or `'select'`: the term slots (action, left operand, policy type, and a unit or right operand of a left operand the profile closes) become dropdowns of the loaded profiles' terms |
| `refuseTerms` | input | `true`: under validation, an edit that writes a term the loaded profiles do not define is refused instead of committed and flagged (default `false`, or the preset's) |
| `preset` | input | `'dataspace'` sets strict and structural validation, `termInput` `'select'`, `refuseTerms`, `limits` of `{ logicalDepth: { max: 2 } }` and the lints `heuristic.single-child-group` and `heuristic.duplicate-action` as warnings; each of `strict`, `validation`, `termInput`, `refuseTerms` and `limits` you set yourself wins |
| `outOfScope` | input | `'show'` (the default), `'strip'` or `'reject'`: what to do with a section whose limit is `max: 0` (say the assigner and assignee of a policy definition) when `json` holds one, in any spelling of its key. `'strip'` drops it, `outOfScopeRemoved` emits what went and `jsonChange` emits the stripped document (so a bound `json` drops it too); `'reject'` fails the load through `jsonError` with the offending `paths` |
| `jsonChange` | output | the JSON-LD text after each committed edit |
| `outOfScopeRemoved` | output | the `{ path, key, limit }` of each section `'strip'` dropped |
| `jsonError`, `loadFailed`, `profileLoaded`, `profileError`, `validationChange`, `validationError` | outputs | as for `ds-odrl-prose` (`jsonError`: the `json` input is not a readable policy) |
| `report` | signal | the latest validation report, or `null` |

## Validation

Without `strict` or `validation` nothing is validated and the component
renders exactly as before. With `[strict]="true"` the policy is checked
against the loaded profiles with the vocabulary read as closed: ODRL plus
those profiles is every term there is, so a term that resolves nowhere is an
error. The report is `ds-prose/validation-report@1`, the same JSON the Rust
crates and the `ds-prose-validate` command give.

```html
<ds-odrl-prose [json]="policy" [profiles]="[researchProfile, otherProfile]"
               [strict]="true" [showViolations]="true"
               (validationChange)="onReport($event)" />
```

```ts
onReport(r: ValidationReport) {
  // r.verdict is 'conforms', 'violations' or 'incomplete'.
  // 'incomplete' is never a pass: something could not be checked, for
  // example a profile the policy declares that is not loaded.
  this.canPublish = r.verdict === 'conforms';
}
```

`validationChange` is deduplicated: it fires when the verdict or the set of
findings changes, not on every edit that leaves them as they were.
`[validation]` takes the other options (`structural`, `failClosed`,
`denyDeprecated`, `odrl`, `ruleset`, per-rule levels); see `ValidateOptions`
in the types. Which rules exist, and what each rests on (the standard, or a
ds-prose convention), is in the
[ds-prose-rs README](https://github.com/ds-labs-org/ds-prose-rs#strict-validation).

To validate without rendering, inject `ProseValidatorService`:
`await service.validate(policyJson, [profileText])` returns the report, and
rejects only when the wasm cannot load or a profile or the options cannot be
read.

If the wasm cannot be loaded (a wrong `PROSE_WASM_URL`, a 404, a network
failure) the element shows an error (`<p class="prose-error" role="alert">`),
logs it with `console.error`, and emits it from `loadFailed`. A failed load is
not remembered: the next change of an input tries again.

## Notes

- The component owns its host element's children (Yew keeps its own virtual
  DOM there). Use it as a leaf.
- It renders client-side only. If your app uses SSR, guard it with
  `isPlatformBrowser` or `@defer`.
- The package version follows the Rust crates' (and `@ds-labs/prose-wasm`'s).
  The ds-prose-rs version of the wasm is also recorded in
  `@ds-labs/prose-wasm`'s `package.json` under `dsProse.prose-yew`.

## Replacing a vendored copy

If you copied this project into your repository and built it yourself, you can
delete the copy, the vendoring script, the `tsconfig` path to it and the lint
and formatter ignores for it, install the package above, and keep only the one
`angular.json` asset line.

Licence: GPL-3.0-only.

## Developing this package

```bash
scripts/build-npm-packages.sh      # from the repository root: both packages, packed
cd prose-angular && npm test && npm run typecheck
```
