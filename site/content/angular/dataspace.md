---
title: Angular in a dataspace
label: Running in a dataspace
description: Narrow the Angular policy editor for a dataspace with the dataspace preset, the constraint-incomplete rule, the logicalDepth limit and the structure lints.
nav: angular
order: 6
---
# Running in a dataspace

A dataspace operator wants its members to write only policies the dataspace can evaluate. This page is about what `ds-odrl-prose-editor` does for that today, using the EONA-X profile as the example.

The editor reads the labels of a real profile, but it does not read the `skos:note` prose that says which values suit which left operand. A profile such as EONA-X (or DSP, or EDC) carries no `prose:operator`, `prose:rightOperand`, `prose:unit` or `prose:closed` data. So what it can narrow is the vocabulary (which actions and left operands exist) and the structure (what a constraint, a group and a rule may look like). The values of a right operand stay free text. Reading values from SHACL shapes, an operator overlay or a value list the host supplies is planned (see the issue tracker) and is not part of the editor yet.

## One switch

`preset="dataspace"` sets what the rest of this page describes, in one input:

```html
<ds-odrl-prose-editor
  [json]="policy" (jsonChange)="policy = $event"
  [profile]="eonaXProfileText"
  preset="dataspace" />
```

| What | Set by the preset |
|---|---|
| Validation | Strict, which reads the vocabulary as closed: ODRL plus the loaded profiles. With the structural tier on, so a rule without a target or an action is an error too. |
| Term slots | `termInput` is `'select'`: the action and the left operand are dropdowns of the profile's terms. |
| Refusing terms | `refuseTerms` is on: an edit that writes a term the profile does not define is refused, and the JSON never holds it. |
| Nesting | `limits` has `logicalDepth: { max: 2 }`: a group may hold a group, and no deeper. |
| Lints | `heuristic.single-child-group` and `heuristic.duplicate-action` are warnings. |

Each input you set yourself wins: `[termInput]="'free'"`, `[refuseTerms]="false"`, `[strict]="false"`, `validation` key by key (its `rules` are added to the preset's) and `limits` list by list. A host that wants two more levels writes `[limits]="{ logicalDepth: { max: 4 } }"` and keeps the rest.

With the EONA-X profile loaded, `abcd` as a left operand and `cxcxcxc` as an action become `profile.unknown-term` errors, and under `refuseTerms` they do not reach the JSON. A value such as `cdcd` is still accepted, because the profile closes no operand.

## What the structure rules say

These are in the validation report under `strict`, and in the problems panel with their fixes.

| Rule | What it finds | Level |
|---|---|---|
| `odrl.constraint-incomplete` | A constraint with no left operand, no operator, no right operand, or an empty right operand value (the state a freshly added condition starts in). | An error in strict mode, with or without the structural tier. |
| `heuristic.single-child-group` | An `and`, `or` or `xone` group around one condition. The fix, "Ungroup", replaces the group by its condition. | Off; a warning with the preset. |
| `heuristic.duplicate-action` | The same action listed twice in one rule, in any spelling of its IRI. The fix removes the second. | Off; a warning with the preset. |

A host that checks the report before it saves can refuse a document whose `verdict` is `violations`, which includes an unfinished constraint.

`odrl.constraint-incomplete` and the two lints are in ruleset 2. A check pinned with `ruleset: 1` does not see them.

## The `logicalDepth` limit

`limits.logicalDepth.max` is how deep groups may nest. A group directly in a condition list is at depth 1, a group in it at depth 2. The editor hides the "group" buttons that would go past the limit, and the reducer refuses the event if a script sends it. A document that already nests deeper is kept as written, and ungrouping stays possible.

See [the editor inputs](/angular/interface/#editor-inputs) for the other inputs, and [validation](/angular/validation/) for the report.
