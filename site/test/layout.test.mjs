// Headless Chrome layout check, run as part of `npm test`. Skipped, with a
// message, when no Chrome is found (set CHROME to point at one).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { buildSite, SITE_DIR } from './helpers.mjs';
import { findChrome, runLayoutCheck, exampleCssFailures, VIEWPORTS, THEMES } from './layout-check.mjs';

test('findChrome honours the CHROME variable first', () => {
  assert.equal(findChrome({ CHROME: '/opt/x/chrome' }, () => false), '/opt/x/chrome');
  assert.equal(findChrome({}, (c) => c === 'chromium'), 'chromium');
  assert.equal(findChrome({}, () => false), null);
});

test('the check covers 1280 and 400 px in light and dark', () => {
  assert.deepEqual(VIEWPORTS.map((v) => v.width), [1280, 400]);
  assert.deepEqual(THEMES, ['light', 'dark']);
});

test('built site: no overflow, nav present, AA contrast, menu, theme toggle', { timeout: 180_000 }, async (t) => {
  const chrome = findChrome();
  if (!chrome) {
    t.skip('no Chrome or Chromium found: layout check not run (set CHROME=/path/to/chrome)');
    return;
  }
  const site = buildSite({ kitchenSink: true });
  const { failures, checked } = await runLayoutCheck({ dist: site.out, base: site.base, chrome, shotDir: process.env.SHOT_DIR });
  assert.ok(checked >= 8 * VIEWPORTS.length * THEMES.length, `only ${checked} page views checked`);
  assert.deepEqual(failures, []);
});

const CHECK = path.join(SITE_DIR, 'test/layout-check.mjs');
const noBrowser = (extra = {}) => spawnSync(process.execPath, [CHECK], { encoding: 'utf8', env: { PATH: '/nonexistent', ...extra } });

test('with no browser the check is skipped, and fails when LAYOUT_REQUIRE_CHROME=1', () => {
  const skipped = noBrowser();
  assert.equal(skipped.status, 0);
  assert.match(skipped.stdout, /skipped: no Chrome/);
  const required = noBrowser({ LAYOUT_REQUIRE_CHROME: '1' });
  assert.equal(required.status, 2);
  assert.match(required.stderr, /FAILED: no Chrome/);
});

function tree(files) {
  const dist = fs.mkdtempSync(path.join(os.tmpdir(), 'css-tree-'));
  for (const [rel, text] of Object.entries(files)) {
    fs.mkdirSync(path.dirname(path.join(dist, rel)), { recursive: true });
    fs.writeFileSync(path.join(dist, rel), text);
  }
  return dist;
}
const EX = (f) => fs.readFileSync(path.join(SITE_DIR, 'content/examples', f), 'utf8');
const reprint = (css) => css.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\[([\w-]+)="([^"]*)"\]/g, '[$1=$2]');

test('exampleCssFailures: the demos hold the example sheets, as copied or re-printed by a bundler', () => {
  const copied = tree({ 'demo/prose-1.css': EX('prose.css'), 'demo/prose-editor-2.css': EX('prose-editor.css'), 'demo-angular/example-3.css': reprint(EX('prose.css')) });
  assert.deepEqual(exampleCssFailures(copied), []);
});

test('exampleCssFailures: says which demo lacks which sheet, or changed a rule, or has no demo', () => {
  const changed = EX('prose.css').replace('border-left: 4px solid var(--ds-border)', 'border-left: 5px solid var(--ds-border)');
  const f = exampleCssFailures(tree({ 'demo/prose-1.css': changed, 'demo/prose-editor-2.css': EX('prose-editor.css'), 'demo-angular/styles.css': 'body{margin:0}' }));
  assert.equal(f.length, 2);
  assert.match(f[0], /demo\/ ships no stylesheet holding examples\/prose\.css/);
  assert.match(f[1], /demo-angular\/ ships no stylesheet holding examples\/prose\.css/);
  assert.match(exampleCssFailures(tree({ 'demo/prose.css': EX('prose.css') })).join('\n'), /demo-angular\/ is not in the tree/);
});

test('exampleCssFailures: the Angular demo does not carry the editor sheet', () => {
  const f = exampleCssFailures(tree({ 'demo/a.css': EX('prose.css') + EX('prose-editor.css'), 'demo-angular/a.css': EX('prose.css') + EX('prose-editor.css') }));
  assert.ok(f.some((m) => /demo-angular\/ carries the editor sheet/.test(m)), f.join('\n'));
});
