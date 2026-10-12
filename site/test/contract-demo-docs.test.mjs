// The Styling pages say how the demos use the example stylesheets. What they
// say is read back from the demo sources and from the switch the build stamps.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { content, frontMatterless, sections, source } from './contract.mjs';
import { renderSheetSwitch } from '../lib/nav.mjs';

const switchHtml = renderSheetSwitch('demo', '/');
const label = /id="sheet-switch-label">([^<]+)</.exec(switchHtml)[1];
const radios = [...switchHtml.matchAll(/value="(\w+)"[^>]*> ([^<]+)</g)].map((m) => m[2]);
const demoSection = (md) => {
  const s = sections(frontMatterless(md), 2);
  const key = [...s.keys()].find((k) => /demo/i.test(k));
  assert.ok(key, `no "## ... demo" section; has ${[...s.keys()].join(', ')}`);
  return { key, text: s.get(key) };
};

test('the switch is called "Example stylesheet", with the radios Unstyled and Themed', () => {
  assert.equal(label, 'Example stylesheet');
  assert.deepEqual(radios, ['Unstyled', 'Themed']);
});

test('Yew styling: the demo section names the switch, both example files and the demo page', () => {
  const { text } = demoSection(content('yew/styling.md'));
  assert.ok(text.includes('[Yew demo](/demo/)'));
  for (const r of radios) assert.ok(text.includes(r), r);
  assert.ok(text.includes(label));
  const links = [...source('demo/index.html').matchAll(/<link data-trunk rel="css" href="\.\.\/([^"]+)"/g)].map((m) => m[1]);
  assert.deepEqual(links, ['site/content/examples/prose.css', 'site/content/examples/prose-editor.css']);
  for (const l of links) assert.ok(text.includes(`\`${l}\``), l);
  assert.match(text, /BASE_CSS/);
});

test('Angular styling: the demo section names the switch, the styles entry and the page', () => {
  const { text } = demoSection(content('angular/styling.md'));
  assert.ok(text.includes('[Angular demo](/demo-angular/)'));
  for (const r of radios) assert.ok(text.includes(r), r);
  assert.ok(text.includes(label));
  const entry = JSON.parse(source('demo-angular/angular.json')).projects['demo-angular'].architect.build.options.styles[0];
  assert.ok(text.includes(`\`${entry.input.replace('../', '')}\``), entry.input);
  assert.ok(text.includes(`\`bundleName\``) && text.includes(`\`${entry.bundleName}\``));
});

test('both pages say what stays on when the example stylesheet is off', () => {
  assert.match(demoSection(content('yew/styling.md')).text, /layout/i);
  assert.match(demoSection(content('angular/styling.md')).text, /layout/i);
});
