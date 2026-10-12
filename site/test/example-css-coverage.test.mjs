// The example stylesheets, rule by rule, over the renderer's real output.
// example-css.test.mjs checks the rule kinds and the editor cascade; this file
// covers the rest of prose.css and prose-editor.css, so that editing any
// colour, weight or selector in them fails a test. Mutation checks on the
// first version of the docs showed each of these edits surviving.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { content, source } from './contract.mjs';
import { chrome, inChrome, reporter } from './chrome.mjs';
import { blockAfter, decls } from './css.mjs';

const skip = chrome ? false : 'no Chrome found (set CHROME)';
const PROSE = content('examples/prose.css');
const EDITOR = content('examples/prose-editor.css');
const BASE_CSS = /pub const BASE_CSS: &str = r#"([\s\S]*?)"#;/.exec(source('prose-yew/src/edit/css.rs'))[1];
const snapshot = (name) => source(`prose-yew/tests/snapshots/${name}`);

const palette = (css, media) => {
  if (!media) return decls(blockAfter(css, ':root {').body);
  return decls(blockAfter(css, ':root:not([data-theme="light"])', css.indexOf('@media (prefers-color-scheme: dark)')).body);
};
const LIGHT = palette(PROSE);
const DARK = palette(PROSE, true);
const rgb = (h) => `rgb(${[1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16)).join(', ')})`;

