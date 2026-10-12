// The writing rules for the docs: short declarative sentences, no marketing
// words, no filler. Checked on the source markdown of every page.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { SITE_DIR, walk } from './helpers.mjs';

const pages = walk(path.join(SITE_DIR, 'content'), (p) => p.endsWith('.md')).map((f) => [path.relative(SITE_DIR, f), fs.readFileSync(f, 'utf8')]);

/** Running text only: no front matter, fenced code, tables, directives or headings. */
function prose(md) {
  return md
    .replace(/^---\n[\s\S]*?\n---\n/, '')
    .replace(/```[\s\S]*?```/g, '')
    .split('\n')
    .filter((l) => !/^(\||\{\{|#|\s*$)/.test(l))
    .join('\n')
    .replace(/\n(?=[-*] )/g, '\n')
    .replace(/^[-*] /gm, '');
}

const sentences = (text) => text.split(/(?<=[.!?:])\s+(?=[A-Z`])/).map((s) => s.replace(/\s+/g, ' ').trim()).filter(Boolean);

test('no sentence of running text is longer than 40 words, unless it is mostly a list of identifiers', () => {
  let n = 0;
  for (const [file, md] of pages) {
    for (const s of sentences(prose(md))) {
      const words = s.replace(/\[([^\]]*)\]\([^)]*\)/g, '$1').split(' ');
      const code = words.filter((w) => w.startsWith('`')).length;
      n++;
      if (code * 2 > words.length) continue;
      assert.ok(words.length <= 40, `${file}: ${words.length} words: ${s.slice(0, 120)}...`);
    }
  }
  assert.ok(n > 300, `only ${n} sentences read`);
});

const BANNED = ['simply', 'just', 'easily', 'easy', 'powerful', 'seamless', 'seamlessly', 'robust', 'blazing', 'leverage', 'elegant', 'effortless', 'comprehensive', 'beautiful', 'modern', 'lightweight', 'world-class', 'out of the box', 'state-of-the-art', 'straightforward', 'basically', 'obviously', 'of course', 'flexible', 'production-ready', 'delightful', 'magic', 'please note', 'it is worth noting', 'note that'];

test('no marketing words and no filler phrases in running text', () => {
  const re = new RegExp(`\\b(${BANNED.join('|')})\\b`, 'i');
  for (const [file, md] of pages) {
    const text = prose(md).replace(/`[^`]*`/g, '');
    const m = re.exec(text);
    assert.ok(!m, `${file}: "${m?.[0]}" in: ${m && text.slice(Math.max(0, m.index - 50), m.index + 60).replace(/\n/g, ' ')}`);
  }
});

test('headings are nouns or short phrases: no heading ends in a colon or a question mark, none is a filler', () => {
  for (const [file, md] of pages) {
    for (const h of md.matchAll(/^#{1,4} (.+)$/gm)) {
      assert.doesNotMatch(h[1], /[:?!]$/, `${file}: heading "${h[1]}"`);
      assert.doesNotMatch(h[1], /^(introduction|overview|getting started|conclusion|summary)$/i, `${file}: filler heading "${h[1]}"`);
    }
  }
});
