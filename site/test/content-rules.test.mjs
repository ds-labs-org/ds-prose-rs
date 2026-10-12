import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { SITE_DIR, buildSite, htmlFiles, read, walk } from './helpers.mjs';
import { CSS, blockAfter } from './css.mjs';

const JS = fs.readFileSync(path.join(SITE_DIR, 'assets/site.js'), 'utf8');
const EMOJI = /[\p{Extended_Pictographic}\u{1F1E6}-\u{1F1FF}\u{FE0F}\u{200D}]/u;

test('no emoji in any site source file', () => {
  const files = walk(SITE_DIR, (p) => !p.includes('node_modules') && !p.includes(`${path.sep}dist${path.sep}`) && /\.(md|mjs|js|css|json|html)$/.test(p));
  assert.ok(files.length > 10);
  for (const f of files) assert.doesNotMatch(fs.readFileSync(f, 'utf8'), EMOJI, f);
});

test('no emoji in the built output', () => {
  const site = buildSite();
  for (const f of htmlFiles(site.out)) assert.doesNotMatch(read(f), EMOJI, f);
});

test('tokens: colour scheme, dark palette under both selectors, reduced motion, focus ring', () => {
  assert.match(CSS, /:root\s*\{[^}]*color-scheme:\s*light dark;/);
  assert.match(CSS, /:root\[data-theme="light"\]\s*\{[^}]*color-scheme:\s*light;/);
  assert.match(CSS, /:root\[data-theme="dark"\]\s*\{[^}]*color-scheme:\s*dark;/);
  assert.match(CSS, /@media \(prefers-color-scheme: dark\)\s*\{\s*:root:not\(\[data-theme="light"\]\)/);
  assert.match(CSS, /@media \(prefers-reduced-motion: reduce\)/);
  assert.match(CSS, /:focus-visible\s*\{[^}]*outline:\s*3px solid var\(--focus\)/);
});

test('body paints an explicit background and colour from tokens', () => {
  assert.match(CSS, /body\.site\s*\{[^}]*background:\s*var\(--bg\);[^}]*color:\s*var\(--text\)/);
});

test('type: system stacks only, tabular numerals, 65ch measure, no webfonts, no listed fonts', () => {
  assert.match(CSS, /font-variant-numeric:\s*tabular-nums/);
  assert.match(CSS, /max-width:\s*65ch/);
  assert.match(CSS, /system-ui/);
  assert.match(CSS, /ui-monospace/);
  assert.doesNotMatch(CSS, /@import|@font-face|url\(/);
  assert.doesNotMatch(CSS, /\b(Inter|Space Grotesk|Roboto Mono|Fira)\b/);
});

test('no gradients, no outline removal, no box shadows', () => {
  assert.doesNotMatch(CSS, /gradient\(/);
  assert.doesNotMatch(CSS, /outline:\s*(none|0)\b/);
  assert.doesNotMatch(CSS, /box-shadow:(?!\s*none)/);
});

test('reduced motion turns off transitions and smooth scrolling', () => {
  const b = blockAfter(CSS, '@media (prefers-reduced-motion: reduce)');
  assert.ok(b);
  assert.match(b.body, /transition:\s*none/);
  assert.match(b.body, /scroll-behavior:\s*auto/);
});

test('every class the shell emits has a rule in site.css', () => {
  for (const c of ['skip-link', 'site-header', 'topnav', 'brand', 'menu-toggle', 'topnav-links', 'topnav-list', 'pkg-group', 'theme-toggle', 'subnav', 'layout', 'code', 'copy', 'table-scroll', 'align-right', 'align-center', 'anchor', 'site-footer'])
    assert.ok(new RegExp(`\\.${c}\\b`).test(CSS), `no rule for .${c}`);
});

test('the nav is sticky and the menu collapses below 48rem', () => {
  assert.match(CSS, /\.site-header\s*\{[^}]*position:\s*sticky;[^}]*top:\s*0/);
  assert.match(CSS, /@media \(max-width: 47\.99rem\)/);
});

test('the skip link is hidden until focused', () => {
  assert.match(CSS, /\.skip-link\s*\{[^}]*position:\s*absolute/);
  assert.match(CSS, /\.skip-link:focus\s*\{/);
});

test('long tokens wrap rather than widen the page', () => {
  assert.match(CSS, /overflow-wrap:\s*anywhere/);
  assert.match(CSS, /\.code pre\s*\{[^}]*overflow:\s*auto/);
  assert.match(CSS, /\.table-scroll\s*\{[^}]*overflow-x:\s*auto/);
  assert.doesNotMatch(CSS, /overflow-x:\s*(clip|hidden)\s*;?\s*\}\s*\n*\s*body/);
});

test('every use of web storage sits inside a try block (site.js and the inline theme script)', () => {
  const nav = fs.readFileSync(path.join(SITE_DIR, 'lib/nav.mjs'), 'utf8');
  for (const [name, src] of [['site.js', JS], ['nav.mjs', nav]]) {
    let n = 0;
    for (const m of src.matchAll(/\b(localStorage|sessionStorage)\b/g)) {
      n++;
      const before = src.slice(0, m.index);
      const lastTry = Math.max(before.lastIndexOf('try {'), before.lastIndexOf('try{'));
      const lastCatch = Math.max(before.lastIndexOf('catch'), -1);
      assert.ok(lastTry > lastCatch, `${name}: ${m[0]} at ${m.index} is not inside try`);
    }
    assert.ok(n > 0, `${name} never touches storage`);
  }
});

test('site.js never writes markup from strings or evaluates code', () => {
  assert.doesNotMatch(JS, /\binnerHTML\b|\beval\(|new Function|document\.write/);
});

test('site.js wires the three behaviours the shell promises', () => {
  assert.match(JS, /\.menu-toggle/);
  assert.match(JS, /\.theme-toggle/);
  assert.match(JS, /\[data-copy\]/);
  assert.match(JS, /aria-expanded/);
  assert.match(JS, /Escape/);
  assert.match(JS, /ds-prose-theme/);
});

test('code in table cells is not broken mid-identifier; the table scrolls instead', () => {
  assert.match(CSS, /\.table-scroll code\s*\{[^}]*white-space:\s*nowrap/);
});
