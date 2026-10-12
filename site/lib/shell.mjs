import { escapeHtml } from './markdown.mjs';
import { headAssets, normaliseBase, renderHeader, REPO_URL } from './nav.mjs';

export function renderPage({ title, description, active, exact = true, base, body, subnav = '' }) {
  const b = normaliseBase(base);
  const main = `<main id="main">\n${body}</main>`;
  const content = subnav ? `<div class="layout">\n${subnav}\n${main}\n</div>` : main;
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escapeHtml(title)} - ds-prose-rs</title>
<meta name="description" content="${escapeHtml(description)}">
${headAssets(b)}
</head>
<body class="site">
<a class="skip-link" href="#main">Skip to content</a>
${renderHeader(active, b, { exact })}
${content}
<footer class="site-footer">
<p>ds-prose-rs is licensed under Apache-2.0. <a href="${REPO_URL}" rel="noopener">Source on GitHub</a>.</p>
</footer>
</body>
</html>
`;
}
