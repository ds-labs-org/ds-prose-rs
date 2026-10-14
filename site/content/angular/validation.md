---
title: Angular validation
label: Validation
description: Validate ODRL policies against profiles in Angular - strict, validation options, profiles, the report signal, validationChange and ProseValidatorService.
nav: angular
order: 4
---
# Validation (Angular)

The component can check a policy against ODRL profiles and show the findings. The service does the same without rendering. Both run the engine that the `ds-prose-validate` command runs, so the reports are the same.

A verdict of `incomplete` is never a pass: it means no error was found but something could not be checked, such as a declared profile that is not loaded.

## Turning validation on

`strict` turns it on with the default options. `validation` turns it on with options of your own and wins over `strict`. `showViolations` shows the findings under the prose, and has an effect only when validation is on.

Each row is evaluated against `validationOptions` in `prose-angular/src/validation-sync.ts` by a test, so this table is what the component does.

| `strict` | `validation` | Options handed to the validator |
|---|---|---|
| `false` | `null` | `null` |
| `true` | `null` | `{"strict":true}` |
| `false` | `{"failClosed":true}` | `{"failClosed":true}` |
| `true` | `{"strict":false}` | `{"strict":false}` |

`null` in the last column means validation is off. The report signal is then `null`.

{{source prose-angular/test/docs-examples.ts#validation}}

## Profiles

A profile is the text of an ODRL profile, JSON-LD, or Turtle with the [Turtle build](/angular/interface/#prose-wasm-module). The component does no network access. The `profile` input takes one text and `profiles` takes a list; a list wins, an empty list clears, and blank texts are dropped.

Each row is evaluated against `profileTexts` in the same file.

| `profile` | `profiles` | Texts handed to the validator |
|---|---|---|
| `null` | `null` | `null` |
| `"a"` | `null` | `["a"]` |
| `"  "` | `null` | `null` |
| `"a"` | `["b"]` | `["b"]` |
| `"a"` | `[]` | `[]` |
| `null` | `["a", " ", "b"]` | `["a","b"]` |

`profileLoaded` emits a `ProfileSummary` for each profile read. `profileError` emits when a text cannot be read, and the previous profile stays. A profile or an options object that did not change is not handed to the wasm module again.

{{source prose-angular/test/docs-examples.ts#profile}}

## Options

`validation` takes `ValidateOptions` from `@ds-labs/prose-wasm`. Every field is optional, and `{}` is strict mode with the profile and prose rules.

| Field | Type | Meaning |
|---|---|---|
| `strict` | `boolean` | Strict, the default, reads the vocabulary as closed. `false` is the advisory reading. |
| `structural` | `boolean` | Also check the structural MUSTs of the Information Model (`odrl.*` rules). Off by default. |
| `failClosed` | `boolean` | Make what could not be checked an error rather than a gap. |
| `denyDeprecated` | `boolean` | Make a deprecated term an error rather than a warning. |
| `odrl` | `"common" \| "core"` | The ODRL terms defined without a profile: Core and Common (the default), or Core only. |
| `ruleset` | `number` | Check only the rules of this ruleset and earlier, so a pinned check does not change with a release. |
| `rules` | `Partial<Record<RuleId, RuleLevel>>` | A level per rule, over everything else. `input.unreadable` stays an error. |

{{source prose-angular/test/docs-examples.ts#options}}

## The report

`validationChange` emits the report when its verdict or its set of findings changes. The first report counts. Wording and order do not count, so an edit that changes nothing the validator found is not an event. The `report` signal always holds the latest. `validationError` emits when the options cannot be read, and the previous options stay.

The report has schema `ds-prose/validation-report@1`. Its fields:

| Field | Type | Meaning |
|---|---|---|
| `schema` | `"ds-prose/validation-report@1"` | The schema of the report. |
| `tool` | `{ name: string; version: string }` | The tool that wrote it. |
| `mode` | `"strict" \| "advisory"` | The reading used. |
| `tiers` | `string[]` | The tiers of rules that ran. |
| `ruleset` | `number` | The ruleset number. |
| `verdict` | `Verdict` | `"conforms"`, `"violations"` or `"incomplete"`. |
| `vocabulary` | `{ odrl: "common" \| "core"; profiles: { iri: string \| null; title: string \| null; version: string \| null }[] }` | The vocabulary checked against. |
| `gaps` | `{ rule: RuleId; detail: string }[]` | What could not be checked. |
| `counts` | `{ error: number; warning: number; info: number }` | Findings by severity. |
| `findings` | `Finding[]` | The findings. |

Each finding has a `rule`, a `severity` of `"error"`, `"warning"` or `"info"`, a `message`, a `target`, a `fingerprint` that stays the same when the input is reformatted, and `fixes`. The full type is `Finding` in `@ds-labs/prose-wasm`.

{{source prose-angular/test/docs-examples.ts#report_signal}}

## Without rendering

`ProseValidatorService.validate(json, profiles, options)` returns the report as a promise. It is strict unless `options.strict` is `false`. A problem in the policy is a finding, and a profile or options it cannot read is a rejection.

{{source prose-angular/test/docs-examples.ts#service}}
