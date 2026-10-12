// The Angular docs against the TypeScript sources and the demo's angular.json.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { content, frontMatterless, sections, source, squash, tableRows, tick, ticks } from './contract.mjs';

const interfaceMd = frontMatterless(content('angular/interface.md'));
const SEC = sections(interfaceMd);
const sec = (h) => {
  const s = SEC.get(h);
  assert.ok(s !== undefined, `angular/interface.md has no "## ${h}"; has ${[...SEC.keys()].join(', ')}`);
  return s;
};
const COMPONENT = source('prose-angular/src/odrl-prose.component.ts');
const col = (r, i) => r[i] ?? '';

function members(kind) {
  // readonly name = input.required<T>() | input<T>(arg) | input(arg) | output<T>() | signal<T>(arg)
  const re = new RegExp(`readonly (\\w+) = ${kind}(\\.required)?(?:<((?:[^<>]|<[^<>]*>)*)>)?\\(([^)]*)\\)`, 'g');
  return [...COMPONENT.matchAll(re)].map((m) => ({ name: m[1], required: !!m[2], generic: m[3] ? squash(m[3]) : null, arg: m[4].trim() }));
}

test('inputs: the table lists exactly the input() members, with type and default', () => {
  const inputs = members('input');
  assert.equal(inputs.length, 7);
  const rows = tableRows(sec('Inputs'));
  assert.deepEqual(rows.map((r) => tick(col(r, 0))).sort(), inputs.map((i) => i.name).sort());
  for (const i of inputs) {
    const row = rows.find((r) => tick(col(r, 0)) === i.name);
    const type = i.generic ?? (/^(true|false)$/.test(i.arg) ? 'boolean' : null);
    assert.ok(type, `cannot infer the type of ${i.name}`);
    assert.equal(squash(tick(col(row, 1)) ?? ''), type, `${i.name}: type`);
    const def = i.required ? 'required' : i.arg === '' ? 'undefined' : i.arg;
    assert.equal(i.required ? col(row, 2) : tick(col(row, 2)), def, `${i.name}: default`);
    assert.ok(col(row, 3).length > 10, `${i.name}: no meaning`);
  }
});

test('outputs: the table lists exactly the output() members and their payload types', () => {
  const outputs = members('output');
  assert.equal(outputs.length, 5);
  const rows = tableRows(sec('Outputs'));
  assert.deepEqual(rows.map((r) => tick(col(r, 0))).sort(), outputs.map((o) => o.name).sort());
  for (const o of outputs) {
    const row = rows.find((r) => tick(col(r, 0)) === o.name);
    assert.equal(squash(tick(col(row, 1)) ?? ''), o.generic, `${o.name}: payload`);
  }
});

test('the report signal: documented with its declared type, initial value, and the read-only advice', () => {
  const [sig] = members('signal');
  assert.equal(sig.name, 'report');
  const s = sec('Signal');
  const row = tableRows(s)[0];
  assert.equal(tick(col(row, 0)), 'report');
  assert.equal(squash(tick(col(row, 1))), `WritableSignal<${sig.generic}>`);
  assert.equal(tick(col(row, 2)), sig.arg);
  assert.match(s, /writable/i, 'the page must say the type is writable');
  assert.match(s, /read it, do not set it|treat it as read-only/i);
});

test('component facts: selector, standalone, empty template, OnPush, no styles, no encapsulation', () => {
  const meta = /@Component\(\{([\s\S]*?)\}\)\s*export class/.exec(COMPONENT)[1];
  assert.match(meta, /selector: 'ds-odrl-prose'/);
  assert.match(meta, /standalone: true/);
  assert.match(meta, /template: ''/);
  assert.match(meta, /ChangeDetectionStrategy\.OnPush/);
  assert.doesNotMatch(meta, /\bstyles?\b|encapsulation/);
  const md = frontMatterless(content('angular/index.md')) + interfaceMd;
  for (const claim of ['`ds-odrl-prose`', 'standalone', 'declares no `styles`', 'no `encapsulation`', 'OnPush']) assert.ok(md.includes(claim), `docs do not say ${claim}`);
});

test('PROSE_WASM_URL: the default string and the resolution rule are the code\'s', () => {
  const rt = source('prose-angular/src/wasm-runtime.ts');
  const dflt = /factory: \(\) => '([^']+)'/.exec(rt)[1];
  assert.equal(dflt, 'prose_wasm_bg.wasm');
  const s = sec('PROSE_WASM_URL');
  assert.ok(s.includes(`'${dflt}'`), 'default string not in the docs');
  assert.match(rt, /new URL\(url, document\.baseURI\)/);
  assert.ok(s.includes('document.baseURI'), 'docs do not say the URL resolves against document.baseURI');
});

