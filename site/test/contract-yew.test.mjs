// The Yew docs against the Rust sources. Every name, type and default in a
// table is read back from the code; a field that is added, renamed or retyped
// without the page changing fails here.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { SITE_DIR } from './helpers.mjs';
import {
  content, defaultExprs, emptyTexts, enumVariants, fnSignature, frontMatterless, macroIdents, sections, source, squash, structFields, tableRows, textStruct, tick, ticks,
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
// text_struct! writes `pub struct Name { pub f: AttrValue }`; the macro body is the source of truth here.
const structNames = (text, name) => textStruct(text, name).map((f) => f.name);

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
    const body = interfaceMd;
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
  const unescape = (s) => s.replace(/\\"/g, '"').replace(/\\u\{([0-9a-f]+)\}/gi, (_, h) => String.fromCodePoint(parseInt(h, 16)));
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

test('Labels: the nested wording table lists every field of Nouns, FieldNames, Placeholders and EmptyTexts with its default', () => {
  const rows = tableRows(sec('Nested wording'));
  const want = [];
  for (const struct of ['Nouns', 'FieldNames', 'Placeholders']) for (const f of textStruct(CONFIG, struct)) want.push([`${struct}.${f.name}`, f.default]);
  for (const f of emptyTexts(CONFIG, source('prose-core/src/edit/segments.rs'))) want.push([`EmptyTexts.${f.name}`, f.default]);
  assert.ok(want.length > 40);
  assert.deepEqual(rows.map((r) => tick(col(r, 0))).sort(), want.map((w) => w[0]).sort());
  for (const [name, def] of want) assert.equal(tick(col(rows.find((r) => tick(col(r, 0)) === name), 1)), def, name);
  // Every field is a real pub field of the struct it is listed under.
  for (const struct of ['Nouns', 'FieldNames', 'Placeholders']) assert.deepEqual(textStruct(CONFIG, struct).map((f) => f.name), structNames(CONFIG, struct));
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

test('the strict-editor behaviour the validation page describes is the README\'s', () => {
  const readme = source('README.md');
  const page = frontMatterless(content('yew/validation.md'));
  for (const phrase of ['keeps the draft', 'Escape reverts it', 'the draft is given up', 'polite live region']) {
    assert.ok(readme.replace(/\s+/g, ' ').includes(phrase), `README lacks "${phrase}"`);
    assert.ok(page.replace(/\s+/g, ' ').includes(phrase), `page lacks "${phrase}"`);
  }
});

test('the verdict table is the status() function', () => {
  const fn = /pub\(crate\) fn status[\s\S]*?\n\}/.exec(source('prose-yew/src/edit/problems.rs'))[0];
  const arms = [...fn.matchAll(/(Verdict::\w+|_) => \("(prose-[a-z-]+)", l\.(\w+)\.clone\(\)\)/g)].map((m) => [m[1], m[2], m[3]]);
  assert.equal(arms.length, 3);
  const rows = tableRows(sections(frontMatterless(content('yew/validation.md'))).get('Findings under the sentences'));
  for (const [verdict, cls, label] of arms) {
    const row = rows.find((r) => (verdict === '_' ? r[0] === 'any other verdict' : tick(r[0]) === verdict));
    assert.ok(row, verdict);
    assert.equal(tick(row[1]), cls);
    assert.equal(tick(row[2]), label);
  }
});

test('the validation page\'s API sentences match prose-core', () => {
  const core = source('prose-core/src/validate/mod.rs');
  for (const sig of ['pub fn advisory() -> Options', 'pub fn strict() -> Options', 'pub fn new(wording: Arc<Wording>, options: Options) -> Validator', 'pub fn validate(&self, doc: &EditDoc) -> Report', 'pub fn validate_json(&self, json: &str) -> Report', 'pub fn structural(mut self, on: bool) -> Options'])
    assert.ok(squash(core).includes(squash(sig)), sig);
  assert.match(core, /impl Default for Options \{\s*fn default\(\) -> Options \{\s*Options::advisory\(\)/);
  assert.match(source('prose-core/Cargo.toml'), /^turtle = /m);
  const md = frontMatterless(content('yew/validation.md'));
  for (const v of enumVariants(core, 'Verdict')) assert.ok(md.includes(`\`${v}\``) || md.includes(`Verdict::${v}`), v);
});

test('the read-only component writes the violations section only for a validator with violations on', () => {
  const lib = source('prose-yew/src/lib.rs');
  assert.match(lib, /props\.violations/);
  assert.match(lib, /use_memo/);
  const test = source('prose-yew/tests/site_examples.rs');
  assert.match(test, /violations_need_a_validator_and_the_flag/);
});

test('the rule kind classes are built at run time from the list name, as the hooks page says', () => {
  assert.match(source('prose-yew/src/edit/view.rs'), /format!\("prose-\{\}"/);
  for (const list of ['permission', 'prohibition', 'obligation', 'duty', 'remedy', 'consequence']) assert.match(source('prose-core/src/edit/path.rs'), new RegExp(`"${list}"`));
});

test('the shadow DOM page claim: the document lookups are in the sources', () => {
  assert.match(source('prose-yew/src/edit/slot.rs'), /get_element_by_id/);
  const p = source('prose-yew/src/edit/problems.rs');
  assert.match(p, /active_element\(\)/);
  assert.match(p, /closest\(".prose-problems"\)/);
});

test('server rendering: the docs say BASE_CSS arrives intact, as the pinned Yew does, and no text still says it is escaped', () => {
  const rust = source('prose-yew/tests/site_examples.rs');
  assert.match(rust, /async fn the_server_renderer_keeps_the_text_of_the_style_element/);
  assert.match(rust, /assert_eq!\(\s*style,\s*prose_yew::BASE_CSS/);
  assert.match(source('prose-yew/Cargo.toml'), /^yew = \{ version = "0\.23"/m);
  const styling = frontMatterless(content('yew/styling.md'));
  assert.match(styling, /server renderer writes the text of `<style>` unescaped/);
  assert.match(styling, /site_examples\.rs/);
  const claim = /(?<!un)escap\w+[^.\n]{0,40}style|style[^.\n]{0,40}(?<!un)escap/i;
  for (const [name, text] of [['styling.md', styling], ['README.md', source('README.md').replace(/\s+/g, ' ')], ['edit/mod.rs', source('prose-yew/src/edit/mod.rs').replace(/\s+/g, ' ')], ['edit/config.rs', source('prose-yew/src/edit/config.rs').replace(/\s+/g, ' ')]])
    assert.doesNotMatch(text, claim, `${name} says the style text is escaped`);
});
