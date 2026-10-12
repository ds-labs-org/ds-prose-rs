// The one place the top navigation is produced. The same markup is written
// into every page and stamped into both demos, so they cannot drift apart.
import { escapeHtml } from './markdown.mjs';

export const REPO_URL = 'https://github.com/ds-labs-org/ds-prose-rs';

export const NAV = [
  { id: 'home', label: 'Home', path: '' },
  { id: 'demo', label: 'Demo (Yew)', path: 'demo/' },
  { id: 'demo-angular', label: 'Demo (Angular)', path: 'demo-angular/' },
  { id: 'yew', label: 'Yew docs', path: 'yew/' },
  { id: 'angular', label: 'Angular docs', path: 'angular/' },
];

export const PACKAGES = [
  { label: 'GitHub', href: REPO_URL },
  { label: 'crates.io (prose-yew)', href: 'https://crates.io/crates/prose-yew' },
  { label: 'npm (@ds-labs/prose-angular)', href: 'https://www.npmjs.com/package/@ds-labs/prose-angular' },
];

export const THEME_KEY = 'ds-prose-theme';

// Runs in <head>, before the stylesheet, so a stored choice is applied before
// first paint. Storage can throw (private windows, blocked site data).
export const THEME_INIT = `(function(){try{var t=localStorage.getItem('${THEME_KEY}');if(t==='light'||t==='dark'){document.documentElement.setAttribute('data-theme',t);}}catch(e){}})();`;

export function normaliseBase(base = '/') {
  let b = base.startsWith('/') ? base : '/' + base;
  return b.endsWith('/') ? b : b + '/';
}

export function renderNav(active, base, { exact = true } = {}) {
  const b = normaliseBase(base);
  const cur = exact ? 'page' : 'true';
  const items = NAV.map((n) => {
    const attr = n.id === active ? ` aria-current="${cur}"` : '';
    return `<li><a href="${b}${n.path}"${attr}>${escapeHtml(n.label)}</a></li>`;
  }).join('\n');
  const pkgs = PACKAGES.map((p) => `<li><a href="${escapeHtml(p.href)}" rel="noopener">${escapeHtml(p.label)}</a></li>`).join('\n');
  return `<nav class="topnav" aria-label="Primary">
<a class="brand" href="${b}">ds-prose-rs</a>
<button type="button" class="menu-toggle" aria-expanded="false" aria-controls="topnav-links" hidden>Menu</button>
<div class="topnav-links" id="topnav-links">
<ul class="topnav-list">
${items}
<li class="pkg"><details class="pkg-group"><summary>Packages</summary>
<ul>
${pkgs}
</ul>
</details></li>
</ul>
<button type="button" class="theme-toggle" aria-pressed="false">Theme: auto</button>
</div>
</nav>`;
}

export function renderHeader(active, base, opts) {
  return `<header class="site-header">\n${renderNav(active, base, opts)}\n</header>`;
}

export function renderSubnav({ section, pages, currentUrl }) {
  const label = NAV.find((n) => n.id === section)?.label ?? section;
  const items = pages
    .map((p) => `<li><a href="${p.url}"${p.url === currentUrl ? ' aria-current="page"' : ''}>${escapeHtml(p.label)}</a></li>`)
    .join('\n');
  return `<nav class="subnav" aria-label="${escapeHtml(label)} sections">\n<ul>\n${items}\n</ul>\n</nav>`;
}

// What <!--ds-topnav-css--> becomes in a head: stylesheet, theme init, script.
export function headAssets(base) {
  const b = normaliseBase(base);
  return `<script>${THEME_INIT}</script>\n<link rel="stylesheet" href="${b}assets/site.css">\n<script src="${b}assets/site.js" defer></script>`;
}

export const MARKER_NAV = '<!--ds-topnav-->';
export const MARKER_HEAD = '<!--ds-topnav-css-->';

function replaceOnce(html, marker, text) {
  const n = html.split(marker).length - 1;
  if (n === 0) throw new Error(`marker ${marker} not found`);
  if (n > 1) throw new Error(`marker ${marker} appears more than once (${n} times)`);
  return html.replace(marker, () => text);
}

export function stampDemo(html, { active, base }) {
  let out = replaceOnce(html, MARKER_HEAD, headAssets(base));
  out = replaceOnce(out, MARKER_NAV, renderHeader(active, base));
  return out;
}
