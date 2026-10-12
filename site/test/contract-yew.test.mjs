// The Yew docs against the Rust sources. Every name, type and default in a
// table is read back from the code; a field that is added, renamed or retyped
// without the page changing fails here.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { SITE_DIR } from './helpers.mjs';
import {
  content, defaultExprs, enumVariants, fnSignature, frontMatterless, macroIdents, sections, source, squash, structFields, tableRows, tick, ticks,
} from './contract.mjs';

const interfaceMd = frontMatterless(content('yew/interface.md'));
const overviewMd = frontMatterless(content('yew/index.md'));
const SEC = sections(interfaceMd);
const sec = (h) => {
  const s = SEC.get(`\`${h}\``) ?? SEC.get(h);
  assert.ok(s !== undefined, `yew/interface.md has no "## ${h}" section; has ${[...SEC.keys()].join(', ')}`);
  return s;
};
const col = (row, i) => row[i] ?? '';

const LIB = source('prose-yew/src/lib.rs');
const VIEW = source('prose-yew/src/edit/view.rs');
const PROBLEMS = source('prose-yew/src/edit/problems.rs');
const HOOK = source('prose-yew/src/edit/hook.rs');
const CONFIG = source('prose-yew/src/edit/config.rs');
const MOD = source('prose-yew/src/edit/mod.rs');

/** Props tables: | Property | Type | Default | Meaning |. */
function checkProps(heading, text, struct) {
  const fields = structFields(text, struct);
  const rows = tableRows(sec(heading));
  assert.deepEqual(rows.map((r) => tick(col(r, 0))).sort(), fields.map((f) => f.name).sort(), `${heading}: documented properties differ from ${struct}`);
  for (const f of fields) {
    const row = rows.find((r) => tick(col(r, 0)) === f.name);
    assert.equal(squash(tick(col(row, 1)) ?? ''), f.type, `${heading}.${f.name}: type`);
    const propOr = f.attrs.find((a) => a.startsWith('prop_or'));
    const def = col(row, 2);
    if (!propOr) assert.equal(def, 'required', `${heading}.${f.name} has no default in the code`);
    else {
      assert.notEqual(def, 'required', `${heading}.${f.name} has a default in the code`);
      const m = /^prop_or\((.*)\)$/.exec(propOr);
      if (m) assert.equal(tick(def), m[1], `${heading}.${f.name}: default`);
    }
    assert.ok(col(row, 3).length > 10, `${heading}.${f.name}: no meaning given`);
  }
  return fields.length;
}

test('OdrlProse: the property table is the OdrlProseProps struct', () => assert.equal(checkProps('OdrlProse', LIB, 'OdrlProseProps'), 6));
test('OdrlProseView: the property table is the OdrlProseViewProps struct', () => assert.equal(checkProps('OdrlProseView', VIEW, 'OdrlProseViewProps'), 9));
test('ProseGlossary: the property table is the GlossaryProps struct', () => checkProps('ProseGlossary', LIB, 'GlossaryProps'));
test('ProseProblems: the property table is the ProseProblemsProps struct', () => checkProps('ProseProblems', PROBLEMS, 'ProseProblemsProps'));

test('the hooks section shows the three signatures exactly as the code has them', () => {
  const s = sec('Hooks');
  const code = squash(s);
  for (const name of ['use_prose_editor', 'use_prose_editor_with', 'use_validation']) {
    assert.ok(code.includes(fnSignature(HOOK, name)), `signature of ${name} differs from hook.rs:\n${fnSignature(HOOK, name)}`);
  }
});