test('ProseValidatorService.validate: the signature is the code\'s', () => {
  const svc = source('prose-angular/src/validator.service.ts');
  const sig = /async validate\(([\s\S]*?)\):\s*(Promise<[^{]+?)\s*\{/.exec(svc);
  const want = squash(`async validate(${sig[1]}): ${sig[2]}`);
  assert.ok(squash(sec('ProseValidatorService')).includes(want), `signature differs:\n${want}`);
  assert.match(svc, /providedIn: 'root'/);
});

test('the load error: the docs quote the element and the text the component writes', () => {
  const s = sec('Load error');
  assert.match(COMPONENT, /message\.className = 'prose-error'/);
  assert.match(COMPONENT, /setAttribute\('role', 'alert'\)/);
  const text = /The ODRL prose renderer could not be loaded \(\$\{this\.wasmUrl\}\)\. ([^`]+)\./.exec(COMPONENT)[1];
  assert.ok(s.includes(text), `docs lack the sentence "${text}"`);
  assert.ok(s.includes('prose-error') && s.includes('role="alert"'));
});

test('the public exports of index.ts are exactly those the interface page lists', () => {
  const index = source('prose-angular/src/index.ts');
  const names = [];
  for (const m of index.matchAll(/export (?:type )?\{([^}]*)\}/g)) names.push(...m[1].split(',').map((s) => s.trim()).filter(Boolean));
  const rows = tableRows(sec('Exports'));
  assert.deepEqual(rows.map((r) => tick(col(r, 0))).sort(), [...names].sort());
  for (const hidden of ['loadWasm', 'createWasmLoader', 'validationOptions', 'profileTexts', 'createChangeTracker', 'parseReport', 'StandaloneReporter', 'createProfileTracker', 'parseSummary']) {
    assert.ok(!names.includes(hidden), `${hidden} is exported now: document it`);
    assert.ok(!ticks(interfaceMd).includes(hidden), `docs name the internal ${hidden}`);
  }
});

test('ProfileSummary is documented field by field', () => {
  const ps = /export interface ProfileSummary \{([\s\S]*?)\n\}/.exec(source('prose-angular/src/profile-sync.ts'))[1];
  const fields = [...ps.matchAll(/^\s*(\w+):\s*([^;\n]+);?/gm)].map((m) => [m[1], squash(m[2])]);
  assert.ok(fields.length >= 6);
  const rows = tableRows(sec('ProfileSummary'));
  assert.deepEqual(rows.map((r) => tick(col(r, 0))).sort(), fields.map((f) => f[0]).sort());
  for (const [name, type] of fields) assert.equal(squash(tick(col(rows.find((r) => tick(col(r, 0)) === name), 1))), type);
});

test('the wasm asset entry in the docs is the one demo-angular builds with', () => {
  const ng = JSON.parse(source('demo-angular/angular.json'));
  const assets = ng.projects['demo-angular'].architect.build.options.assets;
  const entry = assets.find((a) => a.glob === 'prose_wasm_bg.wasm');
  assert.ok(entry, 'demo-angular/angular.json has no wasm asset entry');
  assert.deepEqual(JSON.parse(content('examples/angular-assets.json')), entry);
  const md = frontMatterless(content('angular/index.md'));
  assert.ok(md.includes('{{file examples/angular-assets.json}}'));
  assert.match(md, /does not copy|does not emit/i);
});

test('every compiled region in docs-examples.ts is shown on an Angular page, and every region shown exists', () => {
  const file = source('prose-angular/test/docs-examples.ts');
  const regions = [...file.matchAll(/^\/\/ region: (\w+)\s*$/gm)].map((m) => m[1]);
  const pages = ['index', 'interface', 'styling', 'validation', 'css-hooks'].map((p) => content(`angular/${p}.md`)).join('\n');
  const used = [...pages.matchAll(/\{\{source prose-angular\/test\/docs-examples\.ts#(\w+)\}\}/g)].map((m) => m[1]);
  for (const r of regions) assert.ok(used.includes(r), `region ${r} is typechecked but not shown`);
  for (const u of used) assert.ok(regions.includes(u), `page shows missing region ${u}`);
});

test('the package facts on the overview page: versions, peer range, client-side only, read-only', () => {
  const pkg = JSON.parse(source('prose-angular/package.json'));
  const md = frontMatterless(content('angular/index.md'));
  assert.ok(md.includes(pkg.name) && md.includes('@ds-labs/prose-wasm'));
  assert.ok(md.includes(pkg.peerDependencies['@angular/core'].replace('>=', '')), 'Angular peer range');
  assert.match(md, /client-side only/i);
  assert.match(md, /no editor|read-only/i);
  const version = pkg.version.split('.').slice(0, 2).join('.');
  assert.ok(md.includes(`@ds-labs/prose-angular@${version}`) || md.includes(`"@ds-labs/prose-angular": "^${version}`) || md.includes(`npm install @ds-labs/prose-angular`));
});
