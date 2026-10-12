// The Pages and CI workflows against the site design: the demos build under
// their sub-paths, the stamped site is boot-checked and layout-checked, and the
// action pins do not move. Workflows are read as text; no YAML parser.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { REPO, source } from './contract.mjs';
import { chrome } from './chrome.mjs';

const PAGES = source('.github/workflows/pages.yml');
const CI = source('.github/workflows/ci.yml');

// The pins that were in the workflows before the site was added.
const PINNED = [
  'actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0',
  'dtolnay/rust-toolchain@f50ab1ce44b2f120b2531407517fcbfb1d610fa5 # 1.96.1',
  'taiki-e/install-action@e407f7bafb71fd004bc5c2da3032e5470cbb6ef0 # v2.87.24',
  'actions/configure-pages@983d7736d9b0ae728b81ab479565c72886d7745b # v5',
  'actions/upload-pages-artifact@56afc609e74202658d3ffba0e8f6dda462b719fa # v3',
  'actions/deploy-pages@d6db90164ac5ed86f2b6aed7e0febac5b3c0c03e # v4',
];
// The one action Pages needs and did not have, pinned like the others.
const SETUP_NODE = 'actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020 # v4.4.0';

const uses = (text) => [...text.matchAll(/^\s*-?\s*uses:\s*(.+)$/gm)].map((m) => m[1].trim());

function jobs(text) {
  const body = text.slice(text.indexOf('\njobs:\n') + 7);
  const parts = body.split(/^  ([\w-]+):\s*$/m);
  const out = {};
  for (let i = 1; i < parts.length; i += 2) out[parts[i]] = parts[i + 1];
  return out;
}
const order = (text, needles) => {
  const at = needles.map((n) => {
    const i = typeof n === 'string' ? text.indexOf(n) : text.search(n);
    assert.ok(i >= 0, `missing: ${n}`);
    return i;
  });
  for (let i = 1; i < at.length; i++) assert.ok(at[i] > at[i - 1], `${needles[i]} must come after ${needles[i - 1]}`);
};

// ---- pages.yml ------------------------------------------------------------------

