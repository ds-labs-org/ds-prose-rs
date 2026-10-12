// Claims of the hooks pages that the table tests do not read: the element each
// hook is written on, the wording of the rule-kind select, and the one
// error element that the Angular component writes outside the article.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { REPO, content, frontMatterless, sections, source, tableRows, walkFiles } from './contract.mjs';

const withIncludes = (md) => md.replace(/\{\{include (\S+)\}\}/g, (_, n) => frontMatterless(content(`_partials/${n}.md`)));
const yewHooks = withIncludes(frontMatterless(content('yew/css-hooks.md')));
const ngHooks = withIncludes(frontMatterless(content('angular/css-hooks.md')));

/** class -> set of tags it is written on, for every `<tag ... "class">` in the html! blocks of prose-yew/src. */
function tagsInCode() {
  const found = new Map();
  for (const f of walkFiles(path.join(REPO, 'prose-yew/src'), (p) => p.endsWith('.rs') && !p.endsWith('css.rs'))) {
    const text = fs.readFileSync(f, 'utf8');
    for (const m of text.matchAll(/<([a-z][a-z0-9]*)\b((?:[^<>{}]|\{[^{}]*(?:\{[^{}]*\}[^{}]*)*\})*)>/g)) {
      for (const c of m[2].matchAll(/"(ds-prose|prose-[a-z-]+)"/g)) {
        if (!found.has(c[1])) found.set(c[1], new Set());
        found.get(c[1]).add(m[1]);
      }
    }
  }
  return found;
}

test('the Element column of the Stable tables names the tags the code writes the class on', () => {
  const found = tagsInCode();
  let checked = 0;
  for (const [heading, body] of sections(yewHooks)) {
    if (!heading.startsWith('Stable')) continue;
    for (const row of tableRows(body)) {
      const tags = new Set([...row[1].matchAll(/`([a-z0-9]+)(?:[[.]|`)/g)].map((m) => m[1]));
      for (const sel of [...row[0].matchAll(/`\.([a-z-]+)/g)].map((m) => m[1])) {
        const code = found.get(sel);
        if (!code) continue; // the class is built from a variable, or has no tag in the same literal
        checked++;
        assert.deepEqual([...tags].sort(), [...code].sort(), `.${sel}: the page says ${[...tags]}, the code writes ${[...code]}`);
      }
    }
  }
  assert.ok(checked > 30, `only ${checked} rows could be compared`);
});

test('.prose-rule-kind: the page names the options the select has (the labels may, must not, must)', () => {
  const config = source('prose-yew/src/edit/config.rs');
  const options = ['may', 'must_not', 'must'].map((k) => new RegExp(`\\b${k}: a\\("([^"]+)"\\)`).exec(config)[1]);
  assert.deepEqual(options, ['may', 'must not', 'must']);
  const row = tableRows(sections(yewHooks).get('Stable hooks: editor view')).find((r) => r[0].includes('.prose-rule-kind'));
  assert.ok(row, 'no row for .prose-rule-kind');
  for (const o of options) assert.ok(row[2].includes(o), `the row does not name "${o}": ${row[2]}`);
  assert.doesNotMatch(row[2], /must select/);
});

test('the load error: the component writes a p.prose-error as a child of the host, outside article.ds-prose, and the pages and prose.css say so', () => {
  const component = source('prose-angular/src/odrl-prose.component.ts');
  assert.match(component, /this\.host\.replaceChildren\(message\)/);
  assert.match(component, /message\.className = 'prose-error'/);
  assert.match(ngHooks, /`ds-odrl-prose > \.prose-error`/, 'the Angular hooks page does not name the selector for the load error');
  assert.match(ngHooks, /outside `article\.ds-prose`|not inside `article\.ds-prose`/);
  assert.match(frontMatterless(content('angular/styling.md')) + frontMatterless(content('angular/interface.md')), /ds-odrl-prose > \.prose-error/);
  assert.match(content('examples/prose.css'), /^ds-odrl-prose > \.prose-error\s*\{/m, 'prose.css does not style the load error');
});
