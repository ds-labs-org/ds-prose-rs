---
title: Angular in a dataspace
label: Running in a dataspace
description: Narrow the Angular policy editor for a dataspace with the dataspace preset, the constraint-incomplete rule, the logicalDepth limit and the structure lints.
nav: angular
order: 6
---
# Running in a dataspace

A dataspace operator wants its members to write only policies the dataspace can evaluate. This page is about what `ds-odrl-prose-editor` does for that today, using the EONA-X profile as the example.

The editor reads the labels of a real profile, but it does not read the `skos:note` prose that says which values suit which left operand. A profile such as EONA-X (or DSP, or EDC) carries no `prose:operator`, `prose:rightOperand`, `prose:unit` or `prose:closed` data. So what it can narrow is the vocabulary (which actions and left operands exist) and the structure (what a constraint, a group and a rule may look like). The values of a right operand stay free text. Values can come from the dataspace's companion SHACL shapes (see [Companion shapes](#companion-shapes)). An operator overlay or a value list the host supplies is planned and is not part of the editor yet.

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

## Companion shapes

A dataspace usually publishes SHACL shapes beside its ODRL profile. They say which right operands suit a left operand, which the profile does not. Pass their text in `shapes`, JSON-LD or Turtle (the Turtle needs the [Turtle build](/angular/interface/#prose-wasm-module)):

```html
<ds-odrl-prose-editor
  [json]="policy" (jsonChange)="policy = $event"
  [profile]="eonaXProfileText"
  [shapes]="eonaXShapesText"
  preset="dataspace"
  (shapesLoaded)="onShapes($event)" />
```

The component does no network access: fetch both texts yourself. The same `[shapes]` input exists on `ds-odrl-prose` and on `ProseValidatorService.validate`, and `setShapes(text)` on the wasm handles.

This is not a SHACL engine. Only this subset is read:

| In a shape | What it gives |
|---|---|
| A property shape on `odrl:rightOperand` in a constraint shape selected by `odrl:leftOperand sh:hasValue X`, directly or through `sh:or` or `sh:qualifiedValueShape` | Hints for the right operand of left operand X. |
| `sh:in` | The closed values. The operand becomes a dropdown with `termInput="select"`, and any other value is `prose.value-not-allowed`. |
| `sh:class`, `sh:node` | The IRIs of that class in the profile, with the terms narrower by `skos:broader`. |
| `sh:datatype` | The datatype of the value. |
| `sh:pattern`, `sh:minInclusive`, `sh:maxInclusive` | Lexical checks on the value. The pattern is a small regular expression subset. |
| `sh:in` on `odrl:operator` or `odrl:unit` there | The operators or units that suit X. |
| `sh:in` on `odrl:action` or `odrl:leftOperand` | Closes the action or left operand list for the whole policy, as `profile.unknown-term` under strict validation. |

Everything else is ignored, and each thing ignored is a line in `warnings` of `shapesLoaded`: other `sh:` properties, property paths that are not read, `sh:not` and `sh:xone`, literal values in `sh:in`, and a shape that names no left operand. Shapes only add hints to terms the loaded profiles declare. They never rename a term, so the sentences keep the profile's labels.

A finding that a shape caused has `expected.source` set to `"shapes"` and says "the shapes allow". Findings from the profile say "the profile allows" and carry no `source`. Where both give hints, a shape replaces the profile's for that facet only (operators, values, units, datatype), and the other facets stay the profile's.

An unreadable text emits `shapesError` and the previous shapes stay. Shapes are kept when the profile changes, and `null` or blank text removes them.

Which source to use: the profile for the vocabulary, the shapes for the values the dataspace accepts for each left operand. A value list the shapes cannot say (a literal list, or one fetched at run time) is not read yet.