test('pages: every action is pinned to a commit; the old pins are unchanged', () => {
  const all = uses(PAGES);
  for (const u of all) assert.match(u, /@[0-9a-f]{40}( #.*)?$/, `not pinned: ${u}`);
  for (const p of PINNED) assert.ok(all.includes(p), `pin changed or gone: ${p}`);
  const extra = all.filter((u) => !PINNED.includes(u));
  assert.deepEqual(extra, [SETUP_NODE]);
});

test('pages: the trigger covers the site, the Angular packages and both demos, and still the Rust and the demo', () => {
  const paths = [...(/paths:\n((?:\s+- .+\n)+)/.exec(PAGES)?.[1] ?? '').matchAll(/- "([^"]+)"/g)].map((m) => m[1]);
  for (const p of ['demo/**', 'prose-core/**', 'prose-yew/**', 'Cargo.toml', 'Cargo.lock', 'site/**', 'prose-angular/**', 'prose-wasm/**', 'demo-angular/**', 'scripts/**', '.github/workflows/pages.yml']) {
    assert.ok(paths.includes(p), `paths lacks ${p}: ${paths}`);
  }
});

test('pages: installs Trunk and wasm-pack, and Node 22', () => {
  assert.match(PAGES, /tool: trunk@0\.22\.0-beta\.2,wasm-pack/);
  assert.match(PAGES, /node-version: 22/);
  assert.ok(PAGES.includes(`uses: ${SETUP_NODE}`));
});

test('pages: the steps run in order, test, build, assemble, boot check, upload', () => {
  order(PAGES, [
    'cargo test --workspace --locked',
    'node --test "site/test/*.test.mjs"',
    'trunk build --release --public-url /ds-prose-rs/demo/',
    'scripts/build-npm-packages.sh',
    'npm ci --no-audit --no-fund',
    '--no-save ../dist-npm/tarballs/*.tgz',
    'npx ng build --base-href /ds-prose-rs/demo-angular/',
    'node site/build.mjs',
    'scripts/smoke-demo.sh site/dist/demo /ds-prose-rs/demo/',
    'scripts/smoke-demo.sh site/dist/demo-angular /ds-prose-rs/demo-angular/',
    'node site/test/layout-check.mjs site/dist /ds-prose-rs/',
    'path: site/dist',
  ]);
});

test('pages: the site is built for the project-page base, and the Angular demo for its sub-path', () => {
  assert.match(PAGES, /SITE_BASE:\s*\/ds-prose-rs\/\s*\n\s*run: node site\/build\.mjs/);
  assert.doesNotMatch(PAGES, /trunk build --release --public-url \/ds-prose-rs\/\s*$/m);
  assert.doesNotMatch(PAGES, /path: demo\/dist/);
});

test('pages: the Angular boot check names its title and checks the stamped navigation and the validation controls', () => {
  const step = /smoke-demo\.sh site\/dist\/demo-angular[\s\S]*?(?=\n\s*- |\n\s*\n)/.exec(PAGES)?.[0] ?? '';
  assert.match(step, /SMOKE_TITLE: ds-prose-rs Angular demo/);
  assert.match(step, /SMOKE_EXTRA: .*class="topnav".*id="strict"|SMOKE_EXTRA: .*id="strict".*class="topnav"/);
});

test('pages: the Yew boot check keeps its own needles and adds the stamped navigation', () => {
  const step = /smoke-demo\.sh site\/dist\/demo \/ds-prose-rs\/demo\/[\s\S]*?(?=\n\s*- |\n\s*\n)/.exec(PAGES)?.[0] ?? '';
  assert.match(step, /SMOKE_EXTRA: .*id="profile-json".*id="profile-strict".*class="topnav"/);
});

test('pages: the layout check demands a browser rather than skipping', () => {
  const step = /layout-check\.mjs site\/dist \/ds-prose-rs\/[\s\S]*?(?=\n\s*- |\n\s*\n)/.exec(PAGES)?.[0] ?? '';
  assert.match(step, /LAYOUT_REQUIRE_CHROME:\s*"?1"?/);
});

// ---- ci.yml ---------------------------------------------------------------------

test('ci: no action is added; the existing ones keep their refs', () => {
  const allowed = new Set([
    ...PINNED,
    'actions/checkout@v4',
    'dtolnay/rust-toolchain@1.96.1',
    'Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2.9.2',
    'Swatinem/rust-cache@v2',
    'taiki-e/install-action@v2',
    'actions/setup-node@v4',
  ]);
  for (const u of uses(CI)) assert.ok(allowed.has(u), `unexpected action ref in ci.yml: ${u}`);
});

test('ci: the demo job builds under /demo/ and boots it there', () => {
  const demo = jobs(CI).demo;
  assert.ok(demo.includes('trunk build --release --public-url /ds-prose-rs/demo/'));
  assert.ok(demo.includes('scripts/smoke-demo.sh demo/dist /ds-prose-rs/demo/'));
  assert.doesNotMatch(demo, /--public-url \/ds-prose-rs\/\s*$/m);
});

test('ci: the angular job builds under /demo-angular/, boots it there, and runs the site tests and layout check on the real tree', () => {
  const ng = jobs(CI).angular;
  assert.ok(ng.includes('npx ng build --base-href /ds-prose-rs/demo-angular/'));
  assert.ok(ng.includes('scripts/smoke-demo.sh demo-angular/dist/demo-angular/browser /ds-prose-rs/demo-angular/'));
  assert.match(ng, /tool: trunk@0\.22\.0-beta\.2/);
  order(ng, [
    'scripts/build-npm-packages.sh',
    'npx ng build --base-href /ds-prose-rs/demo-angular/',
    'trunk build --release --public-url /ds-prose-rs/demo/',
    'node --test --test-reporter=spec --test-timeout=240000 "site/test/*.test.mjs"',
    'node site/build.mjs',
    'node site/test/layout-check.mjs site/dist /ds-prose-rs/',
  ]);
  assert.match(ng, /SITE_BASE:\s*\/ds-prose-rs\//);
});

test('ci: the layout check demands a browser in CI', () => {
  assert.match(jobs(CI).angular, /LAYOUT_REQUIRE_CHROME:\s*"?1"?/);
});

// ---- scripts --------------------------------------------------------------------

test('smoke-demo.sh keeps its defaults: demo/dist under /ds-prose-rs/', () => {
  const sh = source('scripts/smoke-demo.sh');
  assert.match(sh, /dist="\$\{1:-demo\/dist\}"/);
  assert.match(sh, /base="\$\{2:-\/ds-prose-rs\/\}"/);
});

function smoke(args, env, html) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'smoke-'));
  fs.writeFileSync(path.join(dir, 'index.html'), html);
  const r = spawnSync(path.join(REPO, 'scripts/smoke-demo.sh'), [dir, ...args], {
    encoding: 'utf8',
    env: { ...process.env, SMOKE_PORT: String(18000 + Math.floor(Math.random() * 1000)), CHROME: chrome, ...env },
    timeout: 90_000,
  });
  fs.rmSync(dir, { recursive: true, force: true });
  return r;
}
const PAGE = (title) => `<!DOCTYPE html><html><head><title>${title}</title></head><body><nav class="topnav"></nav><p class="prose-policy">x</p></body></html>`;

test('smoke-demo.sh boots a stamped page under a sub-path and under the old root path', { skip: !chrome && 'no Chrome or Chromium (set CHROME)', timeout: 200_000 }, () => {
  for (const base of ['/ds-prose-rs/demo/', '/ds-prose-rs/']) {
    const ok = smoke([base], { SMOKE_TITLE: 'my demo', SMOKE_EXTRA: 'class="topnav"' }, PAGE('my demo'));
    assert.equal(ok.status, 0, `${base}: ${ok.stderr}`);
    const bad = smoke([base], { SMOKE_TITLE: 'my demo', SMOKE_EXTRA: 'id="strict"' }, PAGE('my demo'));
    assert.equal(bad.status, 1, `${base} must fail on a missing needle`);
    assert.match(bad.stderr, /id="strict"/);
  }
});
