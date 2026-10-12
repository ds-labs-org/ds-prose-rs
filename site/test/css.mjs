// Tiny CSS helpers for the tests: token extraction and WCAG contrast.
import fs from 'node:fs';
import path from 'node:path';
import { SITE_DIR } from './helpers.mjs';

export const CSS = fs.readFileSync(path.join(SITE_DIR, 'assets/site.css'), 'utf8');

// Body of the first block whose prelude contains `selector`, braces matched.
export function blockAfter(css, selector, from = 0) {
  const at = css.indexOf(selector, from);
  if (at < 0) return null;
  const open = css.indexOf('{', at);
  let depth = 0;
  for (let i = open; i < css.length; i++) {
    if (css[i] === '{') depth++;
    else if (css[i] === '}' && --depth === 0) return { body: css.slice(open + 1, i), end: i };
  }
  return null;
}

export function decls(body) {
  const out = {};
  for (const m of body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) out[m[1]] = m[2].trim();
  return out;
}

export function palettes(css = CSS) {
  const light = decls(blockAfter(css, ':root {').body);
  const media = css.indexOf('@media (prefers-color-scheme: dark)');
  const darkAuto = media < 0 ? null : decls(blockAfter(css, ':root:not([data-theme="light"])', media)?.body ?? '');
  const darkForced = decls(blockAfter(css, ':root[data-theme="dark"]')?.body ?? '');
  return { light, darkAuto, darkForced };
}

const lin = (c) => ((c /= 255) <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
export function luminance([r, g, b]) {
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}
export function ratio(a, b) {
  const [l1, l2] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (l1 + 0.05) / (l2 + 0.05);
}
export function hex(h) {
  const m = /^#([0-9a-f]{6})$/i.exec(h);
  if (!m) throw new Error(`not a 6-digit hex colour: ${h}`);
  return [0, 2, 4].map((i) => parseInt(m[1].slice(i, i + 2), 16));
}
