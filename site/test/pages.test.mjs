import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { buildSite, htmlFiles, read, urlOf } from './helpers.mjs';

const site = buildSite();
const isDemo = (u) => /\/demo(-angular)?\/$/.test(u);
const pages = htmlFiles(site.out)
  .map((f) => ({ f, url: urlOf(site.out, site.base, f), html: read(f) }))
  .filter((p) => !isDemo(p.url));

test('the build produces the home page, section pages and 404.html', () => {
  const urls = pages.map((p) => p.url);
  for (const u of ['/ds-prose-rs/', '/ds-prose-rs/yew/', '/ds-prose-rs/angular/', '/ds-prose-rs/yew/interface/', '/ds-prose-rs/404.html'])
    assert.ok(urls.includes(u), `missing ${u} in ${urls.join(', ')}`);
});

test('the build writes .nojekyll and copies the assets', () => {
  for (const f of ['.nojekyll', 'assets/site.css', 'assets/site.js']) assert.ok(read(path.join(site.out, f)) !== undefined, f);
});

for (const p of pages) {
  test(`page contract: ${p.url}`, () => {
    const h = p.html;
    assert.match(h, /^<!doctype html>/i, 'doctype first');
    assert.match(h, /<html lang="en"[ >]/);
    assert.match(h, /<meta charset="utf-8">/);
    assert.match(h, /<meta name="viewport" content="width=device-width, initial-scale=1">/);
    assert.match(h, /<title>[^<]+ - ds-prose-rs<\/title>/);
    assert.match(h, /<meta name="description" content="[^"]{10,}">/);
    assert.equal((h.match(/<h1[ >]/g) || []).length, 1, 'exactly one h1');
    assert.equal((h.match(/<main id="main"/g) || []).length, 1, 'exactly one main#main');
    assert.equal((h.match(/<header[ >]/g) || []).length, 1);
    assert.equal((h.match(/<footer[ >]/g) || []).length, 1);
  });

  test(`skip link is the first focusable element: ${p.url}`, () => {
    const body = p.html.slice(p.html.indexOf('<body'));
    const first = /<(a|button|input|select|textarea|summary)\b[^>]*>/.exec(body);
    assert.ok(first, 'no focusable element');
    assert.match(first[0], /^<a class="skip-link" href="#main">/);
  });

  test(`landmarks are labelled and the theme script runs before the stylesheet: ${p.url}`, () => {
    assert.match(p.html, /<nav class="topnav" aria-label="Primary">/);
    const init = p.html.indexOf('data-theme');
    const css = p.html.indexOf('assets/site.css');
    assert.ok(init > 0 && css > 0 && init < css || p.html.indexOf('<script>') < css, 'theme init precedes the stylesheet');
    assert.ok(p.html.indexOf('<script>') < css, 'inline theme script precedes the stylesheet link');
  });

  test(`no inline event handlers or style attributes: ${p.url}`, () => {
    assert.doesNotMatch(p.html, /\son[a-z]+="/);
    assert.doesNotMatch(p.html, /\sstyle="/);
  });
}

test('headings do not skip levels inside a page', () => {
  for (const p of pages) {
    const levels = [...p.html.matchAll(/<h([1-6])[ >]/g)].map((m) => Number(m[1]));
    for (let i = 1; i < levels.length; i++) assert.ok(levels[i] - levels[i - 1] <= 1, `${p.url}: h${levels[i - 1]} then h${levels[i]}`);
  }
});
