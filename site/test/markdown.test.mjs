import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { renderMarkdown, parseFrontMatter } from '../lib/markdown.mjs';

const md = (s, o) => renderMarkdown(s, o).html;

test('headings get slugified ids, duplicates are numbered', () => {
  const { html, headings } = renderMarkdown('# Title\n\n## Set up\n\n## Set up\n\n### CSS hooks (stable)\n');
  assert.match(html, /<h1 id="title">Title<\/h1>/);
  assert.match(html, /<h2 id="set-up">Set up/);
  assert.match(html, /<h2 id="set-up-1">Set up/);
  assert.match(html, /<h3 id="css-hooks-stable">CSS hooks \(stable\)/);
  assert.deepEqual(headings.map((h) => [h.level, h.id, h.text]), [
    [1, 'title', 'Title'],
    [2, 'set-up', 'Set up'],
    [2, 'set-up-1', 'Set up'],
    [3, 'css-hooks-stable', 'CSS hooks (stable)'],
  ]);
});

test('h2 and h3 carry an anchor link that points at their own id', () => {
  const html = md('## Install\n');
  assert.match(html, /<a class="anchor" href="#install" aria-label="Link to Install">#<\/a>/);
});

test('text is escaped: angle brackets, ampersands, quotes', () => {
  assert.equal(md('a < b & "c"'), '<p>a &lt; b &amp; &quot;c&quot;</p>\n');
});

test('raw HTML is escaped, not passed through', () => {
  const html = md('<script>alert(1)</script>\n\n<div class="x">y</div>');
  assert.ok(!html.includes('<script'));
  assert.ok(!html.includes('<div class="x"'));
  assert.match(html, /&lt;script&gt;/);
});

test('inline code, including a backtick inside double-backtick code', () => {
  assert.equal(md('use `a<b` here'), '<p>use <code>a&lt;b</code> here</p>\n');
  assert.equal(md('x `` a`b `` y'), '<p>x <code>a`b</code> y</p>\n');
});

test('emphasis and strong, not inside code', () => {
  assert.equal(md('**bold** and *em* and `*raw*`'), '<p><strong>bold</strong> and <em>em</em> and <code>*raw*</code></p>\n');
});

test('links: relative, fragment, https; unsafe schemes become text', () => {
  assert.match(md('[a](/yew/)'), /<a href="\/yew\/">a<\/a>/);
  assert.match(md('[a](#x)'), /<a href="#x">a<\/a>/);
  assert.match(md('[a](https://example.org/?q=1&r=2)'), /<a href="https:\/\/example\.org\/\?q=1&amp;r=2">a<\/a>/);
  const bad = md('[a](javascript:alert(1))');
  assert.ok(!bad.includes('<a '));
  assert.ok(!bad.includes('href'));
});

test('fenced code carries a language class, escaped content and a copy button', () => {
  const html = md('```rust\nlet a = b < c;\n```\n');
  assert.match(html, /<div class="code">/);
  assert.match(html, /<button type="button" class="copy" data-copy>Copy<\/button>/);
  assert.match(html, /<pre tabindex="0" role="region" aria-label="Code: rust"><code class="language-rust">let a = b &lt; c;\n<\/code><\/pre>/);
});

test('fenced code without a language is labelled plainly', () => {
  assert.match(md('```\nx\n```\n'), /aria-label="Code"><code>x\n<\/code>/);
});

test('a fence is not closed by a shorter one, and markdown inside is untouched', () => {
  const html = md('````md\n```js\n# not a heading\n```\n````\n');
  assert.ok(!html.includes('<h1'));
  assert.match(html, /```js\n# not a heading\n```\n/);
});

test('tables: header scope, alignment classes, scroll wrapper', () => {
  const html = md('| Name | Type | N |\n|:--|:-:|--:|\n| `a` | b | 1 |\n');
  assert.match(html, /<div class="table-scroll" tabindex="0" role="region" aria-label="Table">/);
  assert.match(html, /<th scope="col">Name<\/th>/);
  assert.match(html, /<th scope="col" class="align-center">Type<\/th>/);
  assert.match(html, /<th scope="col" class="align-right">N<\/th>/);
  assert.match(html, /<td><code>a<\/code><\/td>/);
  assert.match(html, /<td class="align-right">1<\/td>/);
});

test('a pipe inside inline code does not split a cell when escaped', () => {
  const html = md('| A |\n|---|\n| `x\\|y` |\n');
  assert.match(html, /<td><code>x\|y<\/code><\/td>/);
});

test('lists: unordered, ordered, one nesting level', () => {
  const html = md('- one\n- two\n  - inner a\n  - inner b\n- three\n\n1. first\n2. second\n');
  assert.match(html, /<ul>\s*<li>one<\/li>\s*<li>two\s*<ul>\s*<li>inner a<\/li>\s*<li>inner b<\/li>\s*<\/ul>\s*<\/li>\s*<li>three<\/li>\s*<\/ul>/);
  assert.match(html, /<ol>\s*<li>first<\/li>\s*<li>second<\/li>\s*<\/ol>/);
});

test('blockquote and horizontal rule', () => {
  assert.match(md('> quoted *text*\n> more'), /<blockquote>\s*<p>quoted <em>text<\/em>\s+more<\/p>\s*<\/blockquote>/);
  assert.match(md('a\n\n---\n\nb'), /<hr>/);
});

test('front matter: key value lines, body returned', () => {
  const { data, body } = parseFrontMatter('---\ntitle: Yew docs\nnav: yew\norder: 2\n---\n# Hi\n');
  assert.deepEqual(data, { title: 'Yew docs', nav: 'yew', order: '2' });
  assert.equal(body, '# Hi\n');
  assert.deepEqual(parseFrontMatter('# no front matter\n').data, {});
});

function tmpContent(files) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'md-'));
  for (const [name, text] of Object.entries(files)) {
    fs.mkdirSync(path.dirname(path.join(dir, name)), { recursive: true });
    fs.writeFileSync(path.join(dir, name), text);
  }
  return dir;
}

test('{{file}} embeds a file as a code block, language from the extension', () => {
  const dir = tmpContent({ 'examples/prose.css': 'a { color: red; }\n' });
  const html = md('{{file examples/prose.css}}\n', { contentDir: dir });
  assert.match(html, /<code class="language-css">a \{ color: red; \}\n<\/code>/);
});

test('{{include}} renders a partial in place, headings included', () => {
  const dir = tmpContent({ '_partials/hooks.md': '## Shared\n\nShared text.\n' });
  const { html, headings } = renderMarkdown('# Page\n\n{{include hooks}}\n', { contentDir: dir });
  assert.match(html, /<h2 id="shared">Shared/);
  assert.match(html, /<p>Shared text\.<\/p>/);
  assert.deepEqual(headings.map((h) => h.id), ['page', 'shared']);
});

test('an unknown directive fails the build', () => {
  assert.throws(() => md('{{bogus x}}\n'), /unknown directive.*bogus/i);
});

test('a missing file or partial fails the build', () => {
  const dir = tmpContent({});
  assert.throws(() => md('{{file nope.css}}', { contentDir: dir }), /nope\.css/);
  assert.throws(() => md('{{include nope}}', { contentDir: dir }), /nope/);
});

test('{{file}} cannot read outside the content directory', () => {
  const dir = tmpContent({});
  assert.throws(() => md('{{file ../../../etc/passwd}}', { contentDir: dir }), /outside/i);
});
