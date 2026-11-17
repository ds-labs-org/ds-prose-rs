---
title: Angular interface
label: Interface
description: Every input, output, signal, token and service of the ds-odrl-prose Angular component, with types and defaults taken from the source.
nav: angular
order: 2
---
# Angular interface

The tables on this page are checked against `prose-angular/src/`. A test reads the `input()`, `output()` and `signal()` members of `OdrlProseComponent` and of `OdrlProseEditorComponent` and fails when a name, a type or a default differs from what is written here.

The component is `ds-odrl-prose`, a standalone component with an empty template and `OnPush` change detection. It declares no `styles` and no `encapsulation`. It implements `OnChanges` and `OnDestroy`, and every input change runs one synchronisation with the wasm module.

## Inputs

| Input | Type | Default | Meaning |
|---|---|---|---|
| `json` | `string` | required | The ODRL policy as JSON-LD text. A policy that cannot be read renders a `p.prose-error` with `role="alert"` inside the article, and the next valid input recovers. |
| `articleClass` | `string` | `undefined` | Extra classes, separated by spaces, for the rendered `article.ds-prose`. Nothing else receives them. |
| `profile` | `string \| null` | `null` | One ODRL profile as JSON-LD text, or Turtle with the [Turtle build](#prose-wasm-module). Blank text means no profile. The component does no network access: fetch the text yourself. |
| `profiles` | `string[] \| null` | `null` | Several profiles. A list wins over `profile`, and an empty list clears the profile. |
| `strict` | `boolean` | `false` | Validate with a closed vocabulary: ODRL plus the loaded profiles. |
| `validation` | `ValidateOptions \| null` | `null` | The full validation options. They win over `strict`. |
| `showViolations` | `boolean` | `false` | Show the findings and the verdict under the prose. Effective only when `strict` or `validation` gives options. |

Validation is covered on [its own page](/angular/validation/).

## Outputs

| Output | Payload | Emitted when |
|---|---|---|
| `loadFailed` | `unknown` | The wasm module cannot be loaded. The next input change tries again. |
| `profileLoaded` | `ProfileSummary` | Once per profile read, after the `profile` or `profiles` input changed. |
| `profileError` | `unknown` | A profile text cannot be read. The previous profile stays. |
| `validationChange` | `ValidationReport` | The verdict or the set of findings changed. The first report counts as a change. Never emitted for no report. |
| `validationError` | `unknown` | The `validation` options cannot be read, or building the report threw. The previous options stay. |

The work of the wasm module runs outside the Angular zone (`NgZone.runOutsideAngular`), so Yew's own scheduling does not trigger change detection. The report is published back inside the zone (`NgZone.run`).

## Signal

| Member | Type | Initial | Meaning |
|---|---|---|---|
| `report` | `WritableSignal<ValidationReport \| null>` | `null` | The latest report. `null` while validation is off. |

The declared type is writable, because the source declares `readonly report = signal<...>(null)`. Treat it as read-only: read it, do not set it. The component overwrites it on every synchronisation.

{{source prose-angular/test/docs-examples.ts#report_signal}}

## Editor

`ds-odrl-prose-editor` is a second standalone component with an empty template and `OnPush` change detection. It declares no `styles` and no `encapsulation`; the editor writes its own stylesheet (`BASE_CSS`) into the host. It is controlled: see [Editing](/angular/#editing). It uses the same `PROSE_WASM_URL` and `PROSE_WASM_MODULE` tokens and the same wasm file as `ds-odrl-prose`. Its class names are the [Yew editor hooks](/yew/css-hooks/).

## Editor inputs

| Input | Type | Default | Meaning |
|---|---|---|---|
| `json` | `string` | required | The ODRL policy as JSON-LD text. Controlled: bind back what `jsonChange` emits; the echo is ignored. A text that cannot be read emits `jsonError`. |
| `articleClass` | `string` | `undefined` | Extra classes, separated by spaces, for the rendered `article.ds-prose.prose-edit`. |
| `profile` | `string \| null` | `null` | One ODRL profile as JSON-LD text, or Turtle with the [Turtle build](#prose-wasm-module). The editor then words the sentences with it and suggests its terms. |
| `profiles` | `string[] \| null` | `null` | Several profiles. A list wins over `profile`. |
| `limits` | `EditLimits \| null` | `null` | How many items each list may hold, by list name; a given entry replaces that list's default (`min` 0, `max` unbounded). `{ policyAssigner: { max: 0 } }` removes the assigner button. Unreadable limits are logged and the previous ones stay. |
| `strict` | `boolean \| undefined` | `undefined` | Validate with a closed vocabulary: ODRL plus the loaded profiles. Not set means `false`, or the `preset`'s. |
| `validation` | `ValidateOptions \| null` | `null` | The full validation options. They win over `strict`. While validation is on, the editor marks issues inline and shows a problems panel with fix buttons; edits are flagged, never refused. |
| `refuseTerms` | `boolean \| undefined` | `undefined` | `true`: under validation (`strict`), an edit that writes a term the loaded profiles do not define into an action, left operand, operator, unit, conflict strategy or policy type is refused and shown at the slot, and never reaches `jsonChange` (`StrictEdit::RefuseTerms`). Not set means `false` (the edit is committed and flagged), or the `preset`'s. |
| `preset` | `'dataspace' \| null` | `null` | One switch for a dataspace whose members write only policies it can evaluate. `'dataspace'` sets: strict validation (a closed vocabulary: ODRL plus the loaded profiles) with the structural tier on, so `odrl.rule-target` and the other structural rules apply; `termInput` `'select'`; `refuseTerms`; `limits` of `{ logicalDepth: { max: 2 } }`; and `heuristic.single-child-group` and `heuristic.duplicate-action` as warnings. Whatever you set yourself wins: `strict`, `termInput`, `refuseTerms`, and `validation` key by key (its `rules` are added to the preset's) and `limits` list by list. An unknown name is logged and ignored. See [Running in a dataspace](/angular/dataspace/). |
| `outOfScope` | `OutOfScopeMode` | `'show'` | What to do with a section whose limit is `max: 0` when `json` holds one, so that limits are the scope of the incoming document and not only of the add buttons. `'show'` keeps and shows it. `'strip'` drops it on load and on every later `json`: the policy-level and rule-level (duty included) assigner and assignee, and any other list capped at 0, in the bare, prefixed (`odrl:assigner`) and full-IRI spellings of the key; `outOfScopeRemoved` emits what went, and `jsonChange` never holds it. `'reject'` refuses a document that holds one: `jsonError` emits an `OutOfScopeError` whose `paths` name the sections, at the first mount nothing is mounted. Use it where a host accepts one kind of document, such as policy definitions without parties: `[limits]="{ policyAssigner: { max: 0 }, policyAssignee: { max: 0 }, ruleAssigner: { max: 0 }, ruleAssignee: { max: 0 } }" outOfScope="strip"`. |
| `termInput` | `TermInput \| undefined` | `undefined` | Not set means `'free'`, or the `preset`'s. `'select'` turns the term slots into dropdowns of the terms the loaded profiles define: the action, the left operand and the policy type always, a unit or a right operand only when the profile closes the left operand (`prose:closed`) and lists units or values for it. Other values, such as dates and numbers, stay free text. Options show the profile label, grouped by profile; with strict validation only what it takes is listed. Deprecated terms are listed last, or hidden when `denyDeprecated` makes them an error. A stored value outside the list stays selected and is flagged with `data-outside="true"`. `'free'` keeps typed text with suggestions. |

## Editor outputs

| Output | Payload | Emitted when |
|---|---|---|
| `jsonChange` | `string` | After each committed edit, with the JSON-LD the editor writes. Not emitted for a document the host set through `json`. |
| `outOfScopeRemoved` | `RemovedSection[]` | `outOfScope="strip"` dropped sections from the document, each time it did. Each entry has the `path` (`permission[0].assignee`), the `key` as written and the `limit` that left it out (`ruleAssignee`). |
| `jsonError` | `unknown` | `json` is not a readable ODRL policy, or `outOfScope="reject"` refuses it (an `OutOfScopeError` with `paths`). At the first mount nothing is mounted and the next input change tries again; later the document on screen stays. |
| `loadFailed` | `unknown` | The wasm module cannot be loaded. The next input change tries again. |
| `profileLoaded` | `ProfileSummary` | Once per profile read, after the `profile` or `profiles` input changed. |
| `profileError` | `unknown` | A profile text cannot be read. The previous profile stays. |
| `validationChange` | `ValidationReport` | The verdict or the set of findings changed, also after an edit. Never emitted for no report. |
| `validationError` | `unknown` | The `validation` options cannot be read, or building the report threw. The previous options stay. |

## Editor signal

| Member | Type | Initial | Meaning |
|---|---|---|---|
| `report` | `WritableSignal<ValidationReport \| null>` | `null` | The latest report. `null` while validation is off. Read it, do not set it. |

`EditLimits` maps a list name (`policies`, `permission`, `prohibition`, `obligation`, `duty`, `remedy`, `consequence`, `policyAssigner`, `policyAssignee`, `policyTarget`, `policyAction`, `policyUid`, `ruleAssigner`, `ruleAssignee`, `ruleTarget`, `ruleAction`, `actionRefinement`, `entityRefinement`, `constraints`, `logicalChildren`, `logicalDepth`, `rightOperandValues`, `profile`, `inheritFrom`) to `{ min?: number; max?: number | null }`. `logicalDepth` is not a list: its `max` is how deep `and`, `or` and `xone` groups may nest, and the editor hides the buttons that would go past it. A host that caps `policyUid` at 0 (the identifier lives elsewhere) should also set the `odrl.policy-uid` rule to `"off"` in `validation.rules`: the rule would otherwise report a policy without an identifier that nobody can set.

The wasm work runs outside the zone and `jsonChange` and `validationChange` are emitted inside it, as for `ds-odrl-prose`. When the wasm cannot be loaded, the host shows a `p.prose-error` with `role="alert"`: "The ODRL prose editor could not be loaded (the URL)."

## `PROSE_WASM_URL`

`PROSE_WASM_URL` is an `InjectionToken<string>` provided in root. Its default is `'prose_wasm_bg.wasm'`. The component resolves it against `document.baseURI`, which is the page's `<base href>`, and fetches the file once per page, on first render. A failed load is not remembered, so the next input change retries.

To serve the file from another place, provide the token:

{{source prose-angular/test/docs-examples.ts#wasm_url}}

## `PROSE_WASM_MODULE`

`PROSE_WASM_MODULE` is an `InjectionToken<() => Promise<ProseWasmModule>>` provided in root: the function that imports the build of `@ds-labs/prose-wasm` the component and the service run. The default, `() => import('@ds-labs/prose-wasm')`, reads profiles written in JSON-LD. The bundler puts that module in its own chunk, loaded on first render.

To read Turtle profiles as well, provide the Turtle build and its wasm file. That file is about 30 KiB larger over the wire (brotli), so it is opt-in:

{{source prose-angular/test/docs-examples.ts#turtle}}

Copy `prose_wasm_turtle_bg.wasm` into your assets with the same kind of entry as below, with that file name in `glob`. Without the Turtle build, a Turtle profile emits `profileError` with a message that names the Turtle build.

## `ProseValidatorService`

`ProseValidatorService` is provided in root. It validates a policy against profiles without rendering anything: a submit guard, a list of policies. It runs the same engine as `[strict]` and as the `ds-prose-validate` command, so the report is the same.

```ts
async validate(
  json: string,
  profiles: string | readonly string[] | null = null,
  options: ValidateOptions | null = null,
): Promise<ValidationReport>
```

It waits for the wasm module and does no network access. It is strict unless `options.strict` is `false`. It rejects when the wasm cannot be loaded, or when a profile or the options cannot be read. A problem in the policy is a finding in the report, never a rejection.

{{source prose-angular/test/docs-examples.ts#service}}

It loads the wasm through `document.baseURI`, so it is browser-only like the component. To validate in Node, use `initSync` and `validate` from `@ds-labs/prose-wasm` directly.

## Load error

When the wasm module cannot be loaded, the component logs `ds-odrl-prose: could not load the wasm module` with `console.error` and emits `loadFailed`. If nothing is mounted yet, it also replaces the children of the host with a `p.prose-error` with `role="alert"` and this text, where the URL is the value of `PROSE_WASM_URL`:

> The ODRL prose renderer could not be loaded (the URL). Check that the file is served; it is tried again when the input changes.

A tree that is already mounted is left alone. The error paragraph is a child of the host, outside `article.ds-prose`: select it as `ds-odrl-prose > .prose-error`, as [CSS hooks](/angular/css-hooks/) says.

## Exports

The package exports exactly these names from its entry point. Nothing else is public: the loader and the sync helpers are internal.

| Name | Kind | Meaning |
|---|---|---|
| `OdrlProseComponent` | class | The read-only component. |
| `OdrlProseEditorComponent` | class | The editing component. |
| `PROSE_WASM_URL` | token | Where the wasm file is fetched from. |
| `PROSE_WASM_MODULE` | token | Which build of `@ds-labs/prose-wasm` runs: JSON-LD only (default) or with Turtle. |
| `ProseWasmModule` | type | The JavaScript module of `@ds-labs/prose-wasm`, either build. |
| `ProseValidatorService` | class | Validation without rendering. |
| `ProfileSummary` | type | What `profileLoaded` emits. |
| `DATASPACE_PRESET` | constant | What `preset="dataspace"` sets: the validation options, `termInput`, `limits` and `refuseTerms`, as one object. |
| `Preset` | type | The shape of `DATASPACE_PRESET`. |
| `reportChanged` | function | Whether a report differs from the previous one in verdict or findings. This is the rule behind `validationChange`. |
| `ReportLike` | type | What `reportChanged` reads: a `verdict` and optional findings with a `fingerprint` and a `severity`. |
| `Finding` | type | One finding, from `@ds-labs/prose-wasm`. |
| `RuleId` | type | A frozen rule id, from `@ds-labs/prose-wasm`. |
| `RuleInfo` | type | One entry of the rule catalogue, from `@ds-labs/prose-wasm`. |
| `EditLimits` | type | The `limits` input of the editor, from `@ds-labs/prose-wasm`. |
| `OutOfScopeMode` | type | The `outOfScope` input of the editor, `"show"`, `"strip"` or `"reject"`, from `@ds-labs/prose-wasm`. |
| `RemovedSection` | type | What `outOfScopeRemoved` emits per section, from `@ds-labs/prose-wasm`. |
| `TermInput` | type | The `termInput` input of the editor, `"free"` or `"select"`, from `@ds-labs/prose-wasm`. |
| `ValidateOptions` | type | The validation options, from `@ds-labs/prose-wasm`. |
| `ValidationReport` | type | The report, from `@ds-labs/prose-wasm`. |
| `Verdict` | type | `"conforms"`, `"violations"` or `"incomplete"`. |

## `ProfileSummary`

| Field | Type | Meaning |
|---|---|---|
| `iri` | `string \| null` | The IRI of the profile. |
| `title` | `string \| null` | Its title. |
| `version` | `string \| null` | Its version. |
| `terms` | `number` | How many terms it defines. |
| `counts` | `Record<string, number>` | One entry per kind of term present, for example `action`, `leftOperand`, `operator`. |
| `warnings` | `string[]` | What the reader noted while reading the profile. |

## Compiled examples

Every TypeScript block in these pages is a region of `prose-angular/test/docs-examples.ts`, which `npm run typecheck` compiles against the package. The templates are strings there, so the Angular compiler checks them only in an application.

{{source prose-angular/test/docs-examples.ts#profile}}
