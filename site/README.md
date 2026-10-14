# Pages site

Static site for ds-prose-rs. Node 22 or later, no dependencies.

- `build.mjs` builds `content/*.md` into `dist/` and stamps the top navigation into the two demos. Environment: `SITE_BASE`, `SITE_OUT`, `SITE_DEMO_DIST`, `SITE_ANGULAR_DIST`, `SITE_SKIP_DEMOS=1`, `SITE_REQUIRE_CATALOG=1`.
- `content/catalog/` is not in the repository: `scripts/build-catalog.sh` generates it at build time from ds-odrl-compliance-rdf and the W3C, EDC and Dataspace Protocol examples. Without it the build writes a catalog page that says so; with `SITE_REQUIRE_CATALOG=1` (CI) it fails instead.
- `lib/markdown.mjs` is the markdown renderer. Anything outside its grammar is escaped.
- `lib/nav.mjs` is the only source of the top navigation. A demo `index.html` carries the markers `<!--ds-topnav-css-->` in its head and `<!--ds-topnav-->` in its body. A missing or repeated marker fails the build.
- `assets/site.css` and `assets/site.js` hold the theme tokens, layout and behaviour.
- A stamped demo also gets a stylesheet switch under the navigation (`renderSheetSwitch` in `lib/nav.mjs`): two radios, Unstyled and Themed. `site.js` finds the example stylesheets by what they define and disables them or not. The demo's own layout sheet and the library's `BASE_CSS` stay on.

Tests: `npm test` in this directory, or `node --test "site/test/*.test.mjs"` from the repository root. The layout test needs Chrome or Chromium (set `CHROME`); without one it is skipped with a message. `SHOT_DIR=/some/dir npm test` also writes screenshots.

Whole tree: `node test/layout-check.mjs <dist> <base>` checks a built tree in headless Chrome. With a `dist` it treats the tree as the real one and runs the demo checks too: boot, the stamped navigation, the site theme, the example stylesheet against `content/examples/`, the switch, and the Angular demo's validation controls. `LAYOUT_REQUIRE_CHROME=1` makes a missing browser an error instead of a skip, as in CI. Without a `dist` it builds the sources with stand-in demos.
