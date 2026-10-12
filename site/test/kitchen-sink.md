---
title: Kitchen sink
description: Every block type at an awkward size, used by the layout check only.
nav: home
---
# Kitchen sink

A paragraph with a long token `OdrlProseViewProps::on_change_with_a_very_long_identifier_that_never_breaks` and a long link [https://example.org/a/very/long/path/that/keeps/going/and/going/and/going/without/a/break](https://example.org/a/very/long/path).

## Table

| Property | Type | Default | Description |
|:--|:--|--:|:--|
| `policy` | `AttrValue` | none | The ODRL JSON-LD document to render as sentences, given as text. |
| `class` | `Classes` | empty | Extra classes for the root element of the rendered article, appended after the built-in ones. |
| `show_everything_in_one_column_name` | `Option<Callback<String>>` | `None` | A long row to force the table to scroll inside its own box. |

## Code

```html
<article class="prose-policy prose-with-a-long-class-name another-long-class-name yet-another-class-name" data-policy-uid="urn:uuid:0123456789abcdef0123456789abcdef">
```

### Lists

- one
  - nested
- two

> A quotation.
