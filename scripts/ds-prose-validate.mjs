#!/usr/bin/env node
// ds-prose-validate: validates ODRL policies against ODRL profiles, with the
// same engine as the Rust crates and the Angular wrapper (the wasm module in
// this package), and exits with a code a CI job can act on.
//
//   ds-prose-validate --profile p.ttl [--profile ...] [--strict] files...
//
// Run with --help for the options and the exit codes. The argument handling
// and exit codes are tested with a fake engine in scripts/test/cli.test.mjs,
// and with the real one on the packed tarball by scripts/test-cli-tarball.mjs.
import { readFileSync, realpathSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const USAGE = `Usage: ds-prose-validate [options] <policy.json>...

Validates ODRL policies (JSON-LD) against ODRL profiles (JSON-LD or Turtle).
A policy of "-" is read from standard input (once).
Input must be UTF-8; a leading byte order mark is ignored.

Options:
  -p, --profile <file>    Load a profile; repeat for several.
      --strict            Read the vocabulary as closed: ODRL plus the loaded
                          profiles is every term there is. Without it, the
                          advisory reading: nothing is an error.
      --structural        With --strict: also check the Information Model's
                          structural MUSTs (odrl.* rules).
      --fail-closed       With --strict: what cannot be checked is an error,
                          not a gap.
      --deny-deprecated   With --strict: a deprecated term is an error.
      --odrl <common|core>
                          With --strict: the ODRL terms defined without a
                          profile (default: common, Core plus Common).
      --ruleset <n>       Check only the rules of ruleset n and earlier.
                          Works with or without --strict.
      --rule <id>=<level> Set a rule's level: off, info, warning or error.
                          Repeatable. Works with or without --strict: in the
                          advisory reading, a rule set to error can fail.
      --format <text|json>
                          text (default), or a JSON array with one
                          {file, report, error} per policy, each report
                          unchanged (ds-prose/validation-report@1).
      --allow-incomplete  Exit 0, not 3, when nothing is wrong but something
                          could not be checked (e.g. a declared profile is
                          not loaded).
  -h, --help              This text.

Exit codes:
  0  every policy conforms
  1  at least one policy has violations
  2  usage error, or a policy or profile that cannot be read
  3  no violations, but incomplete (see --allow-incomplete)
When several apply, 2 outranks 1, and 1 outranks 3.
`;

const LEVELS = ['off', 'info', 'warning', 'error'];

class UsageError extends Error {}

/** Reads the command line. Throws a UsageError naming what is wrong. */
export function parseArgs(argv) {
  const a = {
    profiles: [], strict: false, structural: false, failClosed: false, denyDeprecated: false,
    odrl: undefined, ruleset: undefined, rules: undefined, format: 'text',
    allowIncomplete: false, help: false, files: [],
  };
  const flags = {
    '--strict': 'strict', '--structural': 'structural', '--fail-closed': 'failClosed',
    '--deny-deprecated': 'denyDeprecated', '--allow-incomplete': 'allowIncomplete',
    '--help': 'help', '-h': 'help',
  };
  const valued = new Set(['--profile', '-p', '--format', '--odrl', '--ruleset', '--rule']);
  for (let i = 0; i < argv.length; i++) {
    let arg = argv[i];
    if (arg === '--') {
      a.files.push(...argv.slice(i + 1));
      break;
    }
    if (arg === '-' || !arg.startsWith('-')) {
      a.files.push(arg);
      continue;
    }
    let value;
    const eq = arg.startsWith('--') ? arg.indexOf('=') : -1;
    if (eq > 0) {
      value = arg.slice(eq + 1);
      arg = arg.slice(0, eq);
    }
    if (arg in flags) {
      if (value !== undefined) throw new UsageError(`${arg} takes no value`);
      a[flags[arg]] = true;
    } else if (valued.has(arg)) {
      if (value === undefined) {
        if (i + 1 >= argv.length) throw new UsageError(`${arg} needs a value`);
        value = argv[++i];
      }
      setValued(a, arg, value);
    } else {
      throw new UsageError(`unknown option ${arg}`);
    }
  }
  if (a.files.filter((f) => f === '-').length > 1) throw new UsageError('standard input ("-") can be given once: it is read to the end the first time');
  if (!a.strict) {
    for (const [flag, on] of [['--structural', a.structural], ['--fail-closed', a.failClosed], ['--deny-deprecated', a.denyDeprecated], ['--odrl', a.odrl !== undefined]]) {
      if (on) throw new UsageError(`${flag} needs --strict`);
    }
  }
  return a;
}

function setValued(a, arg, value) {
  switch (arg) {
    case '--profile':
    case '-p':
      a.profiles.push(value);
      break;
    case '--format':
      if (value !== 'text' && value !== 'json') throw new UsageError(`--format must be text or json, not "${value}"`);
      a.format = value;
      break;
    case '--odrl':
      if (value !== 'common' && value !== 'core') throw new UsageError(`--odrl must be common or core, not "${value}"`);
      a.odrl = value;
      break;
    case '--ruleset': {
      const n = /^\d+$/.test(value) ? Number(value) : 0;
      if (n < 1) throw new UsageError(`--ruleset must be a whole number from 1, not "${value}"`);
      a.ruleset = n;
      break;
    }
    case '--rule': {
      const eq = value.indexOf('=');
      if (eq < 1) throw new UsageError(`--rule needs <id>=<level>, not "${value}"`);
      const level = value.slice(eq + 1);
      if (!LEVELS.includes(level)) throw new UsageError(`--rule level must be one of ${LEVELS.join(', ')}, not "${level}"`);
      // Defined, not assigned, so a key such as __proto__ is a rule id like
      // any other (the engine then says no rule is called that).
      a.rules ??= {};
      Object.defineProperty(a.rules, value.slice(0, eq), { value: level, enumerable: true, writable: true, configurable: true });
      break;
    }
  }
}

/** The options object `ValidateOptions` the wasm module reads. */
function optionsOf(a) {
  // Rule levels and the ruleset apply to the advisory reading too: the engine
  // honours them there, so they are passed on, not dropped.
  const o = { strict: a.strict };
  if (!a.strict) {
    if (a.ruleset) o.ruleset = a.ruleset;
    if (a.rules) o.rules = a.rules;
    return o;
  }
  if (a.structural) o.structural = true;
  if (a.failClosed) o.failClosed = true;
  if (a.denyDeprecated) o.denyDeprecated = true;
  if (a.odrl) o.odrl = a.odrl;
  if (a.ruleset) o.ruleset = a.ruleset;
  if (a.rules) o.rules = a.rules;
  return o;
}

// Control characters, and the characters that reorder or end lines, in text a
// policy supplies (a term, an IRI, a file name): printed as escapes, so a
// hostile policy cannot forge report lines or move the terminal's cursor.
const UNSAFE = /[\u0000-\u001f\u007f-\u009f\u061c\u200b-\u200f\u2028-\u202e\u2066-\u2069]/g;
const SHORT = { '\n': '\\n', '\r': '\\r', '\t': '\\t' };
const safe = (s) => String(s).replace(UNSAFE, (c) => SHORT[c] ?? `\\u{${c.charCodeAt(0).toString(16)}}`);

const plural = (n, word) => `${n} ${word}${n === 1 ? '' : 's'}`;

function text(file, r) {
  const c = r.counts ?? { error: 0, warning: 0, info: 0 };
  const total = c.error + c.warning + c.info;
  let out = `${safe(file)}: ${r.verdict}`;
  if (total > 0) out += ` (${plural(c.error, 'error')}, ${plural(c.warning, 'warning')}, ${plural(c.info, 'info')})`;
  out += '\n';
  for (const f of r.findings ?? []) {
    out += `  ${safe(f.severity).padEnd(7)} ${safe(f.rule)}  ${safe(f.target?.path || '(document)')}  ${safe(f.message)}\n`;
  }
  return out;
}

/** The exit code one report earns: 0, 1, 2 or 3 (before --allow-incomplete). */
function codeOf(r) {
  if ((r.findings ?? []).some((f) => f.rule === 'input.unreadable')) return 2;
  switch (r.verdict) {
    case 'conforms': return 0;
    case 'violations': return 1;
    case 'incomplete': return 3;
    default: return 2;
  }
}

/** 2 outranks 1, which outranks 3. */
const RANK = { 0: 0, 3: 1, 1: 2, 2: 3 };

/** The wasm module of this package, loaded for Node. */
async function loadEngine() {
  const here = dirname(fileURLToPath(import.meta.url));
  const glue = await import(pathToFileURL(join(here, 'prose_wasm.js')).href);
  glue.initSync({ module: readFileSync(join(here, 'prose_wasm_bg.wasm')) });
  return {
    validator(profiles, options) {
      const v = new glue.ProseValidator(profiles, options);
      return { validate: (json) => v.validate(json) };
    },
  };
}

// Strict UTF-8, as JSON requires: bytes that are not UTF-8 are an error, not
// U+FFFD in a policy that is then certified. A leading byte order mark is
// dropped (the decoder's default), as JSON parsers are allowed to.
const utf8 = new TextDecoder('utf-8', { fatal: true });
const decode = (bytes) => {
  try {
    return utf8.decode(bytes);
  } catch {
    throw new Error('not valid UTF-8');
  }
};

const defaultIo = () => ({
  readFile: (p) => decode(readFileSync(p)),
  readStdin: () => decode(readFileSync(0)),
  stdout: (s) => process.stdout.write(s),
  stderr: (s) => process.stderr.write(s),
  engine: loadEngine,
});

/** Runs the CLI; resolves to the exit code. `io` is for tests. */
export async function run(argv, io = defaultIo()) {
  let a;
  try {
    a = parseArgs(argv);
  } catch (e) {
    if (!(e instanceof UsageError)) throw e;
    io.stderr(`ds-prose-validate: ${e.message}\n\n${USAGE}`);
    return 2;
  }
  if (a.help) {
    io.stdout(USAGE);
    return 0;
  }
  if (a.files.length === 0) {
    io.stderr(`ds-prose-validate: no policy given\n\n${USAGE}`);
    return 2;
  }
  const profiles = [];
  for (const p of a.profiles) {
    try {
      profiles.push(io.readFile(p));
    } catch (e) {
      io.stderr(`ds-prose-validate: cannot read the profile ${p}: ${e.message}\n`);
      return 2;
    }
  }
  let validator;
  try {
    validator = (await io.engine()).validator(profiles, optionsOf(a));
  } catch (e) {
    // The wasm module numbers the profile that failed; say which file it is.
    const message = String(e?.message ?? e).replace(/^profile (\d+): /, (m, n) => (a.profiles[n - 1] ? `${a.profiles[n - 1]}: ` : m));
    io.stderr(`ds-prose-validate: ${message}\n`);
    return 2;
  }
  let exit = 0;
  const results = [];
  for (const file of a.files) {
    const name = file === '-' ? '<stdin>' : file;
    let body;
    try {
      body = file === '-' ? io.readStdin() : io.readFile(file);
    } catch (e) {
      const error = `cannot read ${name}: ${e.message}`;
      io.stderr(`ds-prose-validate: ${safe(error)}\n`);
      results.push({ file: name, report: null, error });
      exit = 2;
      continue;
    }
    let raw;
    let report;
    try {
      raw = validator.validate(body);
      report = JSON.parse(raw);
    } catch (e) {
      // The engine never throws on a policy; if it does, that is a fault in
      // the tool, which must not look like violations (exit 1).
      const error = `cannot check ${name}: ${e?.message ?? e}`;
      io.stderr(`ds-prose-validate: ${safe(error)}\n`);
      results.push({ file: name, report: null, error });
      exit = 2;
      continue;
    }
    let code = codeOf(report);
    if (code === 3 && a.allowIncomplete) code = 0;
    if (RANK[code] > RANK[exit]) exit = code;
    results.push({ file: name, report, error: null, raw });
    if (a.format === 'text') io.stdout(text(name, report));
  }
  if (a.format === 'json') {
    // The reports go out as the validator wrote them, not re-serialised.
    const items = results.map((r) => `{"file":${JSON.stringify(r.file)},"report":${r.raw ?? 'null'},"error":${JSON.stringify(r.error)}}`);
    io.stdout(`[${items.join(',')}]\n`);
  }
  validator.free?.();
  return exit;
}

// Run when started as a command (npm links the bin, so compare real paths).
let entry = '';
try {
  entry = process.argv[1] ? realpathSync(process.argv[1]) : '';
} catch {
  // Not started from a file (e.g. node -e): nothing to run.
}
if (entry === fileURLToPath(import.meta.url)) {
  // Node's own exit code for an uncaught error is 1, which this command
  // defines as "violations": a fault of the tool must never look like that.
  // A reader that closes the pipe early (`| head`) is not a fault: the
  // verdict is still the one computed. Any other failure to write is.
  let outputFailed = false;
  process.stdout.on('error', (e) => {
    if (e?.code !== 'EPIPE') outputFailed = true;
  });
  let code;
  try {
    code = await run(process.argv.slice(2));
  } catch (e) {
    process.stderr.write(`ds-prose-validate: ${safe(e?.message ?? e)}\n`);
    code = 2;
  }
  process.exitCode = code;
  // A write error is reported after run() returns (on a later tick, or once a
  // pending write fails), so it is read when the process is about to exit.
  process.once('exit', () => {
    if (outputFailed) process.exitCode = 2;
  });
}
