import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { buildSite, htmlFiles, read, urlOf } from './helpers.mjs';

const site = buildSite();
const B = site.base;
const pages = htmlFiles(site.out).map((f) => ({ url: urlOf(site.out, B, f), html: read(f) }));

function fileFor(url) {
  const rel = decodeURI(url.slice(B.length));
  const p = path.join(site.out, rel);
  if (rel === '' || rel.endsWith('/')) return path.join(p, 'index.html');
  return fs.existsSync(p) && fs.statSync(p).isDirectory() ? path.join(p, 'index.html') : p;
}
const idsOf = (html) => [...html.matchAll(/\sid="([^"]+)"/g)].map((m) => m[1]);
const refs = (html) => [...html.matchAll(/\s(?:href|src)="([^"]*)"/g)].map((m) => m[1]);

test('every internal href and src resolves to a file under the base path', () => {
  for (const p of pages) {
    for (const r of refs(p.html)) {
      if (/^(https?:|mailto:)/.test(r) || r.startsWith('#')) continue;
      const abs = new URL(r, 'http://x' + p.url);
      assert.ok(abs.pathname.startsWith(B), `${p.url}: ${r} leaves the base ${B}`);
      assert.ok(fs.existsSync(fileFor(abs.pathname)), `${p.url}: ${r} does not resolve`);
    }
  }
});

test('every fragment exists as an id in its target page', () => {
  for (const p of pages) {
    for (const r of refs(p.html)) {
      if (/^(https?:|mailto:)/.test(r) || !r.includes('#')) continue;
      const abs = new URL(r, 'http://x' + p.url);
      if (!abs.hash || abs.hash === '#') continue;
      const target = fileFor(abs.pathname);
      assert.ok(fs.existsSync(target), `${p.url}: ${r}`);
      assert.ok(idsOf(read(target)).includes(abs.hash.slice(1)), `${p.url}: ${r} has no matching id`);
    }
  }
});

test('ids are unique within a page', () => {
  for (const p of pages) {
    const ids = idsOf(p.html);
    assert.equal(new Set(ids).size, ids.length, `${p.url} repeats an id: ${ids.filter((x, i) => ids.indexOf(x) !== i)}`);
  }
});

test('external links are https and opened without a new window', () => {
  let seen = 0;
  for (const p of pages) {
    for (const m of p.html.matchAll(/<a\s[^>]*href="(https?:[^"]+)"[^>]*>/g)) {
      seen++;
      assert.match(m[1], /^https:/, `${p.url}: ${m[1]}`);
      assert.doesNotMatch(m[0], /target=/);
    }
  }
  assert.ok(seen > 0);
});
