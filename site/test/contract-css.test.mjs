// The CSS hooks pages against the sources, and the example stylesheets
// against the hooks pages and the contrast floor.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { SITE_DIR, buildSite, read } from './helpers.mjs';
import { REPO, content, frontMatterless, hookClasses, sections, source, tableRows, walkFiles } from './contract.mjs';
import { blockAfter, decls, hex, ratio } from './css.mjs';

const withIncludes = (md) => md.replace(/\{\{include (\S+)\}\}/g, (_, n) => frontMatterless(content(`_partials/${n}.md`)));
const yewHooks = withIncludes(frontMatterless(content('yew/css-hooks.md')));
const ngHooks = withIncludes(frontMatterless(content('angular/css-hooks.md')));

const CLASS_RE = /\.(ds-prose|prose-[a-z][a-z-]*[a-z])\b/g;
const classesIn = (text) => new Set([...text.matchAll(CLASS_RE)].map((m) => m[1]));
const firstCells = (body) => tableRows(body).map((r) => r[0] ?? '').join('\n');

/** Hook classes in the first column of the tables under headings that start with `prefix`. */
function documented(md, prefix) {
  const out = new Set();
  for (const [h, body] of sections(md)) if (h.startsWith(prefix)) classesIn(firstCells(body)).forEach((c) => out.add(c));
  return out;
}

const SRC_FILES = walkFiles(path.join(REPO, 'prose-yew/src'), (p) => p.endsWith('.rs') && !p.endsWith('css.rs'));
const emittedByView = new Set(SRC_FILES.flatMap((f) => [...hookClasses(fs.readFileSync(f, 'utf8'))]));
const emittedByReader = hookClasses(source('prose-yew/src/lib.rs'));

const stable = documented(yewHooks, 'Stable');
const internal = documented(yewHooks, 'Internal');

test('the Yew hooks page has Stable and Internal sections, and says the split is a recommendation', () => {
  assert.ok(stable.size > 40, `only ${stable.size} stable hooks`);
  assert.ok(internal.size >= 1);
  assert.match(yewHooks, /recommendation/i);
  assert.match(yewHooks, /no CSS stability policy|does not state a CSS stability policy|states no stability policy/i);
});

test('a hook is Stable or Internal, never both', () => {
  for (const c of internal) assert.ok(!stable.has(c), `${c} is listed as both`);
});

test('every documented class is emitted by the code as a string literal', () => {
  for (const c of [...stable, ...internal]) assert.ok(emittedByView.has(c), `.${c} is documented but no string literal in prose-yew/src has it`);
});

test('every class the code emits is documented as Stable or Internal', () => {
  for (const c of emittedByView) assert.ok(stable.has(c) || internal.has(c), `.${c} is emitted but not documented`);
});

test('the read-only renderer partial lists exactly the classes lib.rs emits', () => {
  const part = withIncludes('{{include hooks-renderer}}');
  const listed = documented(part, 'Stable');
  assert.deepEqual([...listed].sort(), [...emittedByReader].sort());
});

test('the Angular hooks page documents the renderer classes and no editor class', () => {
  const listed = documented(ngHooks, 'Stable');
  assert.deepEqual([...listed].sort(), [...emittedByReader].sort());
  for (const editorOnly of ['prose-edit', 'prose-slot', 'prose-add', 'prose-problems']) assert.ok(!listed.has(editorOnly));
  assert.match(ngHooks, /editor is not available in Angular|no editor in the Angular package/i);
});

test('custom properties: the docs list every --prose-* name in the sources and no other', () => {
  const found = new Set();
  for (const dir of ['prose-yew/src', 'prose-wasm/src', 'prose-angular/src'])
    for (const f of walkFiles(path.join(REPO, dir), (p) => /\.(rs|ts)$/.test(p))) for (const m of fs.readFileSync(f, 'utf8').matchAll(/--prose-[a-z]+/g)) found.add(m[0]);
  const docs = new Set([...yewHooks.matchAll(/`(--prose-[a-z]+)`/g)].map((m) => m[1]));
  assert.deepEqual([...docs].sort(), [...found].sort());
  assert.deepEqual([...found].sort(), ['--prose-depth', '--prose-error', '--prose-warning']);
  const internalSection = [...sections(yewHooks)].find(([h]) => h.startsWith('Internal'))[1];
  assert.ok(internalSection.includes('`--prose-depth`'), '--prose-depth must be Internal');
});

