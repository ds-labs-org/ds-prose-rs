// The home page shows a real policy and the sentences the renderer wrote for
// it. The Rust test prose-yew/tests/site_examples.rs pins examples/offer.html
// to the live renderer; this file pins the page to those example files.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { buildSite, read } from './helpers.mjs';
import { content, source } from './contract.mjs';

const site = buildSite();
const home = read(path.join(site.out, 'index.html'));
const unescape = (s) => s.replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&amp;/g, '&');

test('the policy on the page is the repository fixture, byte for byte', () => {
  const fixture = source('prose-core/tests/fixtures/offer.json');
  assert.equal(content('examples/offer.jsonld'), fixture);
  const blocks = [...home.matchAll(/<code class="language-jsonld">([\s\S]*?)<\/code>/g)].map((m) => unescape(m[1]));
  assert.deepEqual(blocks, [fixture]);
});

test('the rendered block is the stored renderer output, and every sentence in it is on the page as text', () => {
  const stored = content('examples/offer.html').trim();
  assert.ok(home.includes(`<div class="rendered" role="region" aria-label="Rendered output" tabindex="0">${stored}</div>`));
  // A sentence holds the IRIs it names as span.prose-iri (#45).
  const sentences = [...stored.matchAll(/<p class="prose-rule-sentence">(.*?)<\/p>/g)].map((m) => unescape(m[1]));
  assert.equal(sentences.length, 2);
  for (const s of sentences) assert.ok(home.includes(s), s);
});

test('the sentences are not typed into the markdown: the page source holds none of them', () => {
  const md = content('index.md');
  for (const s of ['may display', 'must attribute', 'are before the start of 2026-12-31']) assert.ok(!md.includes(s), `index.md hand-types "${s}"`);
});

test('the page loads the example stylesheet and the build ships it', () => {
  assert.match(home, /<link rel="stylesheet" href="\/ds-prose-rs\/examples\/prose\.css">/);
  assert.equal(read(path.join(site.out, 'examples/prose.css')), content('examples/prose.css'));
});

test('the page links the two demos and the two documentation sets, and shows both install lines', () => {
  const main = home.slice(home.indexOf('<main'));
  for (const href of ['/ds-prose-rs/demo/', '/ds-prose-rs/demo-angular/', '/ds-prose-rs/yew/', '/ds-prose-rs/angular/']) assert.ok(main.includes(`href="${href}"`), href);
  assert.match(main, /cargo add prose-yew/);
  assert.match(main, /npm install @ds-labs\/prose-angular/);
});

test('the claims about what the renderer does not do are the README\'s: no editor in npm, read-only components emit no CSS', () => {
  const md = content('index.md');
  assert.match(md, /ships no CSS|emits no CSS|no stylesheet/i);
});

test('the versions in the table are the packages\' versions', () => {
  const md = content('index.md');
  const crate = /^version\s*=\s*"([^"]+)"/m.exec(source('Cargo.toml'))[1];
  const ng = JSON.parse(source('prose-angular/package.json')).version;
  const wasm = JSON.parse(source('prose-wasm/npm/package.json')).version;
  for (const [name, version] of [['`prose-yew` (crate)', crate], ['`@ds-labs/prose-wasm` (npm)', wasm], ['`@ds-labs/prose-angular` (npm)', ng]])
    assert.ok(md.includes(`| ${name} | ${version} |`), `${name} ${version}`);
});
