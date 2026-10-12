// The example stylesheets in a real browser: the colours they promise, in
// light and in dark, over the renderer's real output (examples/offer.html),
// and the cascade rules the styling pages state.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { content, source } from './contract.mjs';
import { chrome, inChrome, reporter } from './chrome.mjs';
import { blockAfter, decls } from './css.mjs';

const skip = chrome ? false : 'no Chrome found (set CHROME)';
const PROSE = content('examples/prose.css');
const EDITOR = content('examples/prose-editor.css');
const OFFER = content('examples/offer.html');
const BASE_CSS = /pub const BASE_CSS: &str = r#"([\s\S]*?)"#;/.exec(source('prose-yew/src/edit/css.rs'))[1];

const tokens = (css) => {
  const light = decls(blockAfter(css, ':root {').body);
  const dark = decls(blockAfter(css, ':root:not([data-theme="light"])', css.indexOf('@media (prefers-color-scheme: dark)')).body);
  return { light, dark };
};
const T = tokens(PROSE);
const rgb = (h) => `rgb(${[1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16)).join(', ')})`;

const page = (theme, body, extraCss = '', probe = '') =>
  `<!doctype html><html${theme ? ` data-theme="${theme}"` : ''}><head><meta charset="utf-8"><style>${PROSE}</style><style>${extraCss}</style></head><body>${body}${reporter(probe)}</body></html>`;

const KINDS = `out({
  permission: cs('.prose-permission > .prose-rule-sentence', 'borderLeftColor'),
  obligation: cs('.prose-obligation > .prose-rule-sentence', 'borderLeftColor'),
  label: cs('.prose-label', 'color'),
  fg: cs('.ds-prose', 'color'),
});`;

test('prose.css over the real renderer output: rule kinds take their token colours (light, forced dark, media dark)', { skip, timeout: 120_000 }, () => {
  const light = inChrome(page('light', OFFER, '', KINDS));
  assert.equal(light.permission, rgb(T.light['--ds-allow']));
  assert.equal(light.obligation, rgb(T.light['--ds-duty']));
  assert.equal(light.label, rgb(T.light['--ds-muted']));
  assert.equal(light.fg, rgb(T.light['--ds-fg']));
  const forced = inChrome(page('dark', OFFER, '', KINDS));
  assert.equal(forced.permission, rgb(T.dark['--ds-allow']));
  assert.equal(forced.fg, rgb(T.dark['--ds-fg']));
  const media = inChrome(page('', OFFER, '', KINDS), { dark: true });
  assert.equal(media.permission, rgb(T.dark['--ds-allow']), 'prefers-color-scheme: dark without data-theme');
  const forcedLight = inChrome(page('light', OFFER, '', KINDS), { dark: true });
  assert.equal(forcedLight.permission, rgb(T.light['--ds-allow']), 'data-theme="light" beats a dark system');
});

test('prose.css leaves a prohibition red and a violation list coloured by severity', { skip, timeout: 120_000 }, () => {
  const html = '<article class="ds-prose"><ol class="prose-rules"><li class="prose-rule prose-prohibition"><p class="prose-rule-sentence">x</p></li></ol>' +
    '<section class="prose-violations"><ul class="prose-violations-list"><li class="prose-violation prose-violation-error">e</li><li class="prose-violation prose-violation-warning">w</li></ul></section></article>';
  const r = inChrome(page('light', html, '', `out({ p: cs('.prose-prohibition > .prose-rule-sentence','borderLeftColor'), e: cs('.prose-violation-error','color'), w: cs('.prose-violation-warning','color') });`));
  assert.equal(r.p, rgb(T.light['--ds-deny']));
  assert.equal(r.e, rgb(T.light['--ds-deny']));
  assert.equal(r.w, rgb(T.light['--ds-warn']));
});

// The cascade the Yew styling page describes: BASE_CSS is a <style> inside the
// article, after every stylesheet in <head>.
const EDIT = `<article class="ds-prose prose-view prose-edit"><style>${BASE_CSS}</style>
<p class="prose-rule-sentence"><span class="prose-slot-wrap"><span class="prose-slot prose-slot-invalid" contenteditable="plaintext-only" data-slot="x">bad</span></span></p></article>`;
const SLOT = `out({ c: cs('.prose-slot', 'borderBottomColor'), w: cs('.prose-slot', 'borderBottomWidth') });`;

test('a host rule of equal specificity loses to BASE_CSS; the ":root" prefix of prose-editor.css wins', { skip, timeout: 120_000 }, () => {
  const equal = inChrome(page('light', EDIT, `.ds-prose.prose-edit .prose-slot-invalid { border-bottom-color: rgb(1, 2, 3); }`, SLOT));
  assert.notEqual(equal.c, 'rgb(1, 2, 3)', 'equal specificity: the later <style> inside the article must win');
  const lifted = inChrome(page('light', EDIT, EDITOR, SLOT));
  assert.equal(lifted.c, rgb(T.light['--ds-deny']), 'prose-editor.css sets the invalid underline from --prose-error');
  const dark = inChrome(page('dark', EDIT, EDITOR, SLOT));
  assert.equal(dark.c, rgb(T.dark['--ds-deny']));
});

test('--prose-error set on an ancestor reaches the editor underline; with nothing set the fallback applies', { skip, timeout: 120_000 }, () => {
  const set = inChrome(`<!doctype html><html><head><meta charset="utf-8"></head><body style="--prose-error: rgb(9, 8, 7)">${EDIT}${reporter(SLOT)}</body></html>`);
  assert.equal(set.c, 'rgb(9, 8, 7)');
  const none = inChrome(`<!doctype html><html><head><meta charset="utf-8"></head><body>${EDIT}${reporter(SLOT)}</body></html>`);
  assert.equal(none.c, 'rgb(201, 25, 11)', 'the fallback #c9190b in BASE_CSS');
});

test('without base_css the host owns the hidden live region: BASE_CSS is what hides .prose-live', { skip, timeout: 120_000 }, () => {
  const live = '<article class="ds-prose prose-edit"><div class="prose-live" role="status" aria-live="polite">added</div></article>';
  const without = inChrome(page('light', live, '', `const r = document.querySelector('.prose-live').getBoundingClientRect(); out({ w: r.width, h: r.height });`));
  assert.ok(without.w > 5 && without.h > 5, 'unstyled, the announcer is visible');
  const withCss = inChrome(page('light', live, BASE_CSS, `const r = document.querySelector('.prose-live').getBoundingClientRect(); out({ w: r.width, h: r.height });`));
  assert.ok(withCss.w <= 1 && withCss.h <= 1, 'BASE_CSS clips it');
});
