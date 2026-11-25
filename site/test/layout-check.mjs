// Headless Chrome layout check for a built site tree. Dependency-free: it
// drives Chrome over the DevTools protocol with Node's built-in WebSocket.
//
//   node test/layout-check.mjs [dist] [base]
//   CHROME=/path/to/chrome   browser to use (else google-chrome, chromium)
//   SHOT_DIR=/some/dir       write one screenshot per page, width and theme
//   LAYOUT_REQUIRE_CHROME=1  fail, rather than skip, when no browser is found
//
// With no dist it builds the site sources with stand-in demos. With a dist it
// is a tree built from the real demos, and the demo checks run as well: boot,
// the stamped navigation, the site theme, the example stylesheet, the switch,
// and the Angular demo's validation controls.
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { normaliseCss } from './css.mjs';

export const VIEWPORTS = [
  { width: 1280, height: 900 },
  { width: 400, height: 800 },
];
export const THEMES = ['light', 'dark'];
const AA = 4.5;

export function findChrome(env = process.env, exists = (c) => spawnSync('which', [c]).status === 0) {
  if (env.CHROME) return env.CHROME;
  return ['google-chrome', 'chromium', 'chromium-browser'].find(exists) ?? null;
}

const MIME = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript', '.json': 'application/json', '.wasm': 'application/wasm', '.svg': 'image/svg+xml' };

function serve(dist, base) {
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://x');
    if (!url.pathname.startsWith(base)) return res.writeHead(404).end('outside base');
    let file = path.join(dist, decodeURIComponent(url.pathname.slice(base.length)));
    if (!file.startsWith(dist)) return res.writeHead(403).end();
    if (fs.existsSync(file) && fs.statSync(file).isDirectory()) file = path.join(file, 'index.html');
    if (!fs.existsSync(file)) return res.writeHead(404).end('not found');
    res.writeHead(200, { 'content-type': MIME[path.extname(file)] ?? 'application/octet-stream' }).end(fs.readFileSync(file));
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve({ server, port: server.address().port })));
}

class Cdp {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    this.listeners = [];
    ws.addEventListener('message', (m) => {
      const msg = JSON.parse(m.data);
      if (msg.id && this.pending.has(msg.id)) {
        const { resolve, reject } = this.pending.get(msg.id);
        this.pending.delete(msg.id);
        msg.error ? reject(new Error(`${msg.error.message}`)) : resolve(msg.result);
      } else if (msg.method) this.listeners.forEach((l) => l(msg));
    });
  }
  send(method, params = {}, sessionId) {
    const id = ++this.id;
    this.ws.send(JSON.stringify({ id, method, params, sessionId }));
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
  }
  once(method, sessionId, ms = 15000) {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`timeout waiting for ${method}`)), ms);
      const l = (msg) => {
        if (msg.method === method && msg.sessionId === sessionId) {
          clearTimeout(timer);
          this.listeners = this.listeners.filter((x) => x !== l);
          resolve(msg.params);
        }
      };
      this.listeners.push(l);
    });
  }
}

async function launch(chrome) {
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'layout-chrome-'));
  const proc = spawn(chrome, ['--headless=new', '--disable-gpu', '--no-sandbox', '--hide-scrollbars', '--remote-debugging-port=0', `--user-data-dir=${profile}`, 'about:blank'], { stdio: 'ignore' });
  const portFile = path.join(profile, 'DevToolsActivePort');
  for (let i = 0; i < 100 && !fs.existsSync(portFile); i++) await new Promise((r) => setTimeout(r, 100));
  if (!fs.existsSync(portFile)) {
    proc.kill();
    throw new Error('Chrome did not open a DevTools port');
  }
  const [port, wsPath] = fs.readFileSync(portFile, 'utf8').trim().split('\n');
  const ws = new WebSocket(`ws://127.0.0.1:${port}${wsPath}`);
  await new Promise((resolve, reject) => {
    ws.addEventListener('open', resolve);
    ws.addEventListener('error', () => reject(new Error('DevTools connection failed')));
  });
  const cdp = new Cdp(ws);
  const close = () => {
    try { ws.close(); } catch { /* already closed */ }
    proc.kill('SIGKILL');
    proc.unref();
    // Chrome's helper processes may still be writing into the profile for a
    // moment; a profile that cannot be removed is not a failed check.
    try { fs.rmSync(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 }); } catch { /* temp dir, cleaned by the OS */ }
  };
  return { cdp, close };
}