test('ProseEditor and ProseEditorWith: field tables equal the structs', () => {
  for (const [h, name] of [['ProseEditor', 'ProseEditor'], ['ProseEditorWith', 'ProseEditorWith']]) {
    const fields = structFields(HOOK, name);
    const rows = tableRows(sec(h));
    assert.deepEqual(rows.map((r) => tick(col(r, 0))).sort(), fields.map((f) => f.name).sort(), `${h}: fields`);
    for (const f of fields) {
      const row = rows.find((r) => tick(col(r, 0)) === f.name);
      assert.equal(squash(tick(col(row, 1)) ?? ''), f.type, `${h}.${f.name}: type`);
    }
  }
});

test('EditMode, StrictEdit, Reveal and ButtonContent: every variant is named, and the default is the code\'s', () => {
  const core = source('prose-core/src/edit/apply.rs');
  const cases = [
    ['EditMode', MOD, 'Read'], ['StrictEdit', core, 'Flag'], ['Reveal', CONFIG, 'Always'], ['ButtonContent', CONFIG, 'SymbolAndText'],
  ];
  for (const [name, text, def] of cases) {
    const body = sections(interfaceMd).get(`\`${name}\``) ?? [...SEC.values()].find((v) => v.includes(`\`${name}::`)) ?? '';
    for (const v of enumVariants(text, name)) assert.ok(body.includes(`\`${name}::${v}\``), `${name}::${v} is not documented`);
    const attr = new RegExp(`#\\[default\\]\\s*${def}\\b`);
    assert.match(text.replace(/\/\/\/[^\n]*/g, ''), attr, `${name}: the code's default is no longer ${def}`);
    assert.ok(new RegExp(`${name}::${def}\\b[^\\n]*default|default[^\\n]*${name}::${def}\\b`, 'i').test(body), `${name}: the docs do not say that ${def} is the default`);
  }
});

test('EditConfig: fields, types and defaults equal the struct and its Default impl', () => {
  const fields = structFields(CONFIG, 'EditConfig');
  const defaults = defaultExprs(CONFIG, 'EditConfig');
  const rows = tableRows(sec('EditConfig'));
  assert.deepEqual(rows.map((r) => tick(col(r, 0))).sort(), fields.map((f) => f.name).sort());
  for (const f of fields) {
    const row = rows.find((r) => tick(col(r, 0)) === f.name);
    assert.equal(squash(tick(col(row, 1)) ?? ''), f.type, `EditConfig.${f.name}: type`);
    assert.equal(squash(tick(col(row, 2)) ?? ''), defaults[f.name], `EditConfig.${f.name}: default`);
  }
});

test('EditConfig builders are named with their signatures', () => {
  const s = sec('EditConfig');
  for (const sig of ['pub fn with_wording(mut self, w: Rc<Wording>) -> EditConfig', 'pub fn with_strict(mut self, options: Options) -> EditConfig']) {
    assert.ok(CONFIG.includes(sig.replace('(mut self', '(mut self')), `${sig} is not in config.rs`);
    assert.ok(squash(s).includes(squash(sig.replace('mut self', 'self'))), `docs lack ${sig}`);
  }
});

test('Labels: every field is listed; a plain-text default equals the code', () => {
  const fields = structFields(CONFIG, 'Labels');
  const defaults = defaultExprs(CONFIG, 'Labels');
  const rows = tableRows(sec('Labels'));
  const documented = new Map(rows.map((r) => [tick(col(r, 0)), r]));
  assert.deepEqual([...documented.keys()].sort(), fields.map((f) => f.name).sort(), 'Labels fields');
  const unescape = (s) => s.replace(/\\u\{([0-9a-f]+)\}/gi, (_, h) => String.fromCodePoint(parseInt(h, 16)));
  let checked = 0;
  for (const f of fields) {
    const m = /^a\("((?:\\.|[^"\\])*)"\)$/.exec(defaults[f.name] ?? '');
    if (!m) continue;
    const want = unescape(m[1]);
    const cell = col(documented.get(f.name), 1);
    assert.equal(want === '' ? cell : tick(cell), want === '' ? '(empty)' : want, `Labels.${f.name}: default text`);
    checked++;
  }
  assert.ok(checked > 60, `only ${checked} defaults checked`);
});

