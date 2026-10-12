---
title: Angular styling
label: Styling
description: How to inject CSS for the output of ds-odrl-prose in Angular - global styles, articleClass, ViewEncapsulation.None and ng-deep - and why a component's own styles do not reach it.
nav: angular
order: 3
---
# Styling (Angular)

`ds-odrl-prose` ships no CSS and writes no `<style>` element. It declares no `styles` and no `encapsulation`, and its template is empty. The sentences are plain elements with `prose-*` classes, created by Yew inside the host element, so the whole job of styling is to choose where your stylesheet lives. The [CSS hooks](/angular/css-hooks/) page lists the classes and ends with a complete stylesheet.

There is no editor in the Angular package, so there is no editor stylesheet to inject. The editor, and its `BASE_CSS`, exist only for Yew hosts.

## Global stylesheet

Put the rules in a stylesheet that Angular does not scope: `src/styles.css`, or any entry in the `styles` array of `angular.json`. This is what `demo-angular` does, and it is the recommended route.

```json
"styles": ["src/styles.css", "src/prose.css"]
```

Copy `prose.css` from the CSS hooks page, rename its `--ds-*` tokens to yours, and register it there. It sets `ds-odrl-prose { display: block; }`. Without that rule the host is `display: inline`, because an unknown element is inline by default; margins, width and background on the host then do not behave as on a block.

## `articleClass`

`articleClass` adds classes to the root `article.ds-prose`, and to nothing else. It is passed when the component mounts and again on every update.

{{source prose-angular/test/docs-examples.ts#article_class}}

Use it to scope a stylesheet to one place without scoping by component: `.my-prose .prose-rule-sentence { ... }` in the global stylesheet. It injects no CSS by itself. It also lets a utility-class framework put its classes on the root.

## Why a component's own styles do not reach the output

By default Angular uses emulated view encapsulation. It rewrites every selector in a component's `styles` so that it only matches elements that carry an attribute named for that component, and it puts that attribute on the elements its own renderer creates. Yew creates the nodes inside `ds-odrl-prose`, so they carry no such attribute and the rewritten selector never matches them. The host element `ds-odrl-prose` is created by the parent's template, so it does carry the parent's attribute.

The compiled forms below are what Angular's own style compiler produces for a component with the id `c1`:

```css
/* source */                           .prose-rule-sentence { font-weight: 600; }
/* emulated encapsulation, compiled */ .prose-rule-sentence[_ngcontent-c1] { font-weight: 600; }

/* source */                           ds-odrl-prose { display: block; }
/* emulated encapsulation, compiled */ ds-odrl-prose[_ngcontent-c1] { display: block; }

/* source */                           :host ::ng-deep .prose-rule-sentence { font-weight: 600; }
/* emulated encapsulation, compiled */ [_nghost-c1] .prose-rule-sentence { font-weight: 600; }
```

So, with default encapsulation, a component's `styles` can style the `ds-odrl-prose` element, and cannot style what is inside it, unless it uses `::ng-deep`.

How this is checked: a test in `site/test/angular-encapsulation.test.mjs` runs `encapsulateStyle` from the `@angular/compiler` of `prose-angular`. It then loads the compiled CSS in Chrome around the real renderer output, with the attribute on the host element and none on the Yew nodes. The first rule does not reach the output, the third does, and `ds-odrl-prose { display: block; }` does apply. The test does not boot an Angular application. That Angular puts the attribute only on elements it creates is Angular's documented behaviour, not code in this repository.

## `ViewEncapsulation.None`

On the component that contains `<ds-odrl-prose>`, `ViewEncapsulation.None` turns its `styles` into global styles. Then `.prose-rule-sentence` works. It also leaks to the whole application, so scope it with `articleClass`.

{{source prose-angular/test/docs-examples.ts#encapsulation_none}}

## `:host ::ng-deep`

`::ng-deep` removes the scoping from what follows it. `:host ::ng-deep .prose-rule-sentence` keeps the rule inside the component, because the left side is still scoped to the component's host, and reaches the output. A bare `::ng-deep` is global; prefer the `:host` form. Angular marks `::ng-deep` as deprecated, and it works today.

{{source prose-angular/test/docs-examples.ts#ng_deep}}

## Custom properties and dark mode

Custom properties inherit through the output, so a variable defined on `ds-odrl-prose` or on any ancestor, including `:root`, is available to the rules that style the classes. That works with any encapsulation, as long as the rule that reads `var(--x)` is one that reaches the output.

The renderer itself reads no custom property. For dark mode, the example stylesheet uses `color-scheme: light dark`, a `prefers-color-scheme: dark` block, and `data-theme` on `<html>` to force either side. The library takes no part in it.

## See it in the demo

The [Angular demo](/demo-angular/) loads the example stylesheet of this page as an entry of its own in the `styles` array of `angular.json`: `site/content/examples/prose.css`, with `bundleName` set to `example`. The file is the one printed here, so the page and the demo cannot differ.

Under the navigation, the switch "Example stylesheet" has two settings. Unstyled disables that file and shows the markup as the component emits it. Themed turns it back on.

The demo's own layout sheet, `src/styles.css`, stays on in both settings. It holds the columns and the validation controls, and none of the rules of the example stylesheet.

## Shadow DOM

Shadow DOM is not supported and not tested. `ProseHandle` mounts into an `Element`. Mounting inside a shadow root is possible for the read-only output, and global styles would then not apply there. Nothing in the repository mounts the component in a shadow root, so this page makes no promise about it.
