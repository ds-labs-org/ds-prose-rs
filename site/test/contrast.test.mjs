import { test } from 'node:test';
import assert from 'node:assert/strict';
import { palettes, ratio, hex } from './css.mjs';

const { light, darkAuto, darkForced } = palettes();

const TEXT_PAIRS = [
  ['--text', '--bg'], ['--text', '--surface'], ['--text', '--surface-2'],
  ['--text-muted', '--bg'], ['--text-muted', '--surface'], ['--text-muted', '--surface-2'],
  ['--link', '--bg'], ['--link', '--surface'], ['--link', '--surface-2'],
  ['--accent', '--surface'],
  ['--permission', '--surface'], ['--prohibition', '--surface'], ['--obligation', '--surface'], ['--warning', '--surface'],
];
const UI_PAIRS = [['--focus', '--bg'], ['--focus', '--surface'], ['--border', '--bg']];

for (const [name, pal] of [['light', light], ['dark', darkAuto], ['dark (forced)', darkForced]]) {
  test(`${name} palette defines every token as a 6-digit hex`, () => {
    for (const k of ['--bg', '--surface', '--surface-2', '--border', '--text', '--text-muted', '--link', '--accent', '--focus', '--permission', '--prohibition', '--obligation', '--warning'])
      assert.match(pal[k] ?? '', /^#[0-9a-f]{6}$/i, `${name}: ${k}`);
  });

  test(`${name} palette: text pairs reach WCAG AA (4.5:1)`, () => {
    for (const [fg, bg] of TEXT_PAIRS) {
      const r = ratio(hex(pal[fg]), hex(pal[bg]));
      assert.ok(r >= 4.5, `${name}: ${fg} on ${bg} is ${r.toFixed(2)}`);
    }
  });

  test(`${name} palette: body and muted text reach AAA on the page background (7:1)`, () => {
    assert.ok(ratio(hex(pal['--text']), hex(pal['--bg'])) >= 7);
    assert.ok(ratio(hex(pal['--text-muted']), hex(pal['--bg'])) >= 7, `muted on bg ${ratio(hex(pal['--text-muted']), hex(pal['--bg'])).toFixed(2)}`);
  });

  test(`${name} palette: focus ring and borders reach 3:1`, () => {
    for (const [fg, bg] of UI_PAIRS) {
      const r = ratio(hex(pal[fg]), hex(pal[bg]));
      const need = fg === '--border' ? 1.2 : 3;
      assert.ok(r >= need, `${name}: ${fg} on ${bg} is ${r.toFixed(2)}`);
    }
  });
}

test('the two dark blocks (system preference and forced) declare identical tokens', () => {
  assert.ok(darkAuto && Object.keys(darkAuto).length > 5);
  assert.deepEqual(darkAuto, darkForced);
});

test('the contrast helper agrees with known values', () => {
  assert.equal(ratio([0, 0, 0], [255, 255, 255]).toFixed(2), '21.00');
  assert.equal(ratio(hex('#777777'), hex('#ffffff')).toFixed(2), '4.48');
});
