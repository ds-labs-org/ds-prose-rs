// Helpers for the contract tests: read the Rust and TypeScript sources and the
// docs markdown, and compare what they say. No compiler, no dependencies.
import fs from 'node:fs';
import path from 'node:path';
import { SITE_DIR } from './helpers.mjs';

export const REPO = path.resolve(SITE_DIR, '..');
export const source = (rel) => fs.readFileSync(path.join(REPO, rel), 'utf8');
export const content = (rel) => fs.readFileSync(path.join(SITE_DIR, 'content', rel), 'utf8');
export const squash = (s) => s.replace(/\s+/g, ' ').replace(/\s*([<>(),:;])\s*/g, '$1').replace(/,(?=[)>\]}])/g, '').trim();

export function walkFiles(dir, pred) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = path.join(dir, e.name);
    return e.isDirectory() ? walkFiles(p, pred) : pred(p) ? [p] : [];
  });
}

// --- Rust ---------------------------------------------------------------------

const stripRustComments = (s) => s.replace(/\/\/[^\n]*/g, '');

/** The text between the braces that open at `from` (the index of `{`). */
function braced(text, from) {
  const t = text;
  let depth = 0;
  for (let i = from; i < t.length; i++) {
    if (t[i] === '"') {
      i++;
      while (i < t.length && t[i] !== '"') i += t[i] === '\\' ? 2 : 1;
    } else if (t[i] === '{') depth++;
    else if (t[i] === '}' && --depth === 0) return t.slice(from + 1, i);
  }
  throw new Error('unbalanced braces');
}

/** Split at commas that are not inside (), [], {} or <>. */
export function topLevel(body, sep = ',') {
  const parts = [];
  let depth = 0;
  let cur = '';
  for (let i = 0; i < body.length; i++) {
    const c = body[i];
    if (c === '"') {
      let j = i + 1;
      while (j < body.length && body[j] !== '"') j += body[j] === '\\' ? 2 : 1;
      cur += body.slice(i, j + 1);
      i = j;
      continue;
    }
    if ('([{<'.includes(c) && !(c === '<' && body[i - 1] === ' ')) depth++;
    if (')]}>'.includes(c) && !(c === '>' && body[i - 1] === '-')) depth--;
    if (c === sep && depth === 0) {
      parts.push(cur);
      cur = '';
    } else cur += c;
  }
  if (cur.trim()) parts.push(cur);
  return parts.map((p) => p.trim()).filter(Boolean);
}

