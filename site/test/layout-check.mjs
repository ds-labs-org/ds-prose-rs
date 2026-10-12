// Headless Chrome layout check for a built site tree. Dependency-free: it
// drives Chrome over the DevTools protocol with Node's built-in WebSocket.
//
//   node test/layout-check.mjs [dist] [base]
//   CHROME=/path/to/chrome   browser to use (else google-chrome, chromium)
//   SHOT_DIR=/some/dir       write one screenshot per page, width and theme
//
// With no dist it builds the site sources with stand-in demos.
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

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
    fs.rmSync(profile, { recursive: true, force: true });
  };
  return { cdp, close };
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

const EXPECTED_LABELS = ['Home', 'Demo (Yew)', 'Demo (Angular)', 'Yew docs', 'Angular docs'];

export async function runLayoutCheck({ dist, base, chrome = findChrome(), shotDir }) {
  if (!chrome) throw new Error('no Chrome found; set CHROME');
  dist = path.resolve(dist);
  const files = [];
  const walk = (d) => fs.readdirSync(d, { withFileTypes: true }).forEach((e) => (e.isDirectory() ? walk(path.join(d, e.name)) : e.name.endsWith('.html') && files.push(path.join(d, e.name))));
  walk(dist);
  const urlPaths = files.map((f) => base + path.relative(dist, f).replace(/index\.html$/, '').split(path.sep).join('/'));

  const { server, port } = await serve(dist, base);
  const { cdp, close } = await launch(chrome);
  const failures = [];
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
            if (!p.listVisible || p.navVisibleLinks !== 5) fail(`wide nav shows ${p.navVisibleLinks} of 5 links`);
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
  } finally {
    close();
    server.close();
  }
  return { failures, checked };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const chrome = findChrome();
  if (!chrome) {
    console.log('layout check skipped: no Chrome or Chromium found (set CHROME=/path/to/chrome)');
    process.exit(0);
  }
  let [dist, base] = process.argv.slice(2);
  if (!dist) {
    const { buildSite } = await import('./helpers.mjs');
    const s = buildSite({ kitchenSink: true });
    dist = s.out;
    base = s.base;
  }
  const { failures, checked } = await runLayoutCheck({ dist, base: base ?? '/', chrome, shotDir: process.env.SHOT_DIR });
  console.log(`layout check: ${checked} page views, ${failures.length} failures`);
  failures.forEach((f) => console.error('  ' + f));
  process.exit(failures.length ? 1 : 0);
}
