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

// Hostile input. Every tag in the output must be one the renderer emits on
// purpose, with only the attributes it emits, and attribute values must carry
// no markup. Text that looks like markup must end up as text.
const TAGS = new Set(['p', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'a', 'code', 'pre', 'div', 'button', 'em', 'strong', 'ul', 'ol', 'li', 'blockquote', 'hr', 'table', 'thead', 'tbody', 'tr', 'th', 'td']);
const ATTRS = new Set(['href', 'class', 'id', 'type', 'data-copy', 'tabindex', 'role', 'aria-label', 'scope']);
function assertInert(html, label) {
  const TAG = /<(\/?)([a-z0-9]+)((?:\s+[a-z-]+(?:="[^"<>]*")?)*)\s*>/g;
  const rest = html.replace(TAG, (all, close, name, attrs) => {
    assert.ok(TAGS.has(name), `${label}: unexpected tag <${name}> in ${all}`);
    for (const a of attrs.matchAll(/\s([a-z-]+)(?:="([^"]*)")?/g)) {
      assert.ok(ATTRS.has(a[1]), `${label}: unexpected attribute ${a[1]} in ${all}`);
      if (a[1] === 'href') assert.doesNotMatch(a[2], /^\s*(javascript|data|vbscript):/i, `${label}: script URL in ${all}`);
    }
    return '';
  });
  assert.doesNotMatch(rest, /[<>]/, `${label}: stray angle bracket outside a known tag: ${rest.match(/.{0,30}[<>].{0,30}/)?.[0]}`);
}

const HOSTILE = [
  '## Say "hi" <script>x</script> & `<b>`',
  '[x](/a`" onmouseover="alert(1)`)',
  '[x](`javascript:alert(1)`)',
  '[<img src=x onerror=alert(1)>](https://a.b)',
  '[x](JaVaScRiPt:alert(1))',
  '[x](data:text/html;base64,AAA)',
  '[x](vbscript:foo)',
  '[x](&#106;avascript:alert(1))',
  '[x](javascript&#58;alert(1))',
  '[x](/a"onmouseover="x)',
  '[x](/a<script>)',
  '| <script>x</script> | b |\n|--|--|\n| [l](javascript:1) | `<img onerror=1>` |',
  '> <script>x</script>',
  '- <img src=x onerror=1>\n  - <svg onload=1>',
  '*<b>* _<i>_ **<u>**',
  '````md\n</code></pre><script>alert(1)</script>\n````',
  '```a"onmouseover="x\ncode\n```',
  '<!-- comment --> <iframe src=javascript:1></iframe>',
  '&lt;script&gt; &#60;script&#62;',
];

test('hostile markdown produces only known tags and attributes, and no script URL', () => {
  for (const src of HOSTILE) assertInert(md(src), JSON.stringify(src));
});

test('a policy snippet with a script tag in a fenced block stays inside the code element as text', () => {
  const policy = '{"@type":"Offer","rdfs:comment":"</code></pre><script>alert(1)</script>","uid":"x\\"><img src=x onerror=alert(1)>"}';
  const html = md('```json\n' + policy + '\n```\n');
  assertInert(html, 'policy');
  assert.equal((html.match(/<script/g) || []).length, 0);
  assert.equal((html.match(/<\/pre>/g) || []).length, 1);
  assert.match(html, /&lt;script&gt;alert\(1\)&lt;\/script&gt;/);
});

test('a link whose target contains inline code is not turned into an href holding markup', () => {
  const html = md('[x](/a`" onmouseover="alert(1)`)');
  assert.doesNotMatch(html, /href="[^"]*[<>]/);
  assert.ok(!html.includes('onmouseover=alert'));
});

test('a NUL character in the source cannot reach an inline code slot', () => {
  const html = md('\u00000\u0000 text `a`');
  assert.equal((html.match(/<code>/g) || []).length, 1);
});

test('a heading with quotes and markup escapes its anchor label', () => {
  const html = md('## Say "hi" <b>\n');
  assert.match(html, /aria-label="Link to Say &quot;hi&quot; &lt;b&gt;"/);
  assertInert(html, 'heading');
});

test('{{file}} escapes a hostile extension used as the language class', () => {
  const dir = tmpContent({ 'x.a"onmouseover="y': 'text\n' });
  const html = md('{{file x.a"onmouseover="y}}\n', { contentDir: dir });
  assert.ok(!html.includes('"onmouseover="'));
  assertInert(html, 'file ext');
});

// --- {{source}} and {{rendered}} -------------------------------------------------

const repoWith = (files) => tmpContent(files);

test('{{source path#region}} embeds the lines between the region markers, dedented, markers removed', () => {
  const repo = repoWith({
    'a/ex.rs': 'fn before() {}\n    // region: one\n    let a = 1;\n      let b = 2;\n    // endregion: one\nfn after() {}\n',
  });
  const html = md('{{source a/ex.rs#one}}\n', { repoDir: repo });
  assert.match(html, /<code class="language-rust">let a = 1;\n  let b = 2;\n<\/code>/);
  assert.ok(!html.includes('region'));
  assert.ok(!html.includes('before'));
});

test('{{source}} names its origin so a reader can find the compiled file', () => {
  const repo = repoWith({ 'a/ex.ts': '// region: r\nconst x = 1;\n// endregion: r\n' });
  const html = md('{{source a/ex.ts#r}}\n', { repoDir: repo });
  assert.match(html, /class="language-typescript"/);
  assert.match(html, /<p class="source-note">From <code>a\/ex\.ts<\/code>, region <code>r<\/code>\.<\/p>/);
});

test('{{source}} fails on a missing file, a missing region, an unclosed region, and a path outside the repo', () => {
  const repo = repoWith({ 'ex.rs': '// region: open\nx\n', 'ok.rs': '// region: r\nx\n// endregion: r\n' });
  assert.throws(() => md('{{source nope.rs#r}}', { repoDir: repo }), /nope\.rs/);
  assert.throws(() => md('{{source ok.rs#absent}}', { repoDir: repo }), /absent/);
  assert.throws(() => md('{{source ex.rs#open}}', { repoDir: repo }), /endregion/);
  assert.throws(() => md('{{source ../../../etc/passwd#r}}', { repoDir: repo }), /outside/i);
  assert.throws(() => md('{{source ok.rs}}', { repoDir: repo }), /#/);
});

test('{{source}} escapes what it embeds', () => {
  const repo = repoWith({ 'x.rs': '// region: r\nlet s = "<script>alert(1)</script>";\n// endregion: r\n' });
  const html = md('{{source x.rs#r}}\n', { repoDir: repo });
  assert.ok(!html.includes('<script>'));
});

test('{{rendered}} embeds a renderer output file in a labelled region, comments removed', () => {
  const dir = tmpContent({ 'examples/o.html': '<!--<[x::Y]>--><article class="ds-prose"><p>Hi</p></article><!--</[x::Y]>-->' });
  const html = md('{{rendered examples/o.html}}\n', { contentDir: dir });
  assert.match(html, /<div class="rendered" role="region" aria-label="Rendered output" tabindex="0"><article class="ds-prose"><p>Hi<\/p><\/article><\/div>/);
  assert.ok(!html.includes('<!--'));
});

test('{{rendered}} refuses a file that holds a script or an event handler, and a missing one', () => {
  const dir = tmpContent({ 'bad.html': '<p onclick="x()">a</p>', 'bad2.html': '<script>1</script>' });
  assert.throws(() => md('{{rendered bad.html}}', { contentDir: dir }), /not renderer output/i);
  assert.throws(() => md('{{rendered bad2.html}}', { contentDir: dir }), /not renderer output/i);
  assert.throws(() => md('{{rendered nope.html}}', { contentDir: dir }), /nope\.html/);
  assert.throws(() => md('{{rendered ../x.html}}', { contentDir: dir }), /outside/i);
});

test('{{rendered repo:path}} reads a file under the repository (a pinned snapshot)', () => {
  const repo = repoWith({ 's/snap.html': '<article class="ds-prose"><p>S</p></article>' });
  const html = md('{{rendered repo:s/snap.html}}\n', { repoDir: repo });
  assert.match(html, /<article class="ds-prose"><p>S<\/p><\/article>/);
});
