// The Angular demo against the component it demonstrates. Every binding in the
// template is an input or output the component declares, and the demo uses
// strict, profile and validationChange, which a downstream app would copy.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { source } from './contract.mjs';

const COMPONENT = source('prose-angular/src/odrl-prose.component.ts');
const TEMPLATE = source('demo-angular/src/app/app.html');
const APP = source('demo-angular/src/app/app.ts');
const FIXTURES = source('demo-angular/src/app/fixtures.ts');

const names = (kind) => [...COMPONENT.matchAll(new RegExp(`readonly (\\w+) = ${kind}(?:\\.required)?(?:<|\\()`, 'g'))].map((m) => m[1]);
const INPUTS = names('input');
const OUTPUTS = names('output');
const tag = /<ds-odrl-prose\b([\s\S]*?)\/>/.exec(TEMPLATE)?.[1] ?? '';
const bound = [...tag.matchAll(/\[(\w+)\]=/g)].map((m) => m[1]);
const listened = [...tag.matchAll(/\((\w+)\)=/g)].map((m) => m[1]);

test('the component declares the inputs and outputs the test reads', () => {
  assert.ok(INPUTS.includes('strict') && INPUTS.includes('profile') && INPUTS.includes('showViolations'));
  assert.ok(OUTPUTS.includes('validationChange') && OUTPUTS.includes('profileLoaded') && OUTPUTS.includes('profileError'));
});

test('the template binds [json], [profile], [strict] and [showViolations]', () => {
  for (const i of ['json', 'profile', 'strict', 'showViolations']) assert.ok(bound.includes(i), `[${i}] is not bound`);
});

test('the template listens to (validationChange), (profileLoaded) and (profileError)', () => {
  for (const o of ['validationChange', 'profileLoaded', 'profileError']) assert.ok(listened.includes(o), `(${o}) is not handled`);
});

test('every binding on <ds-odrl-prose> is an input or output of the component', () => {
  assert.ok(bound.length >= 4 && listened.length >= 3);
  for (const b of bound) assert.ok(INPUTS.includes(b), `[${b}] is not an input`);
  for (const l of listened) assert.ok(OUTPUTS.includes(l), `(${l}) is not an output`);
});

test('the verdict readout is a live region and shows what validationChange emitted', () => {
  assert.match(TEMPLATE, /id="verdict"[^>]*role="status"|role="status"[^>]*id="verdict"/);
  assert.match(APP, /ValidationReport/);
  assert.match(APP, /report\.verdict|\.verdict/);
});

test('the controls: a strict checkbox, a findings checkbox and a profile select, each with a label', () => {
  for (const id of ['strict', 'findings', 'profile']) {
    assert.match(TEMPLATE, new RegExp(`id="${id}"`), `#${id}`);
  }
  assert.match(TEMPLATE, /<label[^>]*>\s*<input[^>]*id="strict"|<label[^>]*for="strict"/);
  assert.match(TEMPLATE, /<label[^>]*for="profile"/);
  assert.match(TEMPLATE, /<select[^>]*id="profile"/);
});

test('the profile is fetched from the asset the build copies, relative to the base href', () => {
  assert.match(APP, /profiles\/research\.jsonld/);
  assert.match(APP, /document\.baseURI/);
});

test('the presets are the repository fixtures, parsed alike; the research policy is the fourth', () => {
  const presets = eval(`(${/PRESETS[^=]*= (\[[\s\S]*\]);/.exec(FIXTURES)[1]})`);
  const expected = [
    ['Offer with duty', 'prose-core/tests/fixtures/offer.json'],
    ['Agreement with logic', 'prose-core/tests/fixtures/agreement.json'],
    ['Set with prohibition', 'prose-core/tests/fixtures/set.json'],
    ['Research (needs the profile)', 'prose-core/tests/profiles/research-policy.json'],
  ];
  assert.deepEqual(presets.map((p) => p.label), expected.map((e) => e[0]));
  for (const [i, [, file]] of expected.entries()) assert.deepEqual(JSON.parse(presets[i].json), JSON.parse(source(file)), file);
});