export function structFields(text, name) {
  const clean = stripRustComments(text);
  const at = clean.search(new RegExp(`pub struct ${name}\\b`));
  if (at < 0) throw new Error(`no struct ${name}`);
  const body = braced(clean, clean.indexOf('{', at));
  return topLevel(body).map((chunk) => {
    const attrs = [...chunk.matchAll(/#\[([^\]]*(?:\([^)]*\))?[^\]]*)\]/g)].map((m) => m[1]);
    const rest = chunk.replace(/#\[[^\]]*\]/g, '').trim();
    const m = /^pub\s+(\w+)\s*:\s*([\s\S]+)$/.exec(rest);
    if (!m) throw new Error(`cannot read field in ${name}: ${chunk}`);
    return { name: m[1], type: squash(m[2]), attrs };
  });
}

export function enumVariants(text, name) {
  const clean = stripRustComments(text);
  const at = clean.search(new RegExp(`pub enum ${name}\\b`));
  if (at < 0) throw new Error(`no enum ${name}`);
  const body = braced(clean, clean.indexOf('{', at));
  return topLevel(body).map((c) => /(\w+)\s*(?:[({=][\s\S]*)?$/.exec(c.replace(/#\[[^\]]*\]/g, '').trim())[1]);
}

/** field -> expression text of `impl Default for T { ... T { field: expr, ... } }`. */
export function defaultExprs(text, name) {
  const clean = stripRustComments(text);
  const at = clean.indexOf(`impl Default for ${name}`);
  if (at < 0) throw new Error(`no Default for ${name}`);
  const lit = clean.indexOf(`${name} {`, clean.indexOf('fn default', at));
  const body = braced(clean, clean.indexOf('{', lit));
  const out = {};
  for (const part of topLevel(body)) {
    const m = /^(\w+)\s*:\s*([\s\S]+)$/.exec(part);
    if (m) out[m[1]] = squash(m[2]);
  }
  return out;
}

/** Fields and string defaults of `text_struct!( Name { field: "default", ... } )`. */
export function textStruct(text, name) {
  const clean = stripRustComments(text);
  const at = clean.search(new RegExp(`\\n\\s+${name} \\{`));
  if (at < 0) throw new Error(`no text_struct ${name}`);
  const body = braced(clean, clean.indexOf('{', at));
  return topLevel(body).map((p) => /^(\w+)\s*:\s*"((?:\\.|[^"\\])*)"$/.exec(p)).map((m) => ({ name: m[1], default: m[2] }));
}

/** EmptyTexts field -> its default text, through prose-core's EmptyText::default_text. */
export function emptyTexts(config, segments) {
  const lit = defaultExprs(config, 'EmptyTexts');
  const texts = Object.fromEntries([...stripRustComments(segments).matchAll(/EmptyText::(\w+) => "((?:\\.|[^"\\])*)"/g)].map((m) => [m[1], m[2]]));
  return Object.entries(lit).map(([name, expr]) => ({ name, default: texts[/EmptyText::(\w+)/.exec(expr)[1]] }));
}

/** Idents listed in `macro_name!( a, b, ... )`. */
export function macroIdents(text, macro) {
  const clean = stripRustComments(text);
  const at = clean.indexOf(`${macro}!(`);
  const end = clean.indexOf(');', at);
  return clean.slice(at + macro.length + 2, end).split(',').map((s) => s.trim()).filter(Boolean);
}

/** The signature `pub fn name(...) -> T` as compact text. */
export function fnSignature(text, name) {
  const clean = stripRustComments(text);
  const at = clean.search(new RegExp(`pub fn ${name}\\b`));
  if (at < 0) throw new Error(`no fn ${name}`);
  let i = clean.indexOf('(', at);
  const open = i;
  for (let depth = 0; i < clean.length; i++) {
    if (clean[i] === '(') depth++;
    else if (clean[i] === ')' && --depth === 0) break;
  }
  const args = clean.slice(open + 1, i);
  const ret = /^\s*->\s*([^{]+?)\s*\{/.exec(clean.slice(i + 1))[1];
  return squash(`pub fn ${name}(${args}) -> ${ret}`);
}

/** Every string literal in a Rust file, as raw text. */
export function stringLiterals(text) {
  return [...text.matchAll(/"((?:\\.|[^"\\\n])*)"/g)].map((m) => m[1]);
}

/** CSS hook class names that appear as whitespace-separated words in string literals. */
export function hookClasses(text) {
  const out = new Set();
  for (const lit of stringLiterals(text)) for (const w of lit.split(/\s+/)) if (/^(ds-prose|prose-[a-z][a-z-]*[a-z])$/.test(w)) out.add(w);
  return out;
}

// --- Markdown -----------------------------------------------------------------

export function frontMatterless(md) {
  return md.replace(/^---\n[\s\S]*?\n---\n?/, '');
}

/** Map of `## heading` text -> its body (up to the next heading of the same or higher level). */
export function sections(md, level = 2) {
  const out = new Map();
  const re = new RegExp(`^${'#'.repeat(level)} (.+)$`, 'gm');
  const marks = [...md.matchAll(re)];
  marks.forEach((m, i) => {
    const end = i + 1 < marks.length ? marks[i + 1].index : md.length;
    const next = md.slice(m.index + m[0].length, end);
    out.set(m[1].trim(), next);
  });
  return out;
}

export function tableRows(md) {
  return md
    .split('\n')
    .filter((l) => /^\|/.test(l) && !/^\|\s*:?-{2,}/.test(l))
    .slice(1) // header
    .map((l) => l.replace(/^\||\|\s*$/g, '').split(/(?<!\\)\|/).map((c) => c.trim().replace(/\\\|/g, '|')));
}

export const tick = (cell) => {
  const m = /`([^`]+)`/.exec(cell);
  return m ? m[1] : null;
};

export const ticks = (md) => [...md.matchAll(/`([^`\n]+)`/g)].map((m) => m[1]);