/** Top-level rules of a stylesheet without at-rules: [{ selectors, body }]. */
function rules(css) {
  const clean = css.replace(/\/\*[\s\S]*?\*\//g, '');
  const out = [];
  let depth = 0;
  let start = 0;
  let prelude = '';
  for (let i = 0; i < clean.length; i++) {
    if (clean[i] === '{') {
      if (depth === 0) prelude = clean.slice(start, i).trim();
      depth++;
    } else if (clean[i] === '}' && --depth === 0) {
      if (!prelude.startsWith('@')) out.push({ selectors: prelude.split(',').map((s) => s.trim()), body: clean.slice(clean.lastIndexOf('{', i) + 1, i) });
      start = i + 1;
    }
  }
  return out;
}

test('prose-editor.css: every selector starts with ":root " so that it outranks BASE_CSS by one point', () => {
  const all = rules(EDITOR);
  assert.ok(all.length > 20);
  for (const { selectors } of all) {
    for (const s of selectors) {
      if (s.startsWith('.prose-problem')) continue; // the problems panel has no BASE_CSS rules to outrank
      assert.ok(s.startsWith(':root .ds-prose.prose-edit'), `selector without the :root prefix: ${s}`);
    }
  }
});

test('prose-editor.css: the selectors without the prefix are exactly those of the problems panel', () => {
  const bare = rules(EDITOR).flatMap((r) => r.selectors).filter((s) => !s.startsWith(':root '));
  assert.ok(bare.length > 8);
  for (const s of bare) assert.match(s, /^\.prose-problem/, s);
  assert.ok(!BASE_CSS.includes('.prose-problem'), 'BASE_CSS now styles the problems panel: the prefix is needed there too');
});

// Fragments. Real ones come from the snapshots the Rust tests pin; the
// synthetic ones cover classes no snapshot has, and use the markup lib.rs writes.
const FRAGMENTS = {
  offer: snapshot('offer.html'),
  violations: snapshot('strict_read_violations.html'),
  glossary: snapshot('profile_research.html'),
  problems: snapshot('strict_problems.html'),
  synthetic:
    '<article class="ds-prose"><section class="prose-violations"><p class="prose-violations-status prose-conforms" role="status">c</p><p class="prose-violations-status prose-incomplete" role="status">i</p>' +
    '<ul class="prose-violations-list"><li class="prose-violation prose-violation-warning">w</li><li class="prose-violation prose-violation-info">n</li></ul></section>' +
    '<aside class="prose-warnings"><p class="prose-label">Not rendered</p></aside>' +
    '<details class="prose-glossary" open><dl><dt><span>t</span> <code>c</code></dt><dd class="prose-deprecated">d</dd></dl></details>' +
    '<p class="prose-error" role="alert">bad</p><span class="prose-inherited">i</span><p class="prose-empty">e</p><span class="prose-term">t</span></article>' +
    '<section class="prose-problems"><p class="prose-problems-status prose-conforms" role="status">c</p><ol class="prose-problem-groups"><li class="prose-problem-group"><ol class="prose-problem-list">' +
    '<li class="prose-problem prose-problem-warning"><span class="prose-problem-message">w</span><button class="prose-problem-fix" data-safe="false">f</button></li>' +
    '<li class="prose-problem prose-problem-info prose-problem-gap"><span class="prose-problem-message">g</span></li></ol></li></ol></section>',
};

// [fragment, selector, CSS property, expected, what]
const T = (tokens) => ({
  allow: rgb(tokens['--ds-allow']), deny: rgb(tokens['--ds-deny']), warn: rgb(tokens['--ds-warn']), muted: rgb(tokens['--ds-muted']), fg: rgb(tokens['--ds-fg']), border: rgb(tokens['--ds-border']),
});
const CASES = (t) => [
  ['offer', '.prose-notes', 'color', t.muted],
  ['synthetic', '.prose-inherited', 'color', t.muted],
  ['synthetic', '.prose-inherited', 'fontStyle', 'italic'],
  ['synthetic', '.prose-empty', 'color', t.muted],
  ['synthetic', '.prose-term', 'textDecorationLine', 'underline'],
  ['violations', '.prose-violations-found', 'color', t.deny],
  ['violations', '.prose-violation-error', 'color', t.deny],
  ['violations', '.prose-rule-id', 'color', t.muted],
  ['violations', '.prose-warnings', 'borderTopColor', t.warn],
  ['violations', '.prose-glossary dt', 'fontWeight', '600'],
  ['violations', '.prose-glossary dt code', 'color', t.muted],
  ['violations', '.prose-glossary dt code', 'fontWeight', '400'],
  ['violations', '.prose-implies', 'color', t.muted],
  ['violations', '.prose-from', 'color', t.muted],
  ['glossary', '.prose-note', 'color', t.muted],
  ['glossary', '.prose-kind-of', 'color', t.muted],
  ['synthetic', '.prose-violations .prose-conforms', 'color', t.allow],
  ['synthetic', '.prose-violations .prose-incomplete', 'color', t.warn],
  ['synthetic', '.prose-violation-warning', 'color', t.warn],
  ['synthetic', '.prose-violation-info', 'color', t.muted],
  ['synthetic', '.prose-deprecated', 'color', t.warn],
  ['synthetic', 'article .prose-error', 'color', t.deny],
  ['problems', '.prose-problems', 'color', t.fg],
  ['problems', '.prose-problem-error .prose-problem-message', 'color', t.deny],
  ['problems', '.prose-problems-status.prose-violations-found', 'color', t.deny],
  ['problems', '.prose-problems-counts', 'color', t.muted],
  ['problems', '.prose-problem-go', 'borderTopColor', t.border],
  ['synthetic', '.prose-problem-warning .prose-problem-message', 'color', t.warn],
  ['synthetic', '.prose-problem-info .prose-problem-message', 'color', t.muted],
  ['synthetic', '.prose-problem-gap .prose-problem-message', 'fontStyle', 'italic'],
  ['synthetic', '.prose-problems-status.prose-conforms', 'color', t.allow],
  ['synthetic', '.prose-problem-fix[data-safe="false"]', 'borderTopColor', t.warn],
];

const run = (theme, cases, dark = false) => {
  const frags = Object.entries(FRAGMENTS).map(([k, v]) => `<div data-frag="${k}">${v}</div>`).join('');
  const probe = `out(${JSON.stringify(cases.map((c) => c.slice(0, 3)))}.map(([f, sel, prop]) => { const e = document.querySelector('[data-frag="' + f + '"] ' + sel) ?? document.querySelector('[data-frag="' + f + '"]' + sel); return e ? getComputedStyle(e)[prop] : null; }));`;
  return inChrome(
    `<!doctype html><html${theme ? ` data-theme="${theme}"` : ''}><head><meta charset="utf-8"><style>${PROSE}</style><style>${EDITOR}</style></head><body>${frags}${reporter(probe)}</body></html>`,
    { dark },
  );
};

test('prose.css and prose-editor.css over real and synthetic output: every colour and weight they set on a hook class, in light and in dark', { skip, timeout: 180_000 }, () => {
  for (const [theme, tokens] of [['light', LIGHT], ['dark', DARK]]) {
    const cases = CASES(T(tokens));
    const got = run(theme, cases);
    cases.forEach(([frag, sel, prop, want], i) => assert.equal(got[i], want, `${theme}: ${frag} ${sel} ${prop}`));
  }
});

test('both stylesheets: every rule that sets a colour or a weight on a hook class is covered by the table above', () => {
  const covered = new Set(CASES(T(LIGHT)).flatMap((c) => c[1].match(/\.prose-[a-z-]+/g)));
  // The rule kinds and the glossary layout are covered in example-css.test.mjs and below.
  const elsewhere = new Set(['.prose-rule-sentence', '.prose-permission', '.prose-prohibition', '.prose-obligation', '.prose-label', '.prose-rules', '.prose-rule', '.prose-violations-status', '.prose-violations-list', '.prose-glossary', '.prose-policy', '.prose-problem']);
  for (const { selectors, body } of [...rules(PROSE), ...rules(EDITOR)]) {
    if (selectors.every((s) => s.startsWith(':root '))) continue; // editor rules: see the cascade tests
    if (!/(^|[;\s])(color|font-weight|font-style|border(-[a-z]+)?)\s*:/.test(body)) continue;
    for (const s of selectors) {
      for (const c of s.match(/\.prose-[a-z-]+/g) ?? []) assert.ok(covered.has(c) || elsewhere.has(c), `${c} (in "${s}") sets colour or weight and no test reads it`);
    }
  }
});

test('prose.css: ds-odrl-prose is a block, and an unstyled one is inline', { skip, timeout: 120_000 }, () => {
  const probe = `out({ d: cs('ds-odrl-prose', 'display') });`;
  const body = '<ds-odrl-prose><article class="ds-prose"><p>x</p></article></ds-odrl-prose>';
  const styled = inChrome(`<!doctype html><html><head><meta charset="utf-8"><style>${PROSE}</style></head><body>${body}${reporter(probe)}</body></html>`);
  assert.equal(styled.d, 'block');
  const bare = inChrome(`<!doctype html><html><head><meta charset="utf-8"></head><body>${body}${reporter(probe)}</body></html>`);
  assert.equal(bare.d, 'inline');
});

test('prose-editor.css wins over BASE_CSS where both set the same property (the locked part and the issue text)', { skip, timeout: 120_000 }, () => {
  const html = `<!doctype html><html data-theme="light"><head><meta charset="utf-8"><style>${PROSE}</style><style>${EDITOR}</style></head><body>` +
    `<article class="ds-prose prose-view prose-edit"><style>${BASE_CSS}</style><p><code class="prose-locked">{}</code> <span class="prose-issue prose-issue-error">e</span></p></article>` +
    reporter(`out({ locked: cs('.prose-locked', 'borderTopColor'), issue: cs('.prose-issue-error', 'color') });`) + '</body></html>';
  const r = inChrome(html);
  assert.equal(r.locked, rgb(LIGHT['--ds-border']), 'the locked border takes the token, not currentColor');
  assert.equal(r.issue, rgb(LIGHT['--ds-deny']), 'the issue text takes --prose-error from prose.css');
});