const EXAMPLES = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../content/examples');

function cssFiles(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = path.join(dir, e.name);
    return e.isDirectory() ? cssFiles(p) : e.name.endsWith('.css') ? [p] : [];
  });
}

// The demos apply the documented example stylesheets. A bundler may re-print a
// sheet (comments, quotes in attribute selectors, whitespace), so the rules are
// compared in a normal form. The Trunk demo ships both sheets, the Angular demo
// the renderer sheet.
export function exampleCssFailures(dist) {
  const out = [];
  const want = { demo: ['prose.css', 'prose-editor.css'], 'demo-angular': ['prose.css'] };
  for (const [demo, sheets] of Object.entries(want)) {
    const dir = path.join(dist, demo);
    if (!fs.existsSync(dir)) {
      out.push(`${demo}/ is not in the tree`);
      continue;
    }
    const built = cssFiles(dir).map((f) => normaliseCss(fs.readFileSync(f, 'utf8')));
    for (const sheet of sheets) {
      const example = normaliseCss(fs.readFileSync(path.join(EXAMPLES, sheet), 'utf8'));
      if (!built.some((b) => b.includes(example))) out.push(`${demo}/ ships no stylesheet holding examples/${sheet} rule for rule`);
    }
    const own = built.join('');
    if (demo === 'demo-angular' && own.includes('.ds-prose.prose-edit')) out.push('demo-angular/ carries the editor sheet, which Angular does not expose');
  }
  return out;
}

// What a script in the page reads about a demo: layout, theme, stylesheet.
const DEMO_PROBE = `(() => {
  const r = (sel) => { const e = document.querySelector(sel); return e ? e.getBoundingClientRect() : null; };
  const root = document.querySelector('#app, app-root');
  const rule = document.querySelector('.prose-permission > .prose-rule-sentence');
  const cs = (e, p) => getComputedStyle(e)[p];
  const header = r('.site-header'), rr = root && root.getBoundingClientRect();
  return {
    headerWidth: header && header.width, headerLeft: header && header.left, innerWidth: window.innerWidth,
    rootWidth: rr && rr.width, rootDisplay: root && cs(root, 'display'),
    bodyBg: cs(document.body, 'backgroundColor'), bodyColor: cs(document.body, 'color'),
    scheme: cs(document.documentElement, 'colorScheme'),
    ruleBorder: rule ? cs(rule, 'borderLeftWidth') : null, ruleColor: rule ? cs(rule, 'borderLeftColor') : null,
    switchRadios: [...document.querySelectorAll('.sheet-switch input[name=ds-sheet]')].map((i) => i.value),
    switchVisible: !!document.querySelector('.sheet-switch') && document.querySelector('.sheet-switch').getBoundingClientRect().height > 0,
    current: [...document.querySelectorAll('.topnav-list > li > a[aria-current]')].map((a) => a.textContent.trim() + ':' + a.getAttribute('aria-current')),
    policies: document.querySelectorAll('.prose-policy').length,
  };
})()`;

const rgb = (hex) => { const n = parseInt(hex.slice(1), 16); return `rgb(${n >> 16}, ${(n >> 8) & 255}, ${n & 255})`; };

