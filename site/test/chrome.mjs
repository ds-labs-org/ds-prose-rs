// Run a page in headless Chrome and read back what a script in it reports.
// The script writes JSON into <pre id="result">; --dump-dom prints the DOM.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { findChrome } from './layout-check.mjs';

export const chrome = findChrome();

export function inChrome(html, { dark = false, width = 1000 } = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'site-chrome-'));
  const file = path.join(dir, 'page.html');
  fs.writeFileSync(file, html);
  const args = ['--headless=new', '--disable-gpu', '--no-sandbox', `--user-data-dir=${path.join(dir, 'profile')}`, `--window-size=${width},800`, '--virtual-time-budget=3000', '--dump-dom'];
  if (dark) args.push('--blink-settings=preferredColorScheme=1');
  const r = spawnSync(chrome, [...args, `file://${file}`], { encoding: 'utf8', timeout: 60_000 });
  fs.rmSync(dir, { recursive: true, force: true });
  const m = /<pre id="result">([\s\S]*?)<\/pre>/.exec(r.stdout ?? '');
  if (!m) throw new Error(`no result from Chrome: ${r.stderr?.slice(0, 500)}`);
  return JSON.parse(m[1].replace(/&quot;/g, '"').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&'));
}

export const reporter = (body) => `<pre id="result"></pre><script>
const out = (v) => { document.getElementById('result').textContent = JSON.stringify(v); };
const cs = (sel, prop) => getComputedStyle(document.querySelector(sel))[prop];
try { ${body} } catch (e) { out({ error: String(e) }); }
</script>`;
