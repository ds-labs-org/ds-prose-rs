// The project name is an acronym, and the site says what it stands for.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { buildSite, htmlFiles, read } from './helpers.mjs';

const EXPANSION = 'Prose Rendering for ODRL Semantic Representation';

test('the home page and the footer of every page spell out PROSE', () => {
  const site = buildSite();
  const pages = htmlFiles(site.out).filter((f) => !f.includes('/demo'));
  assert.ok(pages.length > 5);
  for (const f of pages) {
    assert.ok(read(f).includes(`PROSE: ${EXPANSION}`), `${f} does not spell out PROSE`);
  }
});