async function demoChecks({ send, evaluate, visit, waitFor, key, base, failures, errors, shotDir }) {
  const metrics = (width, height = 900) => send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
  const shot = async (name) => {
    if (!shotDir) return;
    fs.mkdirSync(shotDir, { recursive: true });
    const { data } = await send('Page.captureScreenshot', { format: 'png' });
    fs.writeFileSync(path.join(shotDir, name), Buffer.from(data, 'base64'));
  };
  const demos = [
    { path: 'demo/', name: 'Yew', current: 'Demo (Yew):page' },
    { path: 'demo-angular/', name: 'Angular', current: 'Demo (Angular):page' },
  ];
  // Site tokens, light and dark: the demo page follows them.
  const SITE = { light: { bg: rgb('#f6f8fa'), fg: rgb('#16202b'), allow: rgb('#15803d') }, dark: { bg: rgb('#0f1720'), fg: rgb('#e6edf3'), allow: rgb('#3fb950') } };

  for (const demo of demos) {
    const url = base + demo.path;
    for (const width of [1280, 400]) {
      await metrics(width);
      await send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'light' }] });
      errors.length = 0;
      await visit(url);
      const tag = `${demo.name} demo @${width}`;
      const p = await evaluate(DEMO_PROBE);
      if (!p.policies) failures.push(`${tag}: no policy rendered`);
      if (errors.length) failures.push(`${tag}: uncaught errors ${errors.join('; ')}`);
      if (JSON.stringify(p.current) !== JSON.stringify([demo.current])) failures.push(`${tag}: current entries ${JSON.stringify(p.current)}`);
      if (Math.abs(p.headerWidth - p.innerWidth) > 1 || p.headerLeft !== 0) failures.push(`${tag}: the navigation does not span the page (${p.headerLeft}, ${p.headerWidth} of ${p.innerWidth})`);
      if (p.rootDisplay !== 'block') failures.push(`${tag}: app root is display ${p.rootDisplay}`);
      if (width === 1280 && !(p.rootWidth > 600 && p.rootWidth <= 1153)) failures.push(`${tag}: app root is ${p.rootWidth}px wide, want 600 to 1152`);
      if (JSON.stringify(p.switchRadios) !== JSON.stringify(['unstyled', 'themed']) || !p.switchVisible) failures.push(`${tag}: the stylesheet switch is missing or hidden (${JSON.stringify(p.switchRadios)})`);
      if (p.ruleBorder !== '4px') failures.push(`${tag}: the example stylesheet is not applied (permission border ${p.ruleBorder})`);
      await shot(`demo-${demo.name.toLowerCase()}-${width}-themed.png`);
    }

    // Follows the site theme, in both directions, and its own layout holds.
    await metrics(1280);
    await visit(url);
    for (const theme of ['dark', 'light']) {
      await evaluate(`document.documentElement.setAttribute('data-theme', '${theme}')`);
      const p = await evaluate(DEMO_PROBE);
      if (p.bodyBg !== SITE[theme].bg) failures.push(`${demo.name} demo ${theme}: page background ${p.bodyBg}, want ${SITE[theme].bg}`);
      if (p.bodyColor !== SITE[theme].fg) failures.push(`${demo.name} demo ${theme}: text colour ${p.bodyColor}, want ${SITE[theme].fg}`);
      if (p.ruleColor !== SITE[theme].allow) failures.push(`${demo.name} demo ${theme}: permission mark ${p.ruleColor}, want ${SITE[theme].allow}`);
      if (p.scheme !== theme) failures.push(`${demo.name} demo ${theme}: color-scheme ${p.scheme}`);
    }
    await evaluate(`document.documentElement.setAttribute('data-theme', 'dark')`);
    await new Promise((r) => setTimeout(r, 250)); // links fade their colour over 120 ms
    await shot(`demo-${demo.name.toLowerCase()}-1280-dark.png`);

    // The switch: unstyled drops the example stylesheet, themed brings it back.
    await visit(url);
    await evaluate(`document.querySelector('.sheet-switch input[value=unstyled]').click()`);
    let p = await evaluate(DEMO_PROBE);
    if (p.ruleBorder !== '0px') failures.push(`${demo.name} demo: unstyled still shows the permission mark (${p.ruleBorder})`);
    if (p.rootDisplay !== 'block' || p.headerWidth < 1270) failures.push(`${demo.name} demo: unstyled breaks the demo's own layout`);
    await shot(`demo-${demo.name.toLowerCase()}-1280-unstyled.png`);
    await evaluate(`document.querySelector('.sheet-switch input[value=themed]').click()`);
    p = await evaluate(DEMO_PROBE);
    if (p.ruleBorder !== '4px') failures.push(`${demo.name} demo: themed does not bring the example stylesheet back (${p.ruleBorder})`);
  }

  // Angular demo: strict, profile and validationChange, through the page.
  await metrics(1280);
  const ng = base + 'demo-angular/';
  await visit(ng);
  const read = () => evaluate(`({
    verdict: (document.getElementById('verdict') || {}).textContent || null,
    status: (document.querySelector('.prose-violations-status') || {}).textContent || null,
    statusClass: (document.querySelector('.prose-violations-status') || {}).className || '',
    glossary: !!document.querySelector('.prose-glossary'),
    strict: !!document.getElementById('strict') && document.getElementById('strict').checked,
    live: document.getElementById('verdict') && document.getElementById('verdict').getAttribute('role'),
  })`);
  let v = await read();
  if (v.live !== 'status') failures.push(`Angular demo: #verdict is not a status region (${v.live})`);
  if (!/off/i.test(v.verdict ?? '') || v.status) failures.push(`Angular demo: validation should start off (${JSON.stringify(v)})`);
  await evaluate(`document.getElementById('strict').click()`);
  const verdictIs = (re) => waitFor(`/${re}/i.test((document.getElementById('verdict') || {}).textContent || '')`, 8000);
  if (!(await verdictIs('incomplete|conforms|violations'))) failures.push(`Angular demo: strict on, but validationChange never reached the page (${JSON.stringify(await read())})`);
  await evaluate(`document.getElementById('findings').click()`);
  await waitFor(`document.querySelector('.prose-violations-status')`, 5000);
  v = await read();
  const word = /conforms|violations|incomplete/i.exec(v.verdict ?? '')?.[0]?.toLowerCase();
  const CLASS = { conforms: 'prose-conforms', violations: 'prose-violations-found', incomplete: 'prose-incomplete' };
  if (!v.status || !word || !v.statusClass.split(/\s+/).includes(CLASS[word])) failures.push(`Angular demo: the readout and the findings under the prose disagree (${JSON.stringify(v)})`);
  await shot('demo-angular-1280-strict.png');

  // The research policy declares a profile; loading it through [profile] changes the verdict and adds the glossary.
  await evaluate(`[...document.querySelectorAll('.presets button')].find((b) => /Research/.test(b.textContent)).click()`);
  await waitFor(`/research/i.test(document.getElementById('policy').value)`, 3000);
  const before = (await read()).verdict;
  await evaluate(`(() => { const s = document.getElementById('profile'); s.value = 'research'; s.dispatchEvent(new Event('change', { bubbles: true })); })()`);
  await waitFor(`document.querySelector('.prose-glossary')`, 8000);
  await new Promise((r) => setTimeout(r, 300));
  v = await read();
  if (!v.glossary) failures.push(`Angular demo: [profile] did not reach the component (no glossary): ${JSON.stringify(v)}`);
  if (v.verdict === before) failures.push(`Angular demo: loading the profile left the verdict as it was (${before})`);
  if (!/conforms|violations|incomplete/i.test(v.verdict ?? '')) failures.push(`Angular demo: no verdict with the profile loaded (${JSON.stringify(v)})`);
  await shot('demo-angular-1280-profile.png');

  // Strict off: the readout says so and the findings go.
  await evaluate(`document.getElementById('strict').click()`);
  await waitFor(`/off/i.test(document.getElementById('verdict').textContent)`, 3000);
  v = await read();
  if (!/off/i.test(v.verdict ?? '') || v.status) failures.push(`Angular demo: strict off should clear the verdict and the findings (${JSON.stringify(v)})`);

  // Narrow screen: the validation controls fit.
  await metrics(400, 800);
  await visit(ng);
  await evaluate(`document.getElementById('strict').click()`);
  await verdictIs('incomplete|conforms|violations');
  const narrow = await evaluate(`({ sw: document.documentElement.scrollWidth, iw: window.innerWidth })`);
  if (narrow.sw > narrow.iw) failures.push(`Angular demo @400 with validation on: horizontal overflow (${narrow.sw})`);
  await shot('demo-angular-400-strict.png');
  void key;
}

