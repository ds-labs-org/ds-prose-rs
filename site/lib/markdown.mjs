// A small markdown renderer with a fixed grammar. Anything outside the grammar
// is escaped, so a docs author cannot inject markup. Dependency-free.
//
// Grammar: ATX headings, paragraphs, fenced code, blockquotes, pipe tables,
// ordered and unordered lists (one nesting level), horizontal rules; inline
// code, strong, emphasis, links. Directives on a line of their own:
//   {{file path}}      embed a file under contentDir as a code block
//   {{include name}}   render contentDir/_partials/name.md in place
import fs from 'node:fs';
import path from 'node:path';

export function escapeHtml(s) {
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

export function parseFrontMatter(src) {
  const m = /^---\n([\s\S]*?)\n---\n?/.exec(src);
  if (!m) return { data: {}, body: src };
  const data = {};
  for (const line of m[1].split('\n')) {
    const kv = /^([A-Za-z][\w-]*):\s*(.*)$/.exec(line);
    if (kv) data[kv[1]] = kv[2].trim();
  }
  return { data, body: src.slice(m[0].length) };
}

export function slugify(text) {
  return (
    text
      .toLowerCase()
      .normalize('NFKD')
      .replace(/[̀-ͯ]/g, '')
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '') || 'section'
  );
}

