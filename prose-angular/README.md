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
| `profile` | input | the ODRL profile the prose is worded with, as JSON-LD or Turtle text (`null` or blank: none) |
| `profiles` | input | several profiles, as text each; a list wins over `profile` (an empty list clears them) |
| `strict` | input | validate with a closed vocabulary: ODRL plus the loaded profiles (default `false`) |
| `validation` | input | the full `ValidateOptions`; takes precedence over `strict` |
| `showViolations` | input | also show the findings and the verdict under the prose (needs `strict` or `validation`) |
| `validationChange` | output | emits the `ValidationReport` each time its verdict or findings change |
| `validationError` | output | emits the error when `validation` is not readable options; the previous options stay |
| `report` | signal | the latest `ValidationReport`, or `null` while validation is off |
| `loadFailed` | output | emits the error when the wasm cannot be loaded |
| `profileLoaded` | output | emits a `ProfileSummary` (`iri`, `title`, `version`, `terms`, `counts`, `warnings`) for each profile loaded |
| `profileError` | output | emits the error when `profile` is not a readable profile; the previous profile stays |
| `PROSE_WASM_URL` | injection token | where the browser fetches the wasm from, relative to the page's base href (default `prose_wasm_bg.wasm`) |

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

Licence: Apache-2.0.

## Developing this package

```bash
scripts/build-npm-packages.sh      # from the repository root: both packages, packed
cd prose-angular && npm test && npm run typecheck
```
