// Headless Chrome layout check, run as part of `npm test`. Skipped, with a
// message, when no Chrome is found (set CHROME to point at one).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { buildSite } from './helpers.mjs';
import { findChrome, runLayoutCheck, VIEWPORTS, THEMES } from './layout-check.mjs';

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