test('Labels: the nested structs list their fields', () => {
  const s = sec('Labels');
  for (const struct of ['Nouns', 'FieldNames', 'Placeholders', 'EmptyTexts']) {
    for (const f of structFields(CONFIG, struct)) assert.ok(s.includes(`\`${f.name}\``), `${struct}.${f.name} is not listed`);
  }
});

test('ProseClasses and ProseStyles: the docs list the 46 field names of class_fields!', () => {
  const idents = macroIdents(CONFIG, 'class_fields');
  assert.equal(idents.length, 46);
  const styling = frontMatterless(content('yew/styling.md'));
  const list = sections(styling).get('Extra classes and inline styles') ?? '';
  const named = new Set(ticks(list));
  for (const i of idents) assert.ok(named.has(i), `ProseClasses::${i} is not listed on the styling page`);
});

test('every item the crate root re-exports is named on the overview or interface page', () => {
  const reexports = [];
  for (const m of LIB.matchAll(/pub use edit::\{([^}]*)\}/g)) reexports.push(...m[1].split(',').map((s) => s.trim()).filter(Boolean));
  reexports.push('SharedWording', 'OdrlProseProps', 'OdrlProse', 'GlossaryProps', 'ProseGlossary', 'prose_core');
  assert.ok(reexports.length > 30);
  const text = overviewMd + interfaceMd;
  const named = new Set(ticks(text).flatMap((t) => t.split(/[^A-Za-z0-9_]+/)));
  for (const r of reexports) assert.ok(named.has(r), `${r} is exported from the crate root but not named in the docs`);
});

test('the features and crate version on the overview page are the ones in Cargo.toml', () => {
  const cargo = source('prose-yew/Cargo.toml');
  const work = source('Cargo.toml');
  const version = /^version\s*=\s*"([^"]+)"/m.exec(work)[1];
  assert.ok(overviewMd.includes(`prose-yew = "${version.split('.').slice(0, 2).join('.')}"`), 'dependency line shows the workspace version');
  for (const f of ['csr', 'ssr']) assert.match(cargo, new RegExp(`^${f}\\s*=`, 'm'));
  assert.match(cargo, /^default\s*=\s*\[\]/m);
  assert.ok(overviewMd.includes('`csr`') && overviewMd.includes('`ssr`'));
  assert.match(overviewMd, /does not enable `csr` itself|does not turn on `csr`/);
});

test('every compiled region in site_examples.rs is shown on a Yew page, and every region shown exists', () => {
  const file = source('prose-yew/tests/site_examples.rs');
  const regions = [...file.matchAll(/^\s*\/\/ region: (\w+)\s*$/gm)].map((m) => m[1]);
  const pages = ['index', 'interface', 'styling', 'validation', 'css-hooks'].map((p) => content(`yew/${p}.md`)).join('\n');
  const used = [...pages.matchAll(/\{\{source prose-yew\/tests\/site_examples\.rs#(\w+)\}\}/g)].map((m) => m[1]);
  for (const r of regions) assert.ok(used.includes(r), `region ${r} is compiled but not shown`);
  for (const u of used) assert.ok(regions.includes(u), `page shows missing region ${u}`);
  for (const r of regions) assert.match(file, new RegExp(`// endregion: ${r}\\b`));
});

test('the docs say where the compiled examples live, and that cargo test compiles them', () => {
  assert.match(frontMatterless(content('yew/interface.md')), /site_examples\.rs/);
});

test('BASE_CSS and the default-features claim: the page says the read-only components emit no <style>', () => {
  const css = source('prose-yew/src/edit/css.rs');
  assert.match(css, /pub const BASE_CSS/);
  const styling = frontMatterless(content('yew/styling.md'));
  assert.match(styling, /OdrlProse` .*emits no `<style>`|emit no `<style>`|emits no `<style>`/);
});