// Runs inside the page. Returns plain data; the Node side decides pass or fail.
const PROBE = `(() => {
  const parse = (c) => { const m = /rgba?\\(([^)]+)\\)/.exec(c); if (!m) return null; const p = m[1].split(/[ ,\\/]+/).filter(Boolean).map(Number); return { r: p[0], g: p[1], b: p[2], a: p.length > 3 ? p[3] : 1 }; };
  const lum = (c) => { const f = (v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }; return 0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b); };
  const bgOf = (el) => { for (let e = el; e; e = e.parentElement) { const c = parse(getComputedStyle(e).backgroundColor); if (c && c.a === 1) return c; } const dark = getComputedStyle(document.documentElement).colorScheme.trim() === 'dark' || (document.documentElement.getAttribute('data-theme') !== 'light' && matchMedia('(prefers-color-scheme: dark)').matches); return dark ? { r: 18, g: 18, b: 18, a: 1 } : { r: 255, g: 255, b: 255, a: 1 }; };
  const ratio = (el) => { const fg = parse(getComputedStyle(el).color), bg = bgOf(el); const [a, b] = [lum(fg), lum(bg)].sort((x, y) => y - x); return (a + 0.05) / (b + 0.05); };
  const visible = (el) => { const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0; };
  const pick = (sel) => [...document.querySelectorAll(sel)].find(visible);
  const nav = document.querySelector('nav.topnav');
  const menu = nav && nav.querySelector('.menu-toggle');
  const list = nav && nav.querySelector('.topnav-links');
  const body = pick('main p, #app p, body > p');
  const navLink = nav && [...nav.querySelectorAll('.topnav-list > li > a')].find(visible);
  const current = nav ? [...nav.querySelectorAll('.topnav-list > li > a[aria-current]')] : [];
  return {
    scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth,
    hasNav: !!nav,
    navLabels: nav ? [...nav.querySelectorAll('.topnav-list > li > a')].map((a) => a.textContent.trim()) : [],
    navVisibleLinks: nav ? [...nav.querySelectorAll('.topnav-list > li > a')].filter(visible).length : 0,
    menuVisible: !!menu && visible(menu), listVisible: !!list && visible(list),
    currentCount: current.length,
    bodyRatio: body ? ratio(body) : null, navRatio: navLink ? ratio(navLink) : null,
    h1Ratio: pick('h1') ? ratio(pick('h1')) : null,
    mutedRatio: pick('.subnav a:not([aria-current])') ? ratio(pick('.subnav a:not([aria-current])')) : null,
    theme: document.documentElement.getAttribute('data-theme'),
    title: document.title,
  };
})()`;

