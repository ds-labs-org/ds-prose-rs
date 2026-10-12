# ds-prose-rs Angular demo

An Angular 20 app that uses `<ds-odrl-prose>` from `@ds-labs/prose-angular` the way a downstream app does. It installs the two packed npm packages, not a path into this repository.

It is served at <https://ds-labs-org.github.io/ds-prose-rs/demo-angular/>. The page has the ODRL JSON-LD on one side and the prose on the other, presets, and a Validation group: a Strict checkbox, a checkbox that shows the findings under the prose, and a Profile select. The app fetches the profile, binds `[profile]`, `[strict]` and `[showViolations]`, and shows the verdict from `(validationChange)` in a status region. A switch under the navigation turns the example stylesheet off and on.

## Build

From the repository root, pack the two packages once, then build the app under the path it is served from:

```bash
scripts/build-npm-packages.sh
cd demo-angular
npm ci
npm install --no-save ../dist-npm/tarballs/*.tgz
npx ng build --base-href /ds-prose-rs/demo-angular/
```

The output is `dist/demo-angular/browser`. `site/build.mjs` copies it to `/demo-angular/` of the site and replaces the two stamp markers in its `index.html` with the shared navigation. For a build served from the root, use `npx ng build` without `--base-href`.

## What the build holds

- `angular.json` lists `../site/content/examples/prose.css` as its own `styles` entry, before `src/styles.css`. That is the file the Angular Styling page prints, so the page and the demo cannot drift apart.
- The production configuration sets `optimization.styles` to `minify: false` and `inlineCritical: false`. The built `example-*.css` then holds the same rules as the source file, and `site/test/layout-check.mjs` compares the two.
- `src/styles.css` holds only the demo's own layout. It repeats none of the rules of the example stylesheet.
- The research profile is copied from `prose-core/tests/profiles/research.jsonld` into `profiles/` by an `assets` entry, and the wasm file is copied by the entry the Angular overview page prints.

## Development server

```bash
ng serve
```

Open `http://localhost:4200/`. The navigation markers in `src/index.html` are comments there.

## Unit tests

```bash
ng test
```

The app has no spec files. The site tests (`node --test "site/test/*.test.mjs"` at the repository root) check its template against the component, and `site/test/layout-check.mjs` drives the built app in Chrome.
