// The two demos inside the site: stamp markers in the sources, the shared
// example stylesheets, the switch between unstyled and themed, and the Angular
// demo's use of strict, profile and validationChange. Source-level checks run
// everywhere; the switch itself runs in headless Chrome when there is one.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { buildSite, read, SITE_DIR } from './helpers.mjs';
import { REPO, content, source } from './contract.mjs';
import * as navlib from '../lib/nav.mjs';
import { normaliseCss, selectorsOf } from './css.mjs';
import { chrome, inChrome, reporter } from './chrome.mjs';

const TRUNK = source('demo/index.html');
const NG = source('demo-angular/src/index.html');
const count = (html, needle) => html.split(needle).length - 1;
const head = (html) => /<head>([\s\S]*?)<\/head>/i.exec(html)?.[1] ?? '';
const body = (html) => /<body[^>]*>([\s\S]*)<\/body>/i.exec(html)?.[1] ?? '';

for (const [name, html] of [['demo/index.html', TRUNK], ['demo-angular/src/index.html', NG]]) {
  test(`${name}: each stamp marker appears once, the head marker in <head>, the nav marker first in <body>`, () => {
    assert.equal(count(html, '<!--ds-topnav-css-->'), 1);
    assert.equal(count(html, '<!--ds-topnav-->'), 1);
    assert.ok(head(html).includes('<!--ds-topnav-css-->'), 'head marker is not in <head>');
    assert.ok(body(html).trimStart().startsWith('<!--ds-topnav-->'), 'nav marker is not the first thing in <body>');
    assert.match(html, /^<!doctype html>/i);
  });
}

test('titles: the boot checks look for them', () => {
  assert.match(TRUNK, /<title>ds-prose-rs [^<]*demo<\/title>/);
  assert.match(NG, /<title>ds-prose-rs Angular demo<\/title>/);
});

test('the base elements stay: Trunk feeds its public URL in, Angular rewrites href with --base-href', () => {
  assert.match(TRUNK, /<base data-trunk-public-url\s*\/>/);
  assert.match(NG, /<base href="\/">/);
});

test('Trunk demo: the two example stylesheets come from site/content/examples, before the demo layout', () => {
  const links = [...TRUNK.matchAll(/<link data-trunk rel="css" href="([^"]+)"/g)].map((m) => m[1]);
  assert.deepEqual(links, ['../site/content/examples/prose.css', '../site/content/examples/prose-editor.css', 'styles.css']);
  for (const l of links) assert.ok(fs.existsSync(path.join(REPO, 'demo', l)), l);
});

test('Angular demo: the example stylesheet is its own styles entry, before src/styles.css', () => {
  const ng = JSON.parse(source('demo-angular/angular.json')).projects['demo-angular'].architect.build;
  const styles = ng.options.styles;
  assert.equal(styles.length, 2);
  assert.deepEqual(styles[0], { input: '../site/content/examples/prose.css', bundleName: 'example' });
  assert.equal(styles[1], 'src/styles.css');
  assert.ok(fs.existsSync(path.join(REPO, 'demo-angular', styles[0].input)));
});

test('Angular demo: the production build keeps the example stylesheet readable and in its own file', () => {
  const prod = JSON.parse(source('demo-angular/angular.json')).projects['demo-angular'].architect.build.configurations.production;
  assert.deepEqual(prod.optimization.styles, { minify: false, inlineCritical: false });
});

test('Angular demo: the research profile is an asset, copied from the test profiles', () => {
  const assets = JSON.parse(source('demo-angular/angular.json')).projects['demo-angular'].architect.build.options.assets;
  const entry = assets.find((a) => a.glob === 'research.jsonld');
  assert.deepEqual(entry, { glob: 'research.jsonld', input: '../prose-core/tests/profiles', output: '/profiles' });
  assert.ok(fs.existsSync(path.join(REPO, 'prose-core/tests/profiles/research.jsonld')));
});

