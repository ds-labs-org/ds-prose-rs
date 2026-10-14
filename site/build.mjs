// Static site build. Node 22+, no dependencies.
//
//   SITE_BASE=/ds-prose-rs/ node site/build.mjs
//
// Environment: SITE_BASE (default /), SITE_OUT (default site/dist),
// SITE_DEMO_DIST (default demo/dist), SITE_ANGULAR_DIST (default
// demo-angular/dist/demo-angular/browser), SITE_SKIP_DEMOS=1 to build without
// the demos (local work only; CI must not set it), SITE_REQUIRE_CATALOG=1 to
// fail when content/catalog/ was not generated (scripts/build-catalog.sh).
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseFrontMatter, renderMarkdown } from './lib/markdown.mjs';
import { normaliseBase, renderSubnav, stampDemo } from './lib/nav.mjs';
import { renderPage } from './lib/shell.mjs';

const SITE_DIR = path.dirname(fileURLToPath(import.meta.url));
const SECTIONS = new Set(['yew', 'angular', 'catalog']);

const CATALOG_FALLBACK = path.join('catalog', 'index.md');
const CATALOG_FALLBACK_MD = `---
title: Compliance corpus catalog
label: All cases
description: The catalog of the ds-odrl-compliance-rdf corpus is generated when the site is built; this build has none.
nav: catalog
order: 0
---
# Compliance corpus catalog

This build of the site has no catalog. The catalog is generated from [ds-odrl-compliance-rdf](https://github.com/ds-labs-org/ds-odrl-compliance-rdf) when the site is built. Run \`scripts/build-catalog.sh\` before \`node site/build.mjs\` to generate it.
`;

function walk(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = path.join(dir, e.name);
    return e.isDirectory() ? walk(p) : [p];
  });
}

function urlFor(base, rel) {
  const noExt = rel.replace(/\.md$/, '').split(path.sep).join('/');
  if (noExt === 'index') return base;
  if (noExt.endsWith('/index')) return base + noExt.slice(0, -'index'.length);
  return base + noExt + '/';
}

function outFor(outDir, base, url) {
  const rel = url.slice(base.length);
  return path.join(outDir, rel, 'index.html');
}

// Root-absolute links in content ("/yew/") are written without the deploy
// prefix; add it here so the same source builds for any base.
const prefixLinks = (html, base) => html.replace(/(<a href=")\/(?!\/)/g, `$1${base}`);

export function build({
  contentDir = path.join(SITE_DIR, 'content'),
  assetsDir = path.join(SITE_DIR, 'assets'),
  outDir = path.join(SITE_DIR, 'dist'),
  repoDir = path.resolve(SITE_DIR, '..'),
  base = '/',
  demos = {},
  requireCatalog = false,
} = {}) {
  base = normaliseBase(base);
  fs.rmSync(outDir, { recursive: true, force: true });
  fs.mkdirSync(outDir, { recursive: true });

  const sources = walk(contentDir)
    .map((f) => path.relative(contentDir, f))
    .filter((r) => r.endsWith('.md') && !r.split(path.sep).some((s) => s.startsWith('_')));

  // The catalog is generated at build time from ds-odrl-compliance-rdf. A
  // build without it gets a page that says how to generate it.
  if (!sources.includes(path.join('catalog', 'index.md'))) {
    if (requireCatalog) throw new Error('content/catalog/ is not generated: run scripts/build-catalog.sh');
    sources.push(CATALOG_FALLBACK);
  }

  const pages = sources.map((rel) => {
    const src = rel === CATALOG_FALLBACK ? CATALOG_FALLBACK_MD : fs.readFileSync(path.join(contentDir, rel), 'utf8');
    const { data, body } = parseFrontMatter(src);
    for (const k of ['title', 'description', 'nav']) if (!data[k]) throw new Error(`${rel}: front matter needs ${k}`);
    const { html } = renderMarkdown(body, { contentDir, repoDir });
    return { rel, data, html: prefixLinks(html, base), url: urlFor(base, rel) };
  });

  for (const p of pages) {
    const section = SECTIONS.has(p.data.nav) ? p.data.nav : null;
    const isSectionIndex = section && p.url === `${base}${section}/`;
    let subnav = '';
    if (section) {
      const siblings = pages
        .filter((q) => q.data.nav === section)
        .sort((a, b) => Number(a.data.order ?? 99) - Number(b.data.order ?? 99))
        .map((q) => ({ url: q.url, label: q.data.label ?? q.data.title }));
      subnav = renderSubnav({ section, pages: siblings, currentUrl: p.url });
    }
    const html = renderPage({
      title: p.data.title,
      description: p.data.description,
      active: p.data.nav,
      exact: !section || isSectionIndex,
      base,
      body: p.html,
      subnav,
      stylesheet: p.data.stylesheet ?? '',
    });
    const file = outFor(outDir, base, p.url);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, html);
  }

  fs.writeFileSync(
    path.join(outDir, '404.html'),
    renderPage({
      title: 'Page not found',
      description: 'This page does not exist on the ds-prose-rs site.',
      active: null,
      base,
      body: `<h1>Page not found</h1>\n<p>Go to the <a href="${base}">home page</a>.</p>\n`,
    }),
  );
  fs.writeFileSync(path.join(outDir, '.nojekyll'), '');
  fs.cpSync(assetsDir, path.join(outDir, 'assets'), { recursive: true });
  const examples = path.join(contentDir, 'examples');
  // Renderer-output fragments (.html) are embedded in pages; they are not pages themselves.
  if (fs.existsSync(examples)) fs.cpSync(examples, path.join(outDir, 'examples'), { recursive: true, filter: (src) => !src.endsWith('.html') });

  for (const [name, src] of Object.entries(demos)) {
    const dest = path.join(outDir, name);
    fs.cpSync(src, dest, { recursive: true });
    const index = path.join(dest, 'index.html');
    fs.writeFileSync(index, stampDemo(fs.readFileSync(index, 'utf8'), { active: name, base }));
  }
  return { pages: pages.map((p) => p.url) };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const repo = path.resolve(SITE_DIR, '..');
  const demos = {};
  if (process.env.SITE_SKIP_DEMOS !== '1') {
    demos.demo = path.resolve(process.env.SITE_DEMO_DIST ?? path.join(repo, 'demo/dist'));
    demos['demo-angular'] = path.resolve(process.env.SITE_ANGULAR_DIST ?? path.join(repo, 'demo-angular/dist/demo-angular/browser'));
    for (const [n, d] of Object.entries(demos)) if (!fs.existsSync(d)) throw new Error(`demo "${n}" not built: ${d} (SITE_SKIP_DEMOS=1 to build without)`);
  }
  const out = path.resolve(process.env.SITE_OUT ?? path.join(SITE_DIR, 'dist'));
  const { pages } = build({ outDir: out, base: process.env.SITE_BASE ?? '/', demos, requireCatalog: process.env.SITE_REQUIRE_CATALOG === '1' });
  console.log(`built ${pages.length} pages into ${out}`);
}
