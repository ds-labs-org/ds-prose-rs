// Numbers, colours and ranges that the pages state in running text, tied to
// the constants in the sources. A mutation check showed that each could be
// edited in the docs without a test noticing.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { content, frontMatterless, macroIdents, sections, source, tableRows, tick } from './contract.mjs';

const withIncludes = (md) => md.replace(/\{\{include (\S+)\}\}/g, (_, n) => frontMatterless(content(`_partials/${n}.md`)));
const page = (p) => withIncludes(frontMatterless(content(p)));
const constant = (file, re) => Number(re.exec(source(file))[1]);

test('the undo history cap and the refusal window are the constants of the code', () => {
  const md = page('yew/interface.md');
  assert.ok(md.includes(`capped at ${constant('prose-yew/src/edit/hook.rs', /const HISTORY_CAP: usize = (\d+);/)} entries`));
  assert.ok(md.includes(`within ${constant('prose-yew/src/edit/view.rs', /const PENDING_WINDOW_MS: i32 = (\d+);/)} ms`));
});

test('the glossary limit is one number in prose-core and prose-yew, and the pages say it', () => {
  const yew = constant('prose-yew/src/lib.rs', /const GLOSSARY_MAX: usize = (\d+);/);
  assert.equal(yew, constant('prose-core/src/read.rs', /const MAX_GLOSSARY: usize = (\d+);/));
  assert.ok(page('yew/interface.md').includes(`at most ${yew} entries`));
  assert.ok(page('yew/css-hooks.md').includes(`At most ${yew} entries`));
  assert.ok(page('angular/css-hooks.md').includes(`At most ${yew} entries`));
});

test('the fallback colours of the two custom properties are those of BASE_CSS, on the styling page and on the hooks page', () => {
  const css = source('prose-yew/src/edit/css.rs');
  const fallback = (name) => new RegExp(`var\\(${name},\\s*(#[0-9a-f]{6})\\)`, 'i').exec(css)[1];
  const err = fallback('--prose-error');
  const warn = fallback('--prose-warning');
  assert.ok(page('yew/styling.md').includes(`\`${err}\` and \`${warn}\``));
  const rows = tableRows(sections(page('yew/css-hooks.md')).get('Custom properties'));
  assert.equal(tick(rows.find((r) => tick(r[0]) === '--prose-error')[2]), err);
  assert.equal(tick(rows.find((r) => tick(r[0]) === '--prose-warning')[2]), warn);
});

test('the number of ProseClasses fields on the styling page is the number class_fields! declares', () => {
  const n = macroIdents(source('prose-yew/src/edit/config.rs'), 'class_fields').length;
  assert.ok(page('yew/styling.md').includes(`have the same ${n} fields`));
});

test('the Cargo features in the table are the features of prose-yew, with what they enable', () => {
  const cargo = source('prose-yew/Cargo.toml');
  const rows = tableRows(sections(page('yew/index.md')).get('Features') ?? page('yew/index.md').split('\n').filter((l) => l.startsWith('|')).join('\n'));
  for (const [name, enables] of [['csr', 'yew/csr'], ['ssr', 'yew/ssr']]) {
    assert.match(cargo, new RegExp(`^${name} = \\["${enables}"\\]`, 'm'));
    const row = rows.find((r) => tick(r[0]) === name);
    assert.ok(row, `no row for ${name}`);
    assert.equal(tick(row[1]), enables);
  }
});

test('the Angular major version on the overview page is the major of the peer range', () => {
  const peer = JSON.parse(source('prose-angular/package.json')).peerDependencies['@angular/core'];
  const major = /(\d+)\.\d+\.\d+/.exec(peer)[1];
  const md = page('angular/index.md');
  assert.ok(md.includes(`Angular ${major} or newer`), `the page does not say "Angular ${major} or newer"`);
});

test('the tree on the Angular hooks page names only classes the renderer writes', () => {
  const tree = /```text\n([\s\S]*?)```/.exec(page('angular/css-hooks.md'))[1];
  const written = new Set([...source('prose-yew/src/lib.rs').matchAll(/"((?:ds-prose|prose-[a-z-]+))"/g)].map((m) => m[1]));
  const named = [...tree.matchAll(/\.((?:ds-prose|prose-[a-z-]+))/g)].map((m) => m[1]);
  assert.ok(named.length > 12);
  for (const c of named) assert.ok(written.has(c), `the tree names .${c}, which lib.rs does not write`);
});
