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

Profiles written in Turtle need the Turtle build: import
`@ds-labs/prose-wasm/turtle` instead, and serve `prose_wasm_turtle_bg.wasm`.
It has the same API and is about 30 KiB larger over the wire (brotli), so it
is opt-in. See Profiles and Serving the wasm.

## Documentation

The [Angular docs](https://ds-labs-org.github.io/ds-prose-rs/angular/) describe the component that wraps this package, including how it loads the wasm file. The [site](https://ds-labs-org.github.io/ds-prose-rs/) has a demo of each side.

## Profiles

`setProfile(text)` loads the ODRL profile the prose is worded with: its
labels replace local names in the sentences, and a glossary of the profile
terms follows each policy. The text is JSON-LD as you fetched it (the
package does no network access), or Turtle with the Turtle build:

```js
import init, { ProseHandle } from '@ds-labs/prose-wasm/turtle';

await init({ module_or_path: new URL('/prose_wasm_turtle_bg.wasm', document.baseURI) });
```

Without it, a Turtle text throws, and the message names the Turtle build.

```js
const summary = JSON.parse(handle.setProfile(profileText));
// { iri, title, version, terms, counts: { action: 3, leftOperand: 3, ... }, warnings: [...] }
handle.setProfile(null); // back to the built-in wording; returns "null"
```

It throws when the text is not a readable profile (an HTML landing page, a
document that holds only a `@context`, a JSON Schema, a policy...); the
message names the likely cause, and the previous profile stays. `update`
keeps the loaded profile.

A profile is third-party input, so reading it is bounded: a text over 4 MiB
is refused before it is parsed, and so is one past the term, nesting or
expansion limits; labels and definitions are made plain text (no markup,
no bidi or control characters) and rendered as text. ODRL's own terms keep
their built-in wording whatever a profile calls them. A profile may suggest
operators and values per left operand through the ds-prose `prose:`
extension (`https://ds-labs-org.github.io/ds-prose-rs/profile#`, a ds-prose
convention, not ODRL); the rendering does not use those suggestions, but
strict validation does when a left operand is marked `prose:closed true`
(see Validation). See the
[ds-prose-rs README](https://github.com/ds-labs-org/ds-prose-rs#profiles).

`setProfiles([text, ...])` loads several at once and returns the summaries as
the JSON text of a list (it replaces what `setProfile` loaded; `[]` clears).

## Validation

`validate(policyJson, profiles, options?)` checks a policy against ODRL
profiles and returns the report as JSON text,
`ds-prose/validation-report@1`. It needs no DOM, so it also runs in Node
after `initSync`. `profiles` is one text, a list of texts or `null`. Strict
mode is the default: the vocabulary is read as closed (ODRL plus the loaded
profiles), and the verdict is `conforms`, `violations`, or `incomplete`
when something could not be checked (never a pass). `{ strict: false }` is
the advisory reading, where nothing is an error.

```js
import { validate, rules, ProseValidator } from '@ds-labs/prose-wasm';

const report = JSON.parse(validate(policyJson, [profileText], { structural: true }));
report.verdict; // 'conforms' | 'violations' | 'incomplete'

const v = new ProseValidator([profileText]); // reads the profiles once
v.validate(policyA); v.validate(policyB);    // JSON text each
JSON.parse(rules());                         // the rule catalogue
```

It throws on options or profiles it cannot read, naming the problem; a
problem in the policy is never thrown, it is a finding
(`input.unreadable`). On a mounted handle, `setValidation(options | null)`
adds the findings and the verdict to the rendering, and `report()` returns
the report for the current policy and profiles (`null` while validation is
off). The types (`ValidateOptions`, `ValidationReport`, `Finding`, `Verdict`
and `RuleId`, the union of the rule ids) are in the package.

The rules, their tiers and what each rests on (the standard, a reading of it
under a closed world, or a ds-prose convention) are in the
[ds-prose-rs README](https://github.com/ds-labs-org/ds-prose-rs#strict-validation);
`rules()` returns the same catalogue as data. The `odrl.*` tier (the
Information Model's structural requirements) is off unless `structural` is
set.

## Command line

The package installs `ds-prose-validate`, for CI:

```bash
npx ds-prose-validate --strict --profile research.ttl policy.json
```

The command runs the Turtle build. Options: `--profile` (repeatable; JSON-LD or Turtle), `--strict`,
`--structural`, `--fail-closed`, `--deny-deprecated`, `--odrl common|core`,
`--ruleset <n>`, `--rule <id>=<level>`, `--format text|json`,
`--allow-incomplete`. A policy of `-` is read from standard input (once);
input must be UTF-8. `--rule` and `--ruleset` work with or without `--strict`. Exit
codes: `0` every policy conforms, `1` violations, `2` usage error or a policy
or profile that cannot be read, `3` no violations but incomplete (`0` with
`--allow-incomplete`). When several apply, 2 outranks 1, and 1 outranks 3.
`--format json` prints a JSON array with a `{file, report, error}` per
policy; each report is the validator's own text, unchanged.

Yew owns the element's children: use it as a leaf and do not touch what it
renders. Style it through the `prose-*` classes (`.ds-prose`, `.prose-policy`,
`.prose-rule`, ...) or the class you pass.

## Serving the wasm

The package ships `prose_wasm_bg.wasm` next to the JavaScript, and
`prose_wasm_turtle_bg.wasm` next to `prose_wasm_turtle.js` (the `./turtle`
entry). Each JavaScript file works only with its own wasm file. Your bundler
does not copy the wasm for you: serve the one you use as a static asset and
pass its URL to `init`. With the Angular CLI, one `angular.json` asset entry
does it:

```json
{ "glob": "prose_wasm_bg.wasm", "input": "node_modules/@ds-labs/prose-wasm", "output": "/" }
```

## What is in the wasm

`package.json` records the `ds-prose-rs` workspace version the wasm was built
from under `dsProse.prose-yew`. The package version is its own (semver).
Licence: Apache-2.0.
