// Shared fixtures: build the real site sources into a temp directory, with
// minimal stand-ins for the two demo dists (the real ones need Trunk and ng).
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from '../build.mjs';

export const SITE_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export const BASE = '/ds-prose-rs/';

export const DEMO_HTML = (title) =>
  `<!doctype html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n<title>${title}</title>\n` +
  `<meta name="viewport" content="width=device-width, initial-scale=1">\n<!--ds-topnav-css-->\n</head>\n` +
  `<body>\n<!--ds-topnav-->\n<div id="app"><p class="prose-policy">fixture</p></div>\n</body>\n</html>\n`;

export function fixtureDemos(root) {
  const demo = path.join(root, 'demo-src');
  const ng = path.join(root, 'ng-src');
  fs.mkdirSync(demo, { recursive: true });
  fs.mkdirSync(ng, { recursive: true });
  fs.writeFileSync(path.join(demo, 'index.html'), DEMO_HTML('ds-prose-rs demo'));
  fs.writeFileSync(path.join(demo, 'app.js'), '// fixture\n');
  fs.writeFileSync(path.join(ng, 'index.html'), DEMO_HTML('ds-prose-rs Angular demo'));
  return { demo, 'demo-angular': ng };
}

// A page with every block type at its awkward size: long unbroken tokens, a
// wide table, a long code line. Layout checks build it in; the site does not.
export const KITCHEN_SINK = fs.readFileSync(path.join(SITE_DIR, 'test/kitchen-sink.md'), 'utf8');

export function buildSite({ base = BASE, kitchenSink = false } = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'site-'));
  const out = path.join(root, 'dist');
  const opts = { outDir: out, base, demos: fixtureDemos(root) };
  if (kitchenSink) {
    opts.contentDir = path.join(root, 'content');
    fs.cpSync(path.join(SITE_DIR, 'content'), opts.contentDir, { recursive: true });
    fs.writeFileSync(path.join(opts.contentDir, 'kitchen-sink.md'), KITCHEN_SINK);
  }
  build(opts);
  return { root, out, base };
}

export function walk(dir, pred = () => true) {
  const res = [];
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) res.push(...walk(p, pred));
    else if (pred(p)) res.push(p);
  }
  return res;
}

export const htmlFiles = (out) => walk(out, (p) => p.endsWith('.html'));
export const urlOf = (out, base, file) => base + path.relative(out, file).replace(/index\.html$/, '').split(path.sep).join('/');
export const read = (f) => fs.readFileSync(f, 'utf8');

export function topnav(html) {
  const m = /<nav class="topnav"[\s\S]*?<\/nav>/.exec(html);
  return m ? m[0] : null;
}
