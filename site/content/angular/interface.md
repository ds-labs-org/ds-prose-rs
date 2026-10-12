---
title: Angular interface
label: Interface
description: Every input, output, signal, token and service of the ds-odrl-prose Angular component, with types and defaults taken from the source.
nav: angular
order: 2
---
# Angular interface

The tables on this page are checked against `prose-angular/src/`. A test reads the `input()`, `output()` and `signal()` members of `OdrlProseComponent` and fails when a name, a type or a default differs from what is written here.

The component is `ds-odrl-prose`, a standalone component with an empty template and `OnPush` change detection. It declares no `styles` and no `encapsulation`. It implements `OnChanges` and `OnDestroy`, and every input change runs one synchronisation with the wasm module.

## Inputs

| Input | Type | Default | Meaning |
|---|---|---|---|
| `json` | `string` | required | The ODRL policy as JSON-LD text. A policy that cannot be read renders a `p.prose-error` with `role="alert"` inside the article, and the next valid input recovers. |
| `articleClass` | `string` | `undefined` | Extra classes, separated by spaces, for the rendered `article.ds-prose`. Nothing else receives them. |
| `profile` | `string \| null` | `null` | One ODRL profile as JSON-LD or Turtle text. Blank text means no profile. The component does no network access: fetch the text yourself. |
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

## `PROSE_WASM_URL`

`PROSE_WASM_URL` is an `InjectionToken<string>` provided in root. Its default is `'prose_wasm_bg.wasm'`. The component resolves it against `document.baseURI`, which is the page's `<base href>`, and fetches the file once per page, on first render. A failed load is not remembered, so the next input change retries.

To serve the file from another place, provide the token:

{{source prose-angular/test/docs-examples.ts#wasm_url}}

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

A tree that is already mounted is left alone.

## Exports

The package exports exactly these names from its entry point. Nothing else is public: the loader and the sync helpers are internal.

| Name | Kind | Meaning |
|---|---|---|
| `OdrlProseComponent` | class | The component. |
| `PROSE_WASM_URL` | token | Where the wasm file is fetched from. |
| `ProseValidatorService` | class | Validation without rendering. |
| `ProfileSummary` | type | What `profileLoaded` emits. |
| `reportChanged` | function | Whether a report differs from the previous one in verdict or findings. This is the rule behind `validationChange`. |
| `ReportLike` | type | What `reportChanged` reads: a `verdict` and optional findings with a `fingerprint` and a `severity`. |
| `Finding` | type | One finding, from `@ds-labs/prose-wasm`. |
| `RuleId` | type | A frozen rule id, from `@ds-labs/prose-wasm`. |
| `RuleInfo` | type | One entry of the rule catalogue, from `@ds-labs/prose-wasm`. |
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