// A demo rule restyles the library's output when the thing it styles (its last
// class) is a library class the example sheets also style, and nothing in the
// selector says it is the demo's own element (a class that is not prose-*, or an id).
const isLibraryClass = (c) => /^(ds-)?prose(-|$)/.test(c);
const classesIn = (sel) => [...sel.matchAll(/\.([\w-]+)/g)].map((m) => m[1]);
export function restyled(selectors, exampleSelectors) {
  const taken = new Set(exampleSelectors.flatMap(classesIn).filter(isLibraryClass));
  return selectors.filter((sel) => {
    const classes = classesIn(sel);
    const last = classes.at(-1);
    const ownElement = /#[\w-]/.test(sel.replace(/\[[^\]]*\]/g, '')) || classes.some((c) => !isLibraryClass(c));
    return last !== undefined && taken.has(last) && !ownElement;
  });
}

test('restyled: finds a repeated library rule, leaves the demo\'s own elements and unknown library classes', () => {
  const ex = ['.ds-prose .prose-rule-sentence', '.ds-prose .prose-glossary dt', ':root .ds-prose.prose-edit .prose-slot'];
  assert.deepEqual(restyled(['.prose-rule-sentence', '.prose-glossary dt', '.ds-prose.prose-edit .prose-slot'], ex), ['.prose-rule-sentence', '.prose-glossary dt', '.ds-prose.prose-edit .prose-slot']);
  assert.deepEqual(restyled(['.conformance.prose-rule-sentence', '#problems .prose-rule-sentence', '.ds-prose.prose-edit .prose-toolbar', '.columns', 'body'], ex), []);
});

test('the demos do not restyle what the example stylesheets style', () => {
  const exampleSelectors = [...selectorsOf(content('examples/prose.css')), ...selectorsOf(content('examples/prose-editor.css'))];
  assert.ok(exampleSelectors.length > 40, 'the example selectors were not read');
  for (const f of ['demo/styles.css', 'demo-angular/src/styles.css']) {
    const own = selectorsOf(source(f));
    assert.ok(own.length > 3, `${f}: no selectors read`);
    assert.deepEqual(restyled(own, exampleSelectors), [], `${f} repeats rules of the example stylesheets`);
    assert.ok(!own.includes(':root'), `${f}: the page scheme is the example sheet's and the site's`);
  }
});

test('the body is unconstrained, so the navigation spans the page; the app root holds the width', () => {
  for (const [f, root] of [['demo/styles.css', '#app'], ['demo-angular/src/styles.css', 'app-root']]) {
    const css = source(f);
    const bodyRule = /(?:^|\})\s*body\s*\{([^}]*)\}/.exec(css)?.[1] ?? '';
    assert.doesNotMatch(bodyRule, /max-width|padding/, `${f}: body is constrained`);
    assert.match(bodyRule, /margin:\s*0\b/, `${f}: body margin`);
    const rootRule = new RegExp(`(?:^|\\})\\s*${root}\\s*\\{([^}]*)\\}`).exec(css)?.[1] ?? '';
    assert.match(rootRule, /max-width:\s*72rem/, `${f}: ${root} holds the width`);
    assert.match(rootRule, /display:\s*block/, `${f}: ${root} is a block`);
  }
});

test('the demos take their page colours from the site tokens, with system colours as the fallback', () => {
  for (const f of ['demo/styles.css', 'demo-angular/src/styles.css']) {
    const bodyRule = /(?:^|\})\s*body\s*\{([^}]*)\}/.exec(source(f))?.[1] ?? '';
    assert.match(bodyRule, /background:\s*var\(--bg,\s*Canvas\)/, f);
    assert.match(bodyRule, /color:\s*var\(--text,\s*CanvasText\)/, f);
  }
});

test('normaliseCss ignores comments, whitespace and quotes in attribute selectors, nothing else', () => {
  assert.equal(normaliseCss('/* c */ :root[data-theme="light"] { color: red; }'), normaliseCss(':root[data-theme=light]{color:red;}'));
  assert.notEqual(normaliseCss('a{color:red}'), normaliseCss('a{color:blue}'));
  assert.notEqual(normaliseCss('a{margin:0 0 2rem}'), normaliseCss('a{margin:00 2rem}'));
});