const SAFE_URL = /^(?:https?:|mailto:|\/|#|\.\.?\/|[A-Za-z0-9_-][^:]*$)/;

function inline(src) {
  const codes = [];
  let s = src.replace(/(`+)([\s\S]*?[^`])\1(?!`)/g, (_, _t, body) => {
    if (body.length > 2 && body.startsWith(' ') && body.endsWith(' ')) body = body.slice(1, -1);
    codes.push(`<code>${escapeHtml(body)}</code>`);
    return `\u0000${codes.length - 1}\u0000`;
  });
  s = escapeHtml(s);
  s = s.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (all, text, url) => {
    const raw = url.replace(/&amp;/g, '&');
    return SAFE_URL.test(raw) ? `<a href="${url}">${text}</a>` : text;
  });
  s = s.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
  s = s.replace(/(^|[^*\w])\*([^*\s][^*]*)\*(?![*\w])/g, '$1<em>$2</em>');
  s = s.replace(/(^|[^\w])_([^_\s][^_]*)_(?![\w])/g, '$1<em>$2</em>');
  return s.replace(/\u0000(\d+)\u0000/g, (_, i) => codes[Number(i)]);
}

function plainText(src) {
  return src
    .replace(/`+([^`]+)`+/g, '$1')
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/[*_]/g, '')
    .trim();
}

function splitRow(line) {
  let t = line.trim();
  if (t.startsWith('|')) t = t.slice(1);
  if (t.endsWith('|') && !t.endsWith('\\|')) t = t.slice(0, -1);
  const cells = [];
  let cur = '';
  for (let i = 0; i < t.length; i++) {
    if (t[i] === '\\' && t[i + 1] === '|') {
      cur += '|';
      i++;
    } else if (t[i] === '|') {
      cells.push(cur.trim());
      cur = '';
    } else cur += t[i];
  }
  cells.push(cur.trim());
  return cells;
}

const isTableSep = (l) => /^\s*\|?\s*:?-{1,}:?\s*(\|\s*:?-{1,}:?\s*)*\|?\s*$/.test(l) && l.includes('-') && l.includes('|');
const LIST_RE = /^(\s*)([-*]|\d+\.)\s+(.*)$/;
const DIRECTIVE_RE = /^\{\{\s*(\w+)\s*([^}]*?)\s*\}\}\s*$/;

function codeBlock(text, lang) {
  const label = lang ? `Code: ${escapeHtml(lang)}` : 'Code';
  const cls = lang ? ` class="language-${escapeHtml(lang)}"` : '';
  return (
    `<div class="code"><button type="button" class="copy" data-copy>Copy</button>` +
    `<pre tabindex="0" role="region" aria-label="${label}"><code${cls}>${escapeHtml(text)}</code></pre></div>\n`
  );
}

function inside(contentDir, rel) {
  if (!contentDir) throw new Error(`directive needs a contentDir (${rel})`);
  const root = path.resolve(contentDir);
  const full = path.resolve(root, rel);
  if (full !== root && !full.startsWith(root + path.sep)) throw new Error(`path outside the content directory: ${rel}`);
  return full;
}

function render(src, ctx) {
  const lines = src.replace(/\r\n?/g, '\n').split('\n');
  let out = '';
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim()) {
      i++;
      continue;
    }
    let m;
    // fenced code
    if ((m = /^(`{3,}|~{3,})\s*([\w+-]*)\s*$/.exec(line))) {
      const fence = m[1];
      const body = [];
      i++;
      while (i < lines.length) {
        const c = /^(`{3,}|~{3,})\s*$/.exec(lines[i]);
        if (c && c[1][0] === fence[0] && c[1].length >= fence.length) break;
        body.push(lines[i++]);
      }
      i++;
      out += codeBlock(body.join('\n') + '\n', m[2]);
      continue;
    }
    // directive
    if ((m = DIRECTIVE_RE.exec(line))) {
      const [, name, arg] = m;
      if (name === 'file') {
        const full = inside(ctx.contentDir, arg);
        if (!fs.existsSync(full)) throw new Error(`{{file}}: no such file: ${arg}`);
        const ext = path.extname(arg).slice(1);
        out += codeBlock(fs.readFileSync(full, 'utf8'), ext);
      } else if (name === 'include') {
        const full = inside(ctx.contentDir, `_partials/${arg}.md`);
        if (!fs.existsSync(full)) throw new Error(`{{include}}: no such partial: ${arg}`);
        out += render(parseFrontMatter(fs.readFileSync(full, 'utf8')).body, ctx);
      } else {
        throw new Error(`unknown directive: ${name}`);
      }
      i++;
      continue;
    }
    // heading
    if ((m = /^(#{1,6})\s+(.+?)\s*#*\s*$/.exec(line))) {
      const level = m[1].length;
      const text = plainText(m[2]);
      let id = slugify(text);
      const n = ctx.ids.get(id) ?? 0;
      ctx.ids.set(id, n + 1);
      if (n) id = `${id}-${n}`;
      ctx.headings.push({ level, id, text });
      const anchor = level === 2 || level === 3 ? ` <a class="anchor" href="#${id}" aria-label="Link to ${escapeHtml(text)}">#</a>` : '';
      out += `<h${level} id="${id}">${inline(m[2])}${anchor}</h${level}>\n`;
      i++;
      continue;
    }
    // horizontal rule
    if (/^\s*([-*_])(\s*\1){2,}\s*$/.test(line)) {
      out += '<hr>\n';
      i++;
      continue;
    }
    // blockquote
    if (/^>\s?/.test(line)) {
      const body = [];
      while (i < lines.length && /^>\s?/.test(lines[i])) body.push(lines[i++].replace(/^>\s?/, ''));
      out += `<blockquote>\n${render(body.join('\n'), ctx)}</blockquote>\n`;
      continue;
    }
    // table
    if (line.includes('|') && i + 1 < lines.length && isTableSep(lines[i + 1])) {
      const head = splitRow(line);
      const aligns = splitRow(lines[i + 1]).map((c) => (/^:-+:$/.test(c) ? 'center' : /^-+:$/.test(c) ? 'right' : ''));
      i += 2;
      const rows = [];
      while (i < lines.length && lines[i].trim() && lines[i].includes('|')) rows.push(splitRow(lines[i++]));
      const cls = (a) => (a ? ` class="align-${a}"` : '');
      out += '<div class="table-scroll" tabindex="0" role="region" aria-label="Table">\n<table>\n<thead><tr>';
      head.forEach((c, k) => (out += `<th scope="col"${cls(aligns[k])}>${inline(c)}</th>`));
      out += '</tr></thead>\n<tbody>\n';
      for (const r of rows) {
        out += '<tr>';
        r.forEach((c, k) => (out += `<td${cls(aligns[k])}>${inline(c)}</td>`));
        out += '</tr>\n';
      }
      out += '</tbody>\n</table>\n</div>\n';
      continue;
    }
    // list
    if ((m = LIST_RE.exec(line)) && m[1].length === 0) {
      const tag = /\d/.test(m[2]) ? 'ol' : 'ul';
      const items = [];
      while (i < lines.length) {
        const l = lines[i];
        const lm = LIST_RE.exec(l);
        if (lm && lm[1].length === 0) {
          items.push({ text: lm[3], sub: [] });
        } else if (lm && items.length) {
          items[items.length - 1].sub.push(lm[3]);
        } else if (/^\s{2,}\S/.test(l) && items.length) {
          const it = items[items.length - 1];
          if (it.sub.length) it.sub[it.sub.length - 1] += ' ' + l.trim();
          else it.text += ' ' + l.trim();
        } else break;
        i++;
      }
      out += `<${tag}>\n`;
      for (const it of items) {
        out += `<li>${inline(it.text)}`;
        if (it.sub.length) out += `\n<ul>\n${it.sub.map((s) => `<li>${inline(s)}</li>\n`).join('')}</ul>\n`;
        out += '</li>\n';
      }
      out += `</${tag}>\n`;
      continue;
    }
    // paragraph
    const para = [];
    while (
      i < lines.length &&
      lines[i].trim() &&
      !/^(#{1,6}\s|`{3,}|~{3,}|>)/.test(lines[i]) &&
      !DIRECTIVE_RE.test(lines[i]) &&
      !(LIST_RE.test(lines[i]) && para.length) &&
      !(lines[i].includes('|') && i + 1 < lines.length && isTableSep(lines[i + 1]))
    )
      para.push(lines[i++].trim());
    if (!para.length) {
      para.push(lines[i++].trim());
    }
    out += `<p>${inline(para.join('\n'))}</p>\n`;
  }
  return out;
}

export function renderMarkdown(src, { contentDir } = {}) {
  const ctx = { contentDir, ids: new Map(), headings: [] };
  const html = render(src, ctx);
  return { html, headings: ctx.headings };
}