const DEMO_PATH = /\/demo(-angular)?\/$/;
const EXPECTED_LABELS = ['Home', 'Demo (Yew)', 'Demo (Angular)', 'Yew docs', 'Angular docs', 'Catalog'];

export async function runLayoutCheck({ dist, base, chrome = findChrome(), shotDir, demos = false }) {
  if (!chrome) throw new Error('no Chrome found; set CHROME');
  dist = path.resolve(dist);
  const files = [];
  const walk = (d) => fs.readdirSync(d, { withFileTypes: true }).forEach((e) => (e.isDirectory() ? walk(path.join(d, e.name)) : e.name.endsWith('.html') && files.push(path.join(d, e.name))));
  walk(dist);
  const urlPaths = files.map((f) => base + path.relative(dist, f).replace(/index\.html$/, '').split(path.sep).join('/'));

  const { server, port } = await serve(dist, base);
  const { cdp, close } = await launch(chrome);
  const failures = [];
  if (demos) failures.push(...exampleCssFailures(dist));
  let checked = 0;
  try {
    const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId: sid } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    const send = (m, p) => cdp.send(m, p, sid);
    await send('Page.enable');
    await send('Runtime.enable');
    const errors = [];
    cdp.listeners.push((msg) => {
      if (msg.sessionId === sid && msg.method === 'Runtime.exceptionThrown') errors.push(msg.params.exceptionDetails.text + ' ' + (msg.params.exceptionDetails.exception?.description ?? ''));
    });
    const evaluate = async (expr) => {
      const r = await send('Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true });
      if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
      return r.result.value;
    };
    const visit = async (urlPath) => {
      const loaded = cdp.once('Page.loadEventFired', sid);
      await send('Page.navigate', { url: `http://127.0.0.1:${port}${urlPath}` });
      await loaded;
      await new Promise((r) => setTimeout(r, 150));
      if (demos && DEMO_PATH.test(urlPath)) await waitFor(`!!document.querySelector('.prose-policy')`, 20000);
    };
    const waitFor = async (expr, ms = 5000) => {
      const until = Date.now() + ms;
      while (Date.now() < until) {
        if (await evaluate(`!!(${expr})`)) return true;
        await new Promise((r) => setTimeout(r, 50));
      }
      return false;
    };
    const key = (k, code, vk, text) =>
      send('Input.dispatchKeyEvent', { type: text ? 'keyDown' : 'rawKeyDown', key: k, code, windowsVirtualKeyCode: vk, ...(text ? { text } : {}) }).then(() => send('Input.dispatchKeyEvent', { type: 'keyUp', key: k, code, windowsVirtualKeyCode: vk }));

    for (const vp of VIEWPORTS) {
      for (const theme of THEMES) {
        await send('Emulation.setDeviceMetricsOverride', { width: vp.width, height: vp.height, deviceScaleFactor: 1, mobile: false });
        await send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: theme }] });
        for (const urlPath of urlPaths) {
          const tag = `${urlPath} @${vp.width} ${theme}`;
          await visit(urlPath);
          checked++;
          const p = await evaluate(PROBE);
          const fail = (msg) => failures.push(`${tag}: ${msg}`);
          if (p.scrollWidth > p.innerWidth) fail(`horizontal overflow (scrollWidth ${p.scrollWidth} > ${p.innerWidth})`);
          if (!p.hasNav) fail('no topnav');
          else if (JSON.stringify(p.navLabels) !== JSON.stringify(EXPECTED_LABELS)) fail(`nav labels ${JSON.stringify(p.navLabels)}`);
          if (p.navRatio !== null && p.navRatio < AA && p.navVisibleLinks) fail(`nav text contrast ${p.navRatio.toFixed(2)} < ${AA}`);
          if (p.bodyRatio !== null && p.bodyRatio < AA) fail(`body text contrast ${p.bodyRatio.toFixed(2)} < ${AA}`);
          if (p.h1Ratio !== null && p.h1Ratio < AA) fail(`h1 contrast ${p.h1Ratio.toFixed(2)} < ${AA}`);
          if (p.mutedRatio !== null && p.mutedRatio < AA) fail(`muted text contrast ${p.mutedRatio.toFixed(2)} < ${AA}`);
          if (vp.width >= 768) {
            if (p.menuVisible) fail('menu button visible on a wide screen');
            if (!p.listVisible || p.navVisibleLinks !== EXPECTED_LABELS.length) fail(`wide nav shows ${p.navVisibleLinks} of ${EXPECTED_LABELS.length} links`);
          } else {
            if (!p.menuVisible) fail('menu button not visible on a narrow screen');
            if (p.listVisible) fail('nav links are open before the menu is used');
          }
          if (shotDir && /^\/?(yew\/interface\/|kitchen-sink\/|)$/.test(urlPath.slice(base.length))) {
            fs.mkdirSync(shotDir, { recursive: true });
            const { data } = await send('Page.captureScreenshot', { format: 'png' });
            const name = (urlPath.slice(base.length).replace(/\/$/, '').replace(/\//g, '-') || 'home') + `-${vp.width}-${theme}.png`;
            fs.writeFileSync(path.join(shotDir, name), Buffer.from(data, 'base64'));
          }
        }
      }
    }

    // The page with the awkward blocks: the kitchen sink in a fixture build, a long docs page with code blocks in a real tree.
    const sink = urlPaths.includes(base + 'kitchen-sink/') ? 'kitchen-sink/' : 'yew/styling/';

    // Menu by keyboard at a narrow width.
    await send('Emulation.setDeviceMetricsOverride', { width: 400, height: 800, deviceScaleFactor: 1, mobile: false });
    await visit(base);
    const state = () => evaluate(`({ expanded: document.querySelector('.menu-toggle').getAttribute('aria-expanded'), open: !document.getElementById('topnav-links').hidden, focusOnButton: document.activeElement === document.querySelector('.menu-toggle') })`);
    await evaluate(`document.querySelector('.menu-toggle').focus()`);
    await key('Enter', 'Enter', 13, '\r');
    let s = await state();
    if (s.expanded !== 'true' || !s.open) failures.push(`menu: Enter did not open it (${JSON.stringify(s)})`);
    if (shotDir) {
      fs.mkdirSync(shotDir, { recursive: true });
      const { data } = await send('Page.captureScreenshot', { format: 'png' });
      fs.writeFileSync(path.join(shotDir, 'menu-open-400.png'), Buffer.from(data, 'base64'));
    }
    await evaluate(`document.querySelector('.topnav-list a').focus()`);
    await key('Escape', 'Escape', 27);
    s = await state();
    if (s.expanded !== 'false' || s.open || !s.focusOnButton) failures.push(`menu: Escape did not close it and return focus (${JSON.stringify(s)})`);

    // Just under the breakpoint (48rem = 768 px) the menu must already collapse.
    await send('Emulation.setDeviceMetricsOverride', { width: 700, height: 800, deviceScaleFactor: 1, mobile: false });
    await visit(base);
    const mid = await evaluate(PROBE);
    if (!mid.menuVisible || mid.listVisible) failures.push(`at 700 px the menu button is ${mid.menuVisible ? 'shown' : 'hidden'} and the links are ${mid.listVisible ? 'open' : 'closed'}`);
    if (mid.scrollWidth > mid.innerWidth) failures.push(`horizontal overflow at 700 px (${mid.scrollWidth})`);

    // Copy button: copies exactly the code text, reports the outcome in a live
    // region, falls back when the async clipboard is missing or refuses, never
    // throws, and leaves keyboard focus on the button.
    await send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
    await visit(base + sink);
    const copyState = () => evaluate(`(() => { const b = document.querySelector('[data-copy]'); const st = document.querySelector('[role=status]'); return { label: b.textContent, status: st ? st.textContent : null, focusOnButton: document.activeElement === b, expected: b.parentElement.querySelector('code').textContent, copied: window.__copied ?? null, copiedBy: window.__copiedBy ?? null }; })()`);
    const clickCopy = async (setup) => {
      await visit(base + sink);
      errors.length = 0;
      await evaluate(setup + `; document.querySelector('[data-copy]').focus(); document.querySelector('[data-copy]').click();`);
      await new Promise((r) => setTimeout(r, 120));
      return copyState();
    };
    let c = await clickCopy(`Object.defineProperty(navigator, 'clipboard', { value: { writeText: (t) => { window.__copied = t; window.__copiedBy = 'clipboard'; return Promise.resolve(); } }, configurable: true })`);
    if (c.copied !== c.expected || c.label !== 'Copied' || c.status !== 'Copied' || !c.focusOnButton) failures.push(`copy (async clipboard): ${JSON.stringify(c)}`);
    c = await clickCopy(`Object.defineProperty(navigator, 'clipboard', { value: undefined, configurable: true }); document.execCommand = (cmd) => { const ta = document.querySelector('textarea'); window.__copied = ta && ta.value; window.__copiedBy = cmd; return true; }`);
    if (c.copied !== c.expected || c.copiedBy !== 'copy' || c.label !== 'Copied' || !c.focusOnButton) failures.push(`copy (no async clipboard, execCommand fallback): ${JSON.stringify(c)}`);
    c = await clickCopy(`Object.defineProperty(navigator, 'clipboard', { value: { writeText: () => Promise.reject(new Error('denied')) }, configurable: true }); document.execCommand = () => false`);
    if (c.label !== 'Copy failed' || c.status !== 'Copy failed' || !c.focusOnButton) failures.push(`copy (refused everywhere): ${JSON.stringify(c)}`);
    if (errors.length) failures.push(`copy: uncaught errors ${errors.join('; ')}`);
    c = await clickCopy(`Object.defineProperty(navigator, 'clipboard', { value: { writeText: () => { throw new Error('sync throw'); } }, configurable: true }); document.execCommand = () => false`);
    if (c.label !== 'Copy failed') failures.push(`copy (writeText throws synchronously): ${JSON.stringify(c)}`);

    // Focus is not hidden under the sticky header (WCAG 2.2, 2.4.11). The page scrolls smoothly, so wait for the position to settle.
    await send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 400, deviceScaleFactor: 1, mobile: false });
    await visit(base + sink);
    const obscured = await evaluate(`(async () => { window.scrollTo({ top: document.documentElement.scrollHeight, behavior: 'instant' }); const el = document.querySelector('main pre[tabindex], main a'); el.focus(); let last = -1, still = 0; for (let i = 0; i < 80 && still < 4; i++) { await new Promise((r) => setTimeout(r, 100)); still = window.scrollY === last ? still + 1 : 0; last = window.scrollY; } const h = document.querySelector('.site-header').getBoundingClientRect().bottom; return { top: el.getBoundingClientRect().top, header: h, scrolled: window.scrollY }; })()`);
    if (obscured.scrolled === 0) failures.push('focus check: page is too short to scroll');
    else if (obscured.top < obscured.header) failures.push(`focused element sits under the sticky header: ${JSON.stringify(obscured)}`);

    // Theme toggle: flips, persists across reload, survives throwing storage.
    await send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
    await visit(base);
    await evaluate(`localStorage.clear()`);
    await visit(base);
    await evaluate(`document.querySelector('.theme-toggle').click()`);
    let t = await evaluate(`({ theme: document.documentElement.getAttribute('data-theme'), pressed: document.querySelector('.theme-toggle').getAttribute('aria-pressed'), stored: localStorage.getItem('ds-prose-theme') })`);
    if (t.theme !== 'light' || t.pressed !== 'true' || t.stored !== 'light') failures.push(`theme toggle first click: ${JSON.stringify(t)}`);
    await visit(base);
    t = await evaluate(`document.documentElement.getAttribute('data-theme')`);
    if (t !== 'light') failures.push(`theme choice not applied after reload: ${t}`);
    await evaluate(`localStorage.clear()`);
    const { identifier } = await send('Page.addScriptToEvaluateOnNewDocument', {
      source: `Storage.prototype.getItem = Storage.prototype.setItem = Storage.prototype.removeItem = function () { throw new Error('storage blocked'); };`,
    });
    errors.length = 0;
    await visit(base);
    await evaluate(`document.querySelector('.theme-toggle').click()`);
    t = await evaluate(`document.documentElement.getAttribute('data-theme')`);
    if (t !== 'light') failures.push(`theme toggle fails when storage throws (data-theme=${t})`);
    if (errors.length) failures.push(`uncaught errors with storage blocked: ${errors.join('; ')}`);
    await send('Page.removeScriptToEvaluateOnNewDocument', { identifier });

    if (demos) await demoChecks({ send, evaluate, visit, waitFor, key, base, failures, errors, shotDir });
  } finally {
    close();
    server.close();
  }
  return { failures, checked };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const chrome = findChrome();
  if (!chrome) {
    const required = process.env.LAYOUT_REQUIRE_CHROME === '1';
    console[required ? 'error' : 'log'](`layout check ${required ? 'FAILED' : 'skipped'}: no Chrome or Chromium found (set CHROME=/path/to/chrome)`);
    process.exit(required ? 2 : 0);
  }
  let [dist, base] = process.argv.slice(2);
  const real = Boolean(dist);
  if (!dist) {
    const { buildSite } = await import('./helpers.mjs');
    const s = buildSite({ kitchenSink: true });
    dist = s.out;
    base = s.base;
  }
  const { failures, checked } = await runLayoutCheck({ dist, base: base ?? '/', chrome, shotDir: process.env.SHOT_DIR, demos: real });
  console.log(`layout check: ${checked} page views, ${failures.length} failures`);
  failures.forEach((f) => console.error('  ' + f));
  process.exit(failures.length ? 1 : 0);
}
