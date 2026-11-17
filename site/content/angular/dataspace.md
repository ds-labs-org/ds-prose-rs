---
title: Angular in a dataspace
label: Running in a dataspace
description: Narrow the Angular policy editor for a dataspace with the dataspace preset, the constraint-incomplete rule, the logicalDepth limit and the structure lints.
nav: angular
order: 6
---
# Running in a dataspace

A dataspace operator wants its members to write only policies the dataspace can evaluate. This page is about what `ds-odrl-prose-editor` does for that today, using the EONA-X profile as the example.

The editor reads the labels of a real profile, but it does not read the `skos:note` prose that says which values suit which left operand. A profile such as EONA-X (or DSP, or EDC) carries no `prose:operator`, `prose:rightOperand`, `prose:unit` or `prose:closed` data. So what it can narrow is the vocabulary (which actions and left operands exist) and the structure (what a constraint, a group and a rule may look like). The values of a right operand stay free text. Values can come from the dataspace's companion SHACL shapes (see [Companion shapes](#companion-shapes)) or, when it publishes none, from an operator overlay (see [Operator overlay](#operator-overlay)). A value list the host supplies is planned and is not part of the editor yet.

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
| `sh:in` | The closed values: IRIs, and literals (text or numbers; a literal matches on its text, a number by value, whatever its datatype). The operand becomes a dropdown with `termInput="select"` when the list holds only IRIs, and any other value is `prose.value-not-allowed`. A list with a literal stays a free-text slot, which the validation checks. |
| `sh:class`, `sh:node` | The IRIs of that class in the profile, with the terms narrower by `skos:broader`. |
| `sh:datatype` | The datatype of the value. |
| `sh:pattern`, `sh:minInclusive`, `sh:maxInclusive` | Lexical checks on the value. The pattern is a small regular expression subset. |
| `sh:in` on `odrl:operator` or `odrl:unit` there | The operators or units that suit X (IRIs only). |
| `sh:in` on `odrl:action` or `odrl:leftOperand` | Closes the action or left operand list for the whole policy (IRIs only), as `profile.unknown-term` under strict validation. Like every unknown term, this binds in strict mode only: the editor narrows its action and left operand selects to these lists only while strict validation is on, and advisory validation does not refuse a term outside them. |

Everything else is ignored, and each thing ignored is a line in `warnings` of `shapesLoaded`. That covers other `sh:` properties, property paths that are not read, and `sh:not` and `sh:xone`. It also covers literal values in the `sh:in` of an operator, a unit, an action or a left operand list, and a shape that names no left operand. Shapes only add hints to terms the loaded profiles declare. They never rename a term, so the sentences keep the profile's labels.

A finding that a shape caused has `expected.source` set to `"shapes"` and says "the shapes allow". Findings from the profile say "the profile allows" and carry no `source`. Where both give hints, a shape replaces the profile's for that facet only (operators, values, units, datatype), and the other facets stay the profile's.

An unreadable text emits `shapesError` and the previous shapes stay. Shapes are kept when the profile changes, and `null` or blank text removes them.

## Operator overlay

When the dataspace publishes no SHACL, its governance can publish an overlay next to the profile and version it with it. An overlay is a small annotation-only graph: `prose:` triples (`prose:operator`, `prose:rightOperand`, `prose:unit`, `prose:datatype`, `prose:closed`, `prose:plural`) about left operands that the profile declares. Pass the texts in `overlays`, JSON-LD or Turtle (the Turtle needs the [Turtle build](/angular/interface/#prose-wasm-module)):

```turtle
@prefix prose: <https://ds-labs-org.github.io/ds-prose-rs/profile#> .
@prefix odrl:  <http://www.w3.org/ns/odrl/2/> .
@prefix dx:    <https://example.org/dataspace/> .

dx:purpose prose:closed true ;
    prose:operator odrl:eq, odrl:isAnyOf ;
    prose:rightOperand dx:research, dx:marketing .
```

```html
<ds-odrl-prose-editor
  [json]="policy" (jsonChange)="policy = $event"
  [profile]="eonaXProfileText"
  [overlays]="[eonaXOverlayText]"
  preset="dataspace"
  (overlaysLoaded)="onOverlays($event)" />
```

The same `[overlays]` input exists on `ds-odrl-prose` and on `ProseValidatorService.validate`, and `setOverlays(texts)` on the wasm handles (`[]` removes them).

A second profile that names the same IRI is a "declared by both" conflict. An overlay is not a second declaration: it merges its hints onto the term and never adds or relabels one. A triple about an IRI that no loaded profile declares as a left operand is ignored, with a line in the `warnings` of `overlaysLoaded`, and so is a label or a type the overlay gives. The sentences keep the profile's labels.

An overlay lists exactly the values it means, as `sh:in` does: values narrower than a listed one are not added. Without `prose:closed true` its hints are suggestions, as a profile's are. With it they are binding under strict validation: a right operand becomes a dropdown with `termInput="select"`, and any other value is `prose.value-not-allowed`. A finding an overlay caused has `expected.source` set to `"overlay"` and says "the overlay allows". A later overlay replaces an earlier one for each facet it gives.

An overlay and a shape that say the same thing give the same hints, so a dataspace can start with an overlay and move to shapes later. An overlay cannot say what shapes add: `sh:class`, patterns, bounds, and the closed lists of actions and left operands.

## Which source to use when

Sources are layered, and each facet (operators, values, units, datatype) is taken from the last source that gives it: profile, then overlay, then shapes. A source never relabels a term, and each finding names the source that closed the slot (`profile`, `overlay` or `shapes`; the profile's findings carry no `source`).

| You have | Use |
|---|---|
| Only a profile (EONA-X, DSP, EDC) | The profile for the vocabulary, with `preset="dataspace"` to close the action and left-operand lists to what it declares. Values stay free text. |
| A profile, and SHACL the connector already checks | `[shapes]`. One source of truth: what the editor offers is what the connector checks. Shapes also close the action and left-operand lists, and give classes, patterns and bounds. |
| A profile, and no SHACL | `[overlays]`, published by the governance next to the profile. Lists of values, operators and units per left operand. |
| Both | Both. The shapes win for each facet they give; the overlay fills the rest. |

A value list the shapes and the overlay cannot say (one fetched at run time, such as participants) is not read yet.
