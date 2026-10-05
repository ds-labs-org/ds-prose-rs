//! The stylesheet Edit mode ships by default, scoped to `.ds-prose.prose-edit`.
//! It uses only system colours and `currentColor`, so light and dark both
//! work. Turn it off with `EditConfig::base_css = false`.

pub const BASE_CSS: &str = r#"
.ds-prose.prose-edit .prose-slot {
  display: inline-block;
  min-width: 2ch;
  padding: 0 .25ch;
  border-bottom: 1px dashed currentColor;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  cursor: text;
}
.ds-prose.prose-edit .prose-slot:empty::before {
  content: attr(data-placeholder);
  /* Decorative: empty alternative text keeps it out of the accessible value
     (the slot's aria-placeholder says it instead). Ignored where unsupported. */
  content: attr(data-placeholder) / "";
  opacity: .6;
  font-style: italic;
}
.ds-prose.prose-edit .prose-slot:focus,
.ds-prose.prose-edit .prose-select:focus,
.ds-prose.prose-edit button:focus-visible {
  outline: 2px solid Highlight;
  outline-offset: 1px;
}
.ds-prose.prose-edit .prose-slot[aria-invalid="true"],
.ds-prose.prose-edit .prose-slot-invalid {
  border-bottom: 2px solid var(--prose-error, #c9190b);
}
.ds-prose.prose-edit .prose-slot-warning {
  border-bottom: 2px solid var(--prose-warning, #b45309);
}
.ds-prose.prose-edit .prose-slot-whitespace {
  outline: 1px dotted currentColor;
}
.ds-prose.prose-edit .prose-inherited {
  opacity: .7;
  font-style: italic;
}
.ds-prose.prose-edit .prose-empty {
  opacity: .7;
  font-style: italic;
}
.ds-prose.prose-edit .prose-select {
  font: inherit;
  max-width: 100%;
}
.ds-prose.prose-edit .prose-add,
.ds-prose.prose-edit .prose-remove,
.ds-prose.prose-edit .prose-reorder,
.ds-prose.prose-edit .prose-wrap {
  font: inherit;
  font-size: .85em;
  margin-inline: .2em;
  padding: 0 .4em;
  cursor: pointer;
  line-height: 1.4;
}
.ds-prose.prose-edit .prose-locked {
  display: inline-block;
  max-width: 100%;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
  padding: 0 .3em;
  border: 1px solid currentColor;
  border-radius: 3px;
  opacity: .85;
}
.ds-prose.prose-edit .prose-issue {
  display: block;
  min-height: 1.2em;
  font-size: .85em;
}
.ds-prose.prose-edit .prose-issue-error { color: var(--prose-error, #c9190b); }
.ds-prose.prose-edit .prose-issue-warning { color: var(--prose-warning, #b45309); }
.ds-prose.prose-edit .prose-issue-info { opacity: .8; }
.ds-prose.prose-edit .prose-slot-wrap { position: relative; display: inline-block; }
.ds-prose.prose-edit .prose-suggestions {
  position: absolute;
  z-index: 10;
  left: 0;
  top: 100%;
  min-width: 12em;
  max-height: 14em;
  overflow: auto;
  margin: 2px 0 0;
  padding: 0;
  list-style: none;
  background: Canvas;
  color: CanvasText;
  border: 1px solid currentColor;
  font-size: .9em;
}
.ds-prose.prose-edit .prose-suggestion { padding: .15em .5em; cursor: pointer; }
.ds-prose.prose-edit .prose-suggestion-active,
.ds-prose.prose-edit .prose-suggestion:hover {
  background: Highlight;
  color: HighlightText;
}
.ds-prose.prose-edit .prose-suggestion small { opacity: .75; margin-left: .5em; }
.ds-prose.prose-edit.prose-reveal-hover .prose-add,
.ds-prose.prose-edit.prose-reveal-hover .prose-remove,
.ds-prose.prose-edit.prose-reveal-hover .prose-reorder,
.ds-prose.prose-edit.prose-reveal-hover .prose-wrap {
  opacity: .15;
}
.ds-prose.prose-edit.prose-reveal-hover :hover > .prose-add,
.ds-prose.prose-edit.prose-reveal-hover :hover > .prose-remove,
.ds-prose.prose-edit.prose-reveal-hover :hover > .prose-reorder,
.ds-prose.prose-edit.prose-reveal-hover :hover > .prose-wrap,
.ds-prose.prose-edit.prose-reveal-hover :focus-within > .prose-add,
.ds-prose.prose-edit.prose-reveal-hover :focus-within > .prose-remove,
.ds-prose.prose-edit.prose-reveal-hover :focus-within > .prose-reorder,
.ds-prose.prose-edit.prose-reveal-hover :focus-within > .prose-wrap,
.ds-prose.prose-edit.prose-reveal-hover :hover > .prose-toolbar > button,
.ds-prose.prose-edit.prose-reveal-hover :focus-within > .prose-toolbar > button,
.ds-prose.prose-edit.prose-reveal-hover button:focus {
  opacity: 1;
}
.ds-prose.prose-edit .prose-live {
  position: absolute;
  width: 1px;
  height: 1px;
  margin: -1px;
  padding: 0;
  overflow: hidden;
  clip: rect(0 0 0 0);
  white-space: nowrap;
  border: 0;
}
.ds-prose.prose-edit .prose-decoration { margin-left: .5em; }
"#;
