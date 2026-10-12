# Pages site

Static site for ds-prose-rs. Node 22 or later, no dependencies.

- `build.mjs` builds `content/*.md` into `dist/` and stamps the top navigation into the two demos. Environment: `SITE_BASE`, `SITE_OUT`, `SITE_DEMO_DIST`, `SITE_ANGULAR_DIST`, `SITE_SKIP_DEMOS=1`.
- `lib/markdown.mjs` is the markdown renderer. Anything outside its grammar is escaped.
- `lib/nav.mjs` is the only source of the top navigation. A demo `index.html` carries the markers `<!--ds-topnav-css-->` in its head and `<!--ds-topnav-->` in its body. A missing or repeated marker fails the build.
- `assets/site.css` and `assets/site.js` hold the theme tokens, layout and behaviour.

Tests: `npm test` in this directory, or `node --test "site/test/*.test.mjs"` from the repository root. The layout test needs Chrome or Chromium (set `CHROME`); without one it is skipped with a message. `SHOT_DIR=/some/dir npm test` also writes screenshots.
