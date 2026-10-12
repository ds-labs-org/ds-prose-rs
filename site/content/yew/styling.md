---
title: Yew styling
label: Styling
description: How to inject CSS for the rendered policy and for the editor in a Yew application - global stylesheets, class props, BASE_CSS, ProseClasses and inline styles.
nav: yew
order: 3
---
# Styling (Yew)

Two things can be styled: the rendered policy and the editor. The rendered policy ships no CSS. The editor ships one stylesheet, `BASE_CSS`, which you can keep, extend, or turn off. This page says how to inject yours in each case. The classes you can select are on the [CSS hooks](/yew/css-hooks/) page, which ends with a complete example stylesheet in light and dark.

## The rendered policy

`OdrlProse` emits no `<style>`. The read mode of `OdrlProseView` emits none either. Without a stylesheet you get unstyled `h3`, `p`, `ol` and `ul`, with no visual difference between a permission, a prohibition and an obligation.

All the output is plain markup in the page, so any stylesheet reaches it: a `<link>` in `<head>`, a global stylesheet of your build, or, with Trunk, `<link data-trunk rel="css" href="...">`, which is what `demo/index.html` does.

To scope rules to one place, give the root a class. The `class` prop of `OdrlProse` is added to the root after `ds-prose`:

{{source prose-yew/tests/site_examples.rs#reader_wording}}

That renders `<article class="ds-prose my-prose">`, and `.my-prose .prose-rule-sentence { ... }` reaches the sentences. `class` injects no CSS by itself. It is also how a utility-class framework puts its classes on the root. `OdrlProseView` and `ProseProblems` take `class` the same way.

For a stylesheet to start from, copy `prose.css` from the [CSS hooks](/yew/css-hooks/) page. It colours the rule kinds, the glossary and the findings, from tokens you rename.

## The editor: four routes

In edit mode, `OdrlProseView` writes `<style>` with `BASE_CSS` as the first child of its root. `BASE_CSS` is exported as a `pub const &str`. It uses only system colours (`Highlight`, `Canvas`, `CanvasText`, `HighlightText`) and `currentColor`, so light and dark both work without a host stylesheet. It styles slots, the placeholder, the focus ring, invalid and warning underlines, the buttons, locked parts, issues, the listbox and its options, and it hides the live region. Every rule starts with `.ds-prose.prose-edit`.

### 1. Keep it and recolour

The `<style>` element sits inside the article, so it comes after every stylesheet in `<head>` in document order. A rule of the same specificity in your stylesheet therefore loses. Raise the specificity: prefix the selector with `:root`, which adds one point, or scope it with a class from `class`. The example stylesheet `prose-editor.css` prefixes every selector with `:root`.

A test shows both halves in Chrome. An equal-specificity rule loses to `BASE_CSS`, and the same rule with `:root` in front wins, in light and in dark.

Two custom properties are read by `BASE_CSS`: `--prose-error` for the invalid underline and the error text of an issue, and `--prose-warning` for the warning ones. Set them on any ancestor or on `.ds-prose`. With nothing set, the fallbacks are `#c9190b` and `#b45309`. The read-only renderer reads no custom properties.

### 2. Bring your own stylesheet

Set `base_css` to `false` and ship the CSS yourself, optionally starting from `BASE_CSS`, which you can write into your page head at build time. Then your stylesheet owns every rule, including three that the editor depends on:

- `.prose-live` must be visually hidden. It is the announcer, and unstyled it is visible text.
- `.prose-suggestions` and `.prose-slot-message` must be positioned against `.prose-slot-wrap`.
- The placeholder of an empty slot is `.prose-slot:empty::before { content: attr(data-placeholder) }`.

A test confirms the first point: with `BASE_CSS` the announcer is clipped to a pixel, and without it, it is not.

Server rendering does not need this route. With Yew 0.23, the version `prose-yew` depends on, the server renderer writes the text of `<style>` unescaped, so `BASE_CSS` arrives as CSS. A test in `prose-yew/tests/site_examples.rs` pins that the server output holds `BASE_CSS` byte for byte as the first child of the root. `ssr.rs` pins that `base_css = false` emits no `<style`.

### 3. Extra classes

`EditConfig::classes` is a `ProseClasses`: one `Classes` field per kind of element. Each is added after the built-in `prose-*` name and never replaces it. Use it to put a design system's classes on elements, for example PatternFly's `pf-v5-c-button` on the add buttons.

### 4. Inline styles

`EditConfig::styles` is a `ProseStyles` with the same field names, each an `Option<AttrValue>`, written as the `style` attribute. State styles come after the plain one, separated by `; `: `slot_invalid`, `slot_warning` and `slot_whitespace` after `slot`; `issue_error`, `issue_warning` and `issue_info` after `issue`; `permission`, `prohibition` and `obligation` after `rule`; `suggestion_active` after `suggestion`. An inline style beats every stylesheet that does not use `!important`.

{{source prose-yew/tests/site_examples.rs#own_css}}

That renders `class="ds-prose prose-view my-root prose-edit extra"` on the root and `class="prose-add pf-v5-c-button pf-m-link"` on an add button, with no `<style>`.

## Extra classes and inline styles

`ProseClasses` and `ProseStyles` have the same 46 fields. A field that is empty or `None` adds nothing. `text` and `carried` apply only when set. The names are:

`root`, `policy`, `heading`, `intro`, `notes`, `rules`, `rule`, `permission`, `prohibition`, `obligation`, `sentence`, `text`, `term`, `slot`, `slot_empty`, `slot_invalid`, `slot_warning`, `slot_whitespace`, `inherited`, `select`, `list`, `item`, `add`, `remove`, `reorder`, `wrap`, `toolbar`, `add_bar`, `label`, `conditions`, `condition`, `logical`, `follow_up`, `carried`, `locked`, `issue`, `issue_error`, `issue_warning`, `issue_info`, `warnings`, `decoration`, `suggestions`, `suggestion`, `suggestion_active`, `live`, `empty`.

Not every element has a field. The problems panel, `prose-slot-wrap`, `prose-slot-edit`, `prose-slot-note`, `prose-slot-message`, the suggestion group elements, `prose-heading-row`, `prose-rule-kind` and `prose-locked-note` have fixed class names and no config field. `config.classes.root` comes before `prose-edit` in the root's class list, and the `class` prop comes last.

## Where the panel and the glossary go

`ProseProblems` is a separate component and is placed by the host, usually next to the view and outside `.ds-prose`. Its rules are therefore not scoped under `.ds-prose`: its root is `.prose-problems`. `ProseGlossary` is written by `OdrlProse` inside the article, after each policy.

## Shadow DOM

Shadow DOM is not supported and not tested. The read-only output has no events and no document lookups, so mounting it in a shadow root is plausible, and global CSS would not reach it. For the editor it is not safe. `slot.rs` looks an element up with `document.get_element_by_id`, and `problems.rs` uses `document.active_element()` and `closest(".prose-problems")`. Inside a shadow root those lookups fail silently, so scrolling the active option into view and restoring focus after a fix degrade.
