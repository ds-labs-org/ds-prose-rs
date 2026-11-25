// What the Angular styling page says about component styles, checked with the
// real Angular compiler's style encapsulation (the same function the CLI
// uses) on the styles of the typechecked examples, and in Chrome on the real
// renderer output. Skipped when prose-angular has no node_modules.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { REPO, content, source } from './contract.mjs';
import { chrome, inChrome, reporter } from './chrome.mjs';

const compilerPath = path.join(REPO, 'prose-angular/node_modules/@angular/compiler/fesm2022/compiler.mjs');
const skipAll = fs.existsSync(compilerPath) ? false : 'prose-angular/node_modules is not installed';
const { encapsulateStyle } = skipAll ? {} : await import(compilerPath);

const EXAMPLES = source('prose-angular/test/docs-examples.ts');
const region = (name) => new RegExp(`// region: ${name}\\n([\\s\\S]*?)// endregion: ${name}`).exec(EXAMPLES)[1];
const stylesOf = (name) => /styles: `([\s\S]*?)`/.exec(region(name))[1];
const OFFER = content('examples/offer.html');
const STYLING = content('angular/styling.md');

test('emulated encapsulation scopes a component rule to elements Angular made: the rule gains [_ngcontent-*]', { skip: skipAll }, () => {
  const out = encapsulateStyle('.prose-rule-sentence { font-weight: 600; }', 'c1');
  assert.equal(out, '.prose-rule-sentence[_ngcontent-c1] { font-weight: 600; }');
  assert.ok(STYLING.includes('.prose-rule-sentence[_ngcontent-'), 'the page quotes the rewritten selector');
});

test('the custom element itself is scoped to the parent, so ds-odrl-prose { display: block } works with default encapsulation', { skip: skipAll }, () => {
  assert.equal(encapsulateStyle('ds-odrl-prose { display: block; }', 'c1'), 'ds-odrl-prose[_ngcontent-c1] { display: block; }');
});

test(':host ::ng-deep drops the attribute from the right-hand side; a bare ::ng-deep drops all scoping', { skip: skipAll }, () => {
  assert.match(encapsulateStyle(stylesOf('ng_deep'), 'c1'), /\[_nghost-c1\]\s+\.prose-rule-sentence \{ font-weight: 600; \}/);
  assert.equal(encapsulateStyle('::ng-deep .prose-rule { color: red; }', 'c1'), '.prose-rule { color: red; }');
  assert.ok(STYLING.includes('[_nghost-'), 'the page quotes the :host ::ng-deep result');
});

test('the ViewEncapsulation.None example registers its styles unscoped', { skip: skipAll }, () => {
  assert.match(region('encapsulation_none'), /encapsulation: ViewEncapsulation\.None/);
  assert.ok(STYLING.includes('ViewEncapsulation.None'));
});

const html = (css, hostAttr) =>
  `<!doctype html><html><head><meta charset="utf-8"><style>${css}</style></head><body><ds-odrl-prose ${hostAttr}>${OFFER}</ds-odrl-prose>` +
  reporter(`out({ w: cs('.prose-rule-sentence', 'fontWeight'), d: cs('ds-odrl-prose', 'display') });`) + '</body></html>';

test('in a browser: the scoped rule misses the renderer output, the ng-deep rule and the unscoped rule reach it', { skip: skipAll || (chrome ? false : 'no Chrome'), timeout: 120_000 }, () => {
  // Angular puts _ngcontent-c1 on the elements its template creates (here the host element),
  // and nothing on the nodes Yew creates inside it.
  const attr = '_ngcontent-c1=""';
  const scoped = inChrome(html(encapsulateStyle('.prose-rule-sentence { font-weight: 700; }', 'c1'), attr));
  assert.equal(scoped.w, '400', 'a component rule does not reach the output');
  const deep = inChrome(html(encapsulateStyle(':host ::ng-deep .prose-rule-sentence { font-weight: 700; }', 'c1'), attr).replace('<ds-odrl-prose', '<div _nghost-c1=""><ds-odrl-prose').replace('</ds-odrl-prose>', '</ds-odrl-prose></div>'));
  assert.equal(deep.w, '700');
  const none = inChrome(html('.my-prose .prose-rule-sentence { font-weight: 700; }', attr).replace('<article class="ds-prose">', '<article class="ds-prose my-prose">'));
  assert.equal(none.w, '700');
  const host = inChrome(html(encapsulateStyle('ds-odrl-prose { display: block; }', 'c1'), attr));
  assert.equal(host.d, 'block');
  const bare = inChrome(html('', attr));
  assert.equal(bare.d, 'inline', 'an unknown element is inline until the host says otherwise');
});
