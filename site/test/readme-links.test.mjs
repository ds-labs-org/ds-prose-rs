// Links from the READMEs to the Pages site resolve in the built tree, the site
// map is in the main README, and the CI table matches the workflow.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import fs from 'node:fs';
import { buildSite } from './helpers.mjs';
import { REPO, source } from './contract.mjs';

const ROOT = 'https://ds-labs-org.github.io/ds-prose-rs/';
const site = buildSite({ base: '/ds-prose-rs/' });
const README = source('README.md');
const siteLinks = (md) => [...md.matchAll(/\]\((https:\/\/ds-labs-org\.github\.io\/ds-prose-rs\/[^)\s]*)\)/g)].map((m) => m[1]);
const exists = (url) => {
  const rel = new URL(url).pathname.slice('/ds-prose-rs/'.length);
  const file = path.join(site.out, rel, rel === '' || rel.endsWith('/') ? 'index.html' : '');
  return fs.existsSync(file) && fs.statSync(file).isFile();
};

test('the main README has a site map: home, both demos, both documentation sets', () => {
  const links = siteLinks(README);
  for (const p of ['', 'demo/', 'demo-angular/', 'yew/', 'angular/']) assert.ok(links.includes(ROOT + p), `README does not link ${ROOT + p}`);
});

test('the site map names the pages that exist: Interface, Styling, Validation and CSS hooks of both', () => {
  const links = siteLinks(README);
  for (const s of ['yew', 'angular']) for (const p of ['interface', 'styling', 'validation', 'css-hooks']) assert.ok(links.includes(`${ROOT}${s}/${p}/`), `${s}/${p}/`);
});

test('every link from a README to the site resolves to a page in the built tree', () => {
  for (const f of ['README.md', 'prose-angular/README.md', 'prose-wasm/npm/README.md', 'demo-angular/README.md']) {
    for (const url of siteLinks(source(f))) assert.ok(exists(url), `${f}: ${url} is not a page of the site`);
  }
});

test('the npm READMEs point to the documentation and the demo of their own side', () => {
  assert.ok(siteLinks(source('prose-angular/README.md')).includes(`${ROOT}angular/`));
  assert.ok(siteLinks(source('prose-angular/README.md')).includes(`${ROOT}demo-angular/`));
  assert.ok(siteLinks(source('prose-wasm/npm/README.md')).includes(`${ROOT}angular/`));
});

test('no README calls the site root the demo', () => {
  for (const f of ['README.md', 'prose-angular/README.md', 'prose-wasm/npm/README.md', 'demo-angular/README.md']) {
    assert.doesNotMatch(source(f), /\[[^\]]*demo[^\]]*\]\(https:\/\/ds-labs-org\.github\.io\/ds-prose-rs\/\)/i, f);
  }
});

test('the CI table has a row for every job of ci.yml, and says where the demos are booted', () => {
  const ci = source('.github/workflows/ci.yml');
  const jobsText = ci.slice(ci.indexOf('\njobs:\n') + 7);
  const jobs = [...jobsText.matchAll(/^  ([\w-]+):\s*$/gm)].map((m) => m[1]);
  assert.ok(jobs.includes('angular') && jobs.includes('demo'));
  const table = README.slice(README.indexOf('## CI'), README.indexOf('## npm packages'));
  for (const j of jobs) assert.match(table, new RegExp(`^\\| \`${j}\` \\|`, 'm'), `no row for ${j}`);
  assert.match(table, /^\| `demo` \|.*\/demo\//m);
  assert.match(table, /^\| `angular` \|.*\/demo-angular\/.*layout/m);
});

test('the README says Pages deploys the site, not only the demo', () => {
  const ci = README.slice(README.indexOf('## CI'), README.indexOf('## npm packages'));
  assert.doesNotMatch(ci, /pages\.yml` deploys the demo/);
  assert.match(ci, /pages\.yml` deploys the site/);
});

test('the Angular demo README describes this demo and how it is built for Pages', () => {
  const md = source('demo-angular/README.md');
  assert.match(md, /^# ds-prose-rs Angular demo/m);
  assert.doesNotMatch(md, /generated using \[Angular CLI\]/);
  assert.match(md, /--base-href \/ds-prose-rs\/demo-angular\//);
  assert.match(md, /site\/content\/examples\/prose\.css/);
  assert.match(md, /minify/);
});

test('the development section tells how to build the whole site locally', () => {
  const dev = README.slice(README.indexOf('## Develop'));
  assert.match(dev, /node --test "site\/test\/\*\.test\.mjs"/);
  assert.match(dev, /SITE_BASE=\/ds-prose-rs\/ node site\/build\.mjs/);
  assert.match(dev, /layout-check\.mjs/);
});

test('the site README tells that the demos get the navigation and the switch from the stamp', () => {
  const md = source('site/README.md');
  assert.match(md, /switch/i);
  assert.match(md, /layout-check\.mjs <dist> <base>|layout-check\.mjs/);
  assert.match(md, /LAYOUT_REQUIRE_CHROME/);
});