// ---- the stamped switch --------------------------------------------------------

test('stamping adds the switch after the navigation: two radios, themed checked, a link to the Styling page of the demo', () => {
  assert.equal(typeof navlib.renderSheetSwitch, 'function', 'nav.mjs exports renderSheetSwitch');
  const b = '/ds-prose-rs/';
  for (const [active, styling] of [['demo', 'yew/styling/'], ['demo-angular', 'angular/styling/']]) {
    const demoHtml = `<!doctype html><html><head><!--ds-topnav-css--></head><body><!--ds-topnav--><div id="app"></div></body></html>`;
    const out = navlib.stampDemo(demoHtml, { active, base: b });
    assert.equal(count(out, 'name="ds-sheet"'), 2, active);
    assert.match(out, /<input type="radio" name="ds-sheet" value="unstyled">/);
    assert.match(out, /<input type="radio" name="ds-sheet" value="themed" checked>/);
    assert.ok(out.includes(`href="${b}${styling}"`), `${active}: link to ${styling}`);
    assert.ok(out.indexOf('</header>') < out.indexOf('class="sheet-switch"') && out.indexOf('class="sheet-switch"') < out.indexOf('id="app"'));
  }
});

test('ordinary pages carry no switch', () => {
  const site = buildSite();
  for (const rel of ['index.html', 'yew/index.html', 'angular/styling/index.html']) assert.doesNotMatch(read(path.join(site.out, rel)), /ds-sheet/, rel);
});

test('the switch is a labelled group, with labels on the radios', () => {
  const html = navlib.renderSheetSwitch('demo', '/');
  assert.match(html, /<div class="sheet-switch" role="group" aria-labelledby="sheet-switch-label">/);
  assert.match(html, /<span id="sheet-switch-label">Example stylesheet<\/span>/);
  assert.match(html, /<label><input type="radio" name="ds-sheet" value="unstyled"> Unstyled<\/label>/);
  assert.match(html, /<label><input type="radio" name="ds-sheet" value="themed" checked> Themed<\/label>/);
});

test('in Chrome the switch disables the example stylesheets and nothing else, and back', { skip: !chrome && 'no Chrome or Chromium (set CHROME)' }, () => {
  const siteJs = fs.readFileSync(path.join(SITE_DIR, 'assets/site.js'), 'utf8');
  const html = `<!doctype html><html><head><meta charset="utf-8">
<style id="renderer">${content('examples/prose.css')}</style>
<style id="editor">${content('examples/prose-editor.css')}</style>
<style id="own">.own { border-left: 7px solid red; }</style>
<style id="library">.ds-prose.prose-edit .prose-slot { outline: 2px solid red; }</style>
</head><body>${navlib.renderSheetSwitch('demo', '/')}
<article class="ds-prose"><div class="prose-permission"><p class="prose-rule-sentence" id="s">may display</p></div><p class="own" id="o">x</p></article>
<script>${siteJs}</script>
${reporter(`
window.addEventListener('load', () => {
  const dis = () => ['renderer', 'editor', 'own', 'library'].map((id) => document.getElementById(id).sheet.disabled);
  const border = () => cs('#s', 'borderLeftWidth');
  const own = () => cs('#o', 'borderLeftWidth');
  const r = { start: { dis: dis(), border: border(), own: own() }, checked: document.querySelector('input[value=themed]').checked };
  document.querySelector('input[value=unstyled]').click();
  r.unstyled = { dis: dis(), border: border(), own: own() };
  document.querySelector('input[value=themed]').click();
  r.themed = { dis: dis(), border: border(), own: own() };
  out(r);
});`)}</body></html>`;
  const r = inChrome(html);
  assert.equal(r.error, undefined);
  assert.deepEqual(r.start, { dis: [false, false, false, false], border: '4px', own: '7px' });
  assert.equal(r.checked, true);
  assert.deepEqual(r.unstyled, { dis: [true, true, false, false], border: '0px', own: '7px' });
  assert.deepEqual(r.themed, { dis: [false, false, false, false], border: '4px', own: '7px' });
});
