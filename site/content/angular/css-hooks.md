---
title: Angular CSS hooks
label: CSS hooks
description: Class names of the output of the ds-odrl-prose component that a stylesheet can rely on, and a complete themed example stylesheet.
nav: angular
order: 5
---
# CSS hooks (Angular)

This page lists what a stylesheet can select in the output of `<ds-odrl-prose>`. The Angular package renders the read-only output of `OdrlProse` and nothing else, so the hooks are the read-only ones. There is no editor in the Angular package; the editor hooks are on the [Yew CSS hooks](/yew/css-hooks/) page and do not apply here.

The repository states no CSS stability policy. The split between Stable and Internal on the Yew page is a recommendation, and every class below is Stable in that split. The table is checked against `prose-yew/src/lib.rs`, the file that writes this markup: a test fails when a class is missing here or when this page names one the file does not write.

The tree under the host element is:

```text
ds-odrl-prose                          the Angular host element
  article.ds-prose                     one root; articleClass is added here
    section.prose-policy               one per policy
      h3, p, ul.prose-notes, ol.prose-rules > li.prose-rule.prose-permission | .prose-prohibition | .prose-obligation
        p.prose-rule-sentence
        p.prose-label, ul                conditions, limits, follow-up rules
      details.prose-glossary             only with a profile that explains a term the policy uses
    aside.prose-warnings                 only when something was not rendered
    section.prose-violations             only with validation and showViolations
  article.ds-prose > p.prose-error       instead of the policy, when the JSON cannot be read
  p.prose-error                          instead of everything above, when the wasm cannot be loaded; outside article.ds-prose
```

The load error is a `p.prose-error` that is a direct child of `ds-odrl-prose`, outside `article.ds-prose`. Select it as `ds-odrl-prose > .prose-error`. The rule `.ds-prose .prose-error` does not match it, so `prose.css` has a second rule for it.

{{include hooks-renderer}}

## Custom properties

The read-only renderer reads no custom properties, and the Angular package sets none. Theming with variables works because custom properties inherit: define them on `ds-odrl-prose` or an ancestor, and read them with `var()` in the rules that style the classes above. The example does exactly that with `--ds-*` tokens.

## Example stylesheet

`prose.css` is a complete starting point: the policy, the glossary and the validation findings, in light and in dark. It defines its own tokens in light, in dark by system preference and in dark by `data-theme="dark"` on `<html>`, and it sets `ds-odrl-prose { display: block; }`. Rename the tokens to yours and keep the selectors. The same file is served at `examples/prose.css`, and the home page of this site loads it. A test renders the real output with it in Chrome and checks the colours.

Register it as a global stylesheet; see [Styling](/angular/styling/) for why a component's own `styles` do not reach the output.

{{file examples/prose.css}}