test('the read-only renderer reads no custom property: the docs say so, the code agrees', () => {
  const css = source('prose-yew/src/edit/css.rs');
  assert.match(css, /var\(--prose-error/);
  assert.ok(!fs.readFileSync(path.join(REPO, 'prose-yew/src/lib.rs'), 'utf8').includes('--prose-'));
  assert.match(yewHooks, /read-only renderer reads no custom propert/i);
});

test('data attributes named in the docs exist in the sources', () => {
  const all = SRC_FILES.map((f) => fs.readFileSync(f, 'utf8')).join('\n');
  const attrs = new Set([...(yewHooks + ngHooks).matchAll(/`\[?(data-[a-z]+)/g)].map((m) => m[1]));
  assert.ok(attrs.size >= 5);
  for (const a of attrs) assert.ok(all.includes(a), `${a} is not in prose-yew/src`);
});

test('BASE_CSS: every rule starts at .ds-prose.prose-edit, as the styling page says', () => {
  const css = /pub const BASE_CSS: &str = r#"([\s\S]*?)"#;/.exec(source('prose-yew/src/edit/css.rs'))[1].replace(/\/\*[\s\S]*?\*\//g, '');
  const preludes = [...css.matchAll(/(?:^|\})\s*([^{}@][^{}]*)\{/g)].map((m) => m[1]);
  assert.ok(preludes.length > 20);
  for (const p of preludes) for (const sel of p.split(',')) assert.match(sel.trim(), /^\.ds-prose\.prose-edit\b/, sel);
  assert.match(frontMatterless(content('yew/styling.md')), /`\.ds-prose\.prose-edit`/);
});

// --- the example stylesheets ---------------------------------------------------------

const PROSE = content('examples/prose.css');
const EDITOR = content('examples/prose-editor.css');

test('the example stylesheets use only documented hooks that the code emits', () => {
  for (const [name, css] of [['prose.css', PROSE], ['prose-editor.css', EDITOR]]) {
    const used = classesIn(css.replace(/\/\*[\s\S]*?\*\//g, ''));
    assert.ok(used.size > 10, name);
    for (const c of used) {
      assert.ok(emittedByView.has(c), `${name}: .${c} is not emitted by the code`);
      assert.ok(stable.has(c), `${name}: .${c} is not a Stable hook`);
    }
  }
});

test('prose.css sets color-scheme, the dark palette under both selectors, and the two library properties', () => {
  assert.match(PROSE, /:root\s*\{[^}]*color-scheme:\s*light dark;/);
  assert.match(PROSE, /@media \(prefers-color-scheme: dark\)\s*\{\s*:root:not\(\[data-theme="light"\]\)/);
  assert.match(PROSE, /:root\[data-theme="dark"\]\s*\{/);
  assert.match(PROSE, /--prose-error:\s*var\(--ds-deny\)/);
  assert.match(PROSE, /--prose-warning:\s*var\(--ds-warn\)/);
});

function palettes(css) {
  const light = decls(blockAfter(css, ':root {').body);
  const media = css.indexOf('@media (prefers-color-scheme: dark)');
  const dark = decls(blockAfter(css, ':root:not([data-theme="light"])', media).body);
  const forced = decls(blockAfter(css, ':root[data-theme="dark"]').body);
  return { light, dark, forced };
}

test('the palettes define the same tokens; the forced dark palette equals the media-query one', () => {
  const { light, dark, forced } = palettes(PROSE);
  const own = (o) => Object.keys(o).filter((k) => k.startsWith('--ds-')).sort();
  assert.deepEqual(own(dark), own(forced));
  for (const k of own(dark)) assert.equal(dark[k], forced[k], `${k}`);
  for (const k of own(dark)) assert.ok(k in light, `${k} is missing from the light palette`);
  const used = new Set([...(PROSE + EDITOR).matchAll(/var\((--ds-[a-z-]+)/g)].map((m) => m[1]));
  for (const k of used) assert.ok(k in light, `${k} is used but not defined`);
});

test('text tokens reach 4.5:1 on the example background in light and dark', () => {
  const { light, dark } = palettes(PROSE);
  for (const [name, p] of [['light', light], ['dark', dark]]) {
    for (const k of ['--ds-fg', '--ds-muted', '--ds-allow', '--ds-deny', '--ds-duty', '--ds-warn']) {
      const r = ratio(hex(p[k]), hex(p['--ds-bg']));
      assert.ok(r >= 4.5, `${name} ${k} on --ds-bg is ${r.toFixed(2)}`);
    }
    const sel = ratio(hex(p['--ds-selected-fg']), hex(p['--ds-selected-bg']));
    assert.ok(sel >= 4.5, `${name} selected text ${sel.toFixed(2)}`);
  }
});

test('built site: the hooks pages embed both stylesheets verbatim, and /examples/ ships the same bytes', () => {
  const site = buildSite();
  const text = (html) => [...html.matchAll(/<code class="language-css">([\s\S]*?)<\/code>/g)].map((m) => m[1].replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&amp;/g, '&'));
  const yew = text(read(path.join(site.out, 'yew/css-hooks/index.html')));
  const ng = text(read(path.join(site.out, 'angular/css-hooks/index.html')));
  assert.ok(yew.includes(PROSE) && yew.includes(EDITOR), 'Yew hooks page');
  assert.ok(ng.includes(PROSE) && !ng.includes(EDITOR), 'Angular hooks page has prose.css only');
  for (const f of ['prose.css', 'prose-editor.css']) assert.equal(read(path.join(site.out, 'examples', f)), content(`examples/${f}`));
});
