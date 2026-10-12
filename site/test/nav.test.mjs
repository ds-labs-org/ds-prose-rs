import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { buildSite, htmlFiles, read, topnav, urlOf, DEMO_HTML } from './helpers.mjs';
import { renderNav, stampDemo, NAV, PACKAGES } from '../lib/nav.mjs';

const site = buildSite();
const B = site.base;
const files = htmlFiles(site.out).map((f) => ({ url: urlOf(site.out, B, f), html: read(f) }));
const normalise = (nav) => nav.replace(/ aria-current="[^"]*"/g, '');
const topLinks = (nav) => [...nav.matchAll(/<li><a href="([^"]+)"( aria-current="(\w+)")?>([^<]+)<\/a><\/li>/g)].map((m) => ({ href: m[1], current: m[3], label: m[4] }));

test('the top entries are Home, Demo (Yew), Demo (Angular), Yew docs, Angular docs, in that order', () => {
  assert.deepEqual(NAV.map((n) => n.label), ['Home', 'Demo (Yew)', 'Demo (Angular)', 'Yew docs', 'Angular docs']);
  const links = topLinks(renderNav('home', B));
  assert.deepEqual(links.map((l) => l.href), [B, B + 'demo/', B + 'demo-angular/', B + 'yew/', B + 'angular/']);
});

test('a Packages group lists GitHub, crates.io and npm, as external links', () => {
  assert.deepEqual(PACKAGES.map((p) => p.label), ['GitHub', 'crates.io (prose-yew)', 'npm (@ds-labs/prose-angular)']);
  const nav = renderNav('home', B);
  assert.match(nav, /<details class="pkg-group"><summary>Packages<\/summary>/);
  for (const p of PACKAGES) assert.ok(nav.includes(`href="${p.href}"`), p.href);
  assert.ok(PACKAGES.every((p) => p.href.startsWith('https://')));
});

test('menu button and the list it controls are wired by id', () => {
  const nav = renderNav('home', B);
  assert.match(nav, /<button type="button" class="menu-toggle" aria-expanded="false" aria-controls="topnav-links" hidden>Menu<\/button>/);
  assert.match(nav, /<div class="topnav-links" id="topnav-links">/);
});

test('the theme toggle is a button with aria-pressed', () => {
  assert.match(renderNav('home', B), /<button type="button" class="theme-toggle" aria-pressed="false">Theme: auto<\/button>/);
});

test('exactly one top entry is current; page for exact, true for a section', () => {
  const exact = topLinks(renderNav('yew', B, { exact: true }));
  assert.deepEqual(exact.filter((l) => l.current).map((l) => [l.label, l.current]), [['Yew docs', 'page']]);
  const section = topLinks(renderNav('yew', B, { exact: false }));
  assert.deepEqual(section.filter((l) => l.current).map((l) => [l.label, l.current]), [['Yew docs', 'true']]);
  assert.equal(topLinks(renderNav(null, B)).filter((l) => l.current).length, 0);
});

test('nav links honour the base path', () => {
  const nav = renderNav('home', '/');
  assert.ok(nav.includes('href="/yew/"'));
  assert.ok(!nav.includes('ds-prose-rs/yew'));
});

const expectCurrent = {
  [B]: ['Home', 'page'],
  [B + 'demo/']: ['Demo (Yew)', 'page'],
  [B + 'demo-angular/']: ['Demo (Angular)', 'page'],
  [B + 'yew/']: ['Yew docs', 'page'],
  [B + 'angular/']: ['Angular docs', 'page'],
  [B + 'yew/interface/']: ['Yew docs', 'true'],
};

test('the nav is the same on every page and both demos, apart from aria-current', () => {
  const navs = files.map((f) => ({ url: f.url, nav: topnav(f.html) }));
  assert.ok(navs.length >= 8, `only ${navs.length} pages carry a nav`);
  for (const n of navs) assert.ok(n.nav, `no topnav on ${n.url}`);
  const ref = normalise(navs[0].nav);
  for (const n of navs) assert.equal(normalise(n.nav), ref, `nav differs on ${n.url}`);
});

test('each page marks the right entry', () => {
  for (const [url, [label, kind]] of Object.entries(expectCurrent)) {
    const f = files.find((x) => x.url === url);
    assert.ok(f, `no page ${url}`);
    const cur = topLinks(topnav(f.html)).filter((l) => l.current);
    assert.deepEqual(cur.map((l) => [l.label, l.current]), [[label, kind]], url);
  }
  const nf = files.find((x) => x.url === B + '404.html');
  assert.equal(topLinks(topnav(nf.html)).filter((l) => l.current).length, 0);
});

test('docs pages carry a labelled section sub-nav with the current page marked', () => {
  const f = files.find((x) => x.url === B + 'yew/interface/');
  const sub = /<nav class="subnav" aria-label="Yew docs sections">[\s\S]*?<\/nav>/.exec(f.html);
  assert.ok(sub, 'no subnav');
  assert.match(sub[0], new RegExp(`<a href="${B}yew/">Overview</a>`));
  assert.match(sub[0], new RegExp(`<a href="${B}yew/interface/" aria-current="page">Interface</a>`));
  assert.equal((sub[0].match(/aria-current/g) || []).length, 1);
  const home = files.find((x) => x.url === B);
  assert.doesNotMatch(home.html, /class="subnav"/);
});

test('demos are stamped: markers replaced, doctype and ids kept, assets linked once', () => {
  for (const [dir, title] of [['demo', 'ds-prose-rs demo'], ['demo-angular', 'ds-prose-rs Angular demo']]) {
    const html = read(path.join(site.out, dir, 'index.html'));
    assert.doesNotMatch(html, /ds-topnav/);
    assert.match(html, /^<!doctype html>/i);
    assert.ok(html.includes(`<title>${title}</title>`));
    assert.ok(html.includes('class="prose-policy"'));
    assert.equal((html.match(/assets\/site\.css/g) || []).length, 1);
    assert.equal((html.match(/assets\/site\.js/g) || []).length, 1);
    assert.ok(html.indexOf('<nav class="topnav"') > html.indexOf('<body'));
  }
});

test('stamping fails when a marker is missing or appears twice', () => {
  const ok = DEMO_HTML('x');
  assert.doesNotThrow(() => stampDemo(ok, { active: 'demo', base: B }));
  assert.throws(() => stampDemo(ok.replace('<!--ds-topnav-->', ''), { active: 'demo', base: B }), /ds-topnav/);
  assert.throws(() => stampDemo(ok.replace('<!--ds-topnav-css-->', ''), { active: 'demo', base: B }), /ds-topnav-css/);
  assert.throws(() => stampDemo(ok.replace('</body>', '<!--ds-topnav--></body>'), { active: 'demo', base: B }), /twice|more than once|2/);
});
