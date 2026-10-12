// Run with `node --test scripts/test/*.test.mjs`. The CLI's argument handling
// and exit codes, with a fake engine in place of the wasm module; the real one
// is exercised on the packed tarball by scripts/test-cli-tarball.mjs.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { copyFileSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { after, test } from 'node:test';
import { parseArgs, run } from '../ds-prose-validate.mjs';

const report = (verdict, findings = []) => JSON.stringify({
  schema: 'ds-prose/validation-report@1',
  verdict,
  counts: { error: findings.filter((f) => f.severity === 'error').length, warning: 0, info: 0 },
  gaps: [],
  findings,
});
const finding = (rule, severity = 'error', message = `${rule} happened`, path = 'policy[0]') => ({
  rule, severity, message, target: { kind: 'node', path }, fingerprint: '0123456789abcdef',
});

/** Runs the CLI against in-memory files; `reports` maps a file's text to its report text. */
async function cli(argv, { files = {}, reports = {}, stdin = '', engineError, ctorError, validateError } = {}) {
  const out = [];
  const err = [];
  const calls = [];
  const io = {
    readFile: (p) => {
      if (!(p in files)) throw Object.assign(new Error(`ENOENT: ${p}`), { code: 'ENOENT' });
      return files[p];
    },
    readStdin: () => stdin,
    stdout: (s) => out.push(s),
    stderr: (s) => err.push(s),
    engine: async () => {
      if (engineError) throw new Error(engineError);
      return {
        validator(profiles, options) {
          calls.push({ profiles, options });
          if (ctorError) throw new Error(ctorError(profiles));
          return {
            validate: (json) => {
              if (validateError?.(json)) throw new Error(validateError(json));
              return reports[json] ?? report('conforms');
            },
          };
        },
      };
    },
  };
  const code = await run(argv, io);
  return { code, out: out.join(''), err: err.join(''), calls };
}

test('parseArgs reads flags in both forms and the files after --', () => {
  const a = parseArgs(['--profile', 'p.ttl', '--profile=q.jsonld', '--strict', '--format=json', '--', '-x.json', 'b.json']);
  assert.deepEqual(a.profiles, ['p.ttl', 'q.jsonld']);
  assert.equal(a.strict, true);
  assert.equal(a.format, 'json');
  assert.deepEqual(a.files, ['-x.json', 'b.json']);
});

test('no files is a usage error with the usage text', async () => {
  const r = await cli([]);
  assert.equal(r.code, 2);
  assert.match(r.err, /Usage: ds-prose-validate/);
  assert.equal(r.out, '');
});

test('--help prints the usage and exits 0', async () => {
  const r = await cli(['--help']);
  assert.equal(r.code, 0);
  assert.match(r.out, /Usage: ds-prose-validate/);
  assert.match(r.out, /--allow-incomplete/);
  assert.match(r.out, /exit codes/i);
});

test('usage errors name the problem and exit 2', async () => {
  const bad = [
    [['--nope', 'a.json'], /--nope/],
    [['--format', 'xml', 'a.json'], /xml/],
    [['--odrl', 'all', '--strict', 'a.json'], /all/],
    [['--ruleset', 'x', '--strict', 'a.json'], /ruleset/],
    [['--ruleset', '0', '--strict', 'a.json'], /ruleset/],
    [['--rule', 'profile.unknown-term', '--strict', 'a.json'], /rule/],
    [['--rule', 'x=loud', '--strict', 'a.json'], /loud/],
    [['--profile'], /--profile/],
    [['--structural', 'a.json'], /--strict/],
    [['--fail-closed', 'a.json'], /--strict/],
    [['--deny-deprecated', 'a.json'], /--strict/],
  ];
  for (const [argv, re] of bad) {
    const r = await cli(argv, { files: { 'a.json': '{}' } });
    assert.equal(r.code, 2, argv.join(' '));
    assert.match(r.err, re, argv.join(' '));
    assert.equal(r.calls.length, 0, 'nothing is validated after a usage error');
  }
});

test('flags become the options object the wasm module reads', async () => {
  let r = await cli(['a.json'], { files: { 'a.json': '{}' } });
  assert.deepEqual(r.calls[0].options, { strict: false });
  r = await cli(['--strict', 'a.json'], { files: { 'a.json': '{}' } });
  assert.deepEqual(r.calls[0].options, { strict: true });
  r = await cli(
    ['--strict', '--structural', '--fail-closed', '--deny-deprecated', '--odrl', 'core', '--ruleset', '1',
      '--rule', 'profile.unknown-term=warning', '--rule=prose.unit-not-allowed=off', 'a.json'],
    { files: { 'a.json': '{}' } },
  );
  assert.deepEqual(r.calls[0].options, {
    strict: true, structural: true, failClosed: true, denyDeprecated: true, odrl: 'core', ruleset: 1,
    rules: { 'profile.unknown-term': 'warning', 'prose.unit-not-allowed': 'off' },
  });
});

test('profiles are read once, in order, and handed over as text', async () => {
  const r = await cli(['-p', 'p.ttl', '--profile', 'q.jsonld', 'a.json', 'b.json'], {
    files: { 'p.ttl': 'TTL', 'q.jsonld': 'LD', 'a.json': 'A', 'b.json': 'B' },
  });
  assert.equal(r.calls.length, 1, 'one validator for every file');
  assert.deepEqual(r.calls[0].profiles, ['TTL', 'LD']);
});

test('exit 0 when every file conforms', async () => {
  const r = await cli(['--strict', 'a.json', 'b.json'], { files: { 'a.json': 'A', 'b.json': 'B' } });
  assert.equal(r.code, 0);
});

test('exit 1 on violations, whatever else is in the run', async () => {
  const reports = { B: report('violations', [finding('profile.unknown-term')]), C: report('incomplete') };
  const files = { 'a.json': 'A', 'b.json': 'B', 'c.json': 'C' };
  assert.equal((await cli(['--strict', 'a.json', 'b.json'], { files, reports })).code, 1);
  assert.equal((await cli(['--strict', 'c.json', 'b.json'], { files, reports })).code, 1);
  assert.equal((await cli(['--strict', '--allow-incomplete', 'b.json'], { files, reports })).code, 1);
});

test('exit 3 when incomplete; --allow-incomplete makes it 0', async () => {
  const reports = { C: report('incomplete') };
  const files = { 'c.json': 'C', 'a.json': 'A' };
  assert.equal((await cli(['--strict', 'c.json', 'a.json'], { files, reports })).code, 3);
  assert.equal((await cli(['--strict', '--allow-incomplete', 'c.json', 'a.json'], { files, reports })).code, 0);
});

test('exit 2 for a file that cannot be read, after checking the others', async () => {
  const reports = { B: report('violations', [finding('profile.unknown-term')]) };
  const r = await cli(['--strict', 'missing.json', 'b.json'], { files: { 'b.json': 'B' }, reports });
  assert.equal(r.code, 2, 'unreadable outranks violations');
  assert.match(r.err, /missing\.json/);
  assert.match(r.out, /b\.json/, 'the other file is still reported');
});

test('exit 2 for a policy the validator reports as unreadable', async () => {
  const reports = { X: report('violations', [finding('input.unreadable')]) };
  const r = await cli(['--strict', 'x.json'], { files: { 'x.json': 'X' }, reports });
  assert.equal(r.code, 2);
  assert.match(r.out, /input\.unreadable/);
});

test('exit 2 for a profile that cannot be read, naming the file', async () => {
  let r = await cli(['--profile', 'nope.ttl', 'a.json'], { files: { 'a.json': 'A' } });
  assert.equal(r.code, 2);
  assert.match(r.err, /nope\.ttl/);
  r = await cli(['--profile', 'p.ttl', '--profile', 'q.ttl', 'a.json'], {
    files: { 'p.ttl': 'P', 'q.ttl': 'Q', 'a.json': 'A' },
    ctorError: () => 'profile 2: this is an HTML page',
  });
  assert.equal(r.code, 2);
  assert.match(r.err, /q\.ttl: this is an HTML page/);
  assert.doesNotMatch(r.err, /profile 2/);
});

test('exit 2 when the wasm module cannot be loaded', async () => {
  const r = await cli(['a.json'], { files: { 'a.json': 'A' }, engineError: 'no wasm' });
  assert.equal(r.code, 2);
  assert.match(r.err, /no wasm/);
});

test('text output gives the verdict and each finding', async () => {
  const reports = { B: report('violations', [finding('profile.unknown-term', 'error', 'ex:x is not defined', 'policy[0].permission[0].action[0]#name')]) };
  const r = await cli(['--strict', 'a.json', 'b.json'], { files: { 'a.json': 'A', 'b.json': 'B' }, reports });
  assert.match(r.out, /a\.json: conforms/);
  assert.match(r.out, /b\.json: violations \(1 error/);
  assert.match(r.out, /error\s+profile\.unknown-term\s+policy\[0\]\.permission\[0\]\.action\[0\]#name\s+ex:x is not defined/);
});

test('json output is the reports, unchanged, per file', async () => {
  const reports = { B: report('violations', [finding('profile.unknown-term')]) };
  const r = await cli(['--strict', '--format', 'json', 'a.json', 'b.json', 'gone.json'], {
    files: { 'a.json': 'A', 'b.json': 'B' }, reports,
  });
  const out = JSON.parse(r.out);
  assert.deepEqual(out.map((x) => x.file), ['a.json', 'b.json', 'gone.json']);
  assert.equal(JSON.stringify(out[1].report), reports.B);
  assert.equal(out[0].error, null);
  assert.equal(out[2].report, null);
  assert.match(out[2].error, /gone\.json/);
  assert.equal(r.code, 2);
});

test('- reads the policy from standard input', async () => {
  const reports = { STDIN: report('incomplete') };
  const r = await cli(['--strict', '-'], { stdin: 'STDIN', reports });
  assert.equal(r.code, 3);
  assert.match(r.out, /<stdin>: incomplete/);
});

test('an engine that fails on one policy is exit 2, not a crash, and the rest are still checked', async () => {
  const r = await cli(['--strict', 'a.json', 'b.json'], {
    files: { 'a.json': 'A', 'b.json': 'B' },
    validateError: (json) => (json === 'A' ? 'unreachable executed' : ''),
    reports: { B: report('violations', [finding('profile.unknown-term')]) },
  });
  assert.equal(r.code, 2);
  assert.match(r.err, /a\.json/);
  assert.match(r.err, /unreachable executed/);
  assert.match(r.out, /b\.json: violations/);
});

test('a report that is not JSON is exit 2 and names the file', async () => {
  const r = await cli(['--strict', 'a.json'], { files: { 'a.json': 'A' }, reports: { A: 'not json' } });
  assert.equal(r.code, 2);
  assert.match(r.err, /a\.json/);
});

test('--rule with an id that is an Object.prototype key is passed on, not dropped', async () => {
  const r = await cli(['--strict', '--rule', '__proto__=off', 'a.json'], { files: { 'a.json': 'A' } });
  assert.deepEqual(Object.keys(r.calls[0].options.rules), ['__proto__']);
});

test('--rule and --ruleset are passed on without --strict: the engine honours levels in the advisory reading too', async () => {
  let r = await cli(['--rule', 'profile.unknown-term=error', 'a.json'], { files: { 'a.json': '{}' } });
  assert.equal(r.code, 0);
  assert.deepEqual(r.calls[0].options, { strict: false, rules: { 'profile.unknown-term': 'error' } });
  r = await cli(['--ruleset', '1', 'a.json'], { files: { 'a.json': '{}' } });
  assert.deepEqual(r.calls[0].options, { strict: false, ruleset: 1 });
});

test('text output escapes control characters from the policy, so it cannot forge lines or move the cursor', async () => {
  const hostile = 'the action "x" is fine\nother.json: conforms\n\u001b[2K  info z\u0085' + String.fromCharCode(0x2028) + '\u007f';
  const reports = { B: report('violations', [finding('profile.unknown-term', 'error', hostile, 'policy[0]\nfake#name')]) };
  const r = await cli(['--strict', 'b\u001b[31m.json'], { files: { 'b\u001b[31m.json': 'B' }, reports });
  const control = new RegExp('[\\u0000-\\u001f\\u007f-\\u009f\\u2028\\u2029]');
  assert.doesNotMatch(r.out.replace(/\n/g, ''), control, 'only the report\'s own newlines remain');
  assert.equal(r.out.split('\n').length, 3, 'a header, one finding line, and the final newline');
  assert.match(r.out, /\\n/);
  assert.match(r.out, /\\u\{1b\}\[2K/);
  assert.match(r.out, /b\\u\{1b\}\[31m\.json: violations/);
  // The json format keeps the text as the engine wrote it (JSON escapes it).
  const j = await cli(['--strict', '--format', 'json', 'b.json'], { files: { 'b.json': 'B' }, reports });
  assert.equal(JSON.parse(j.out)[0].report.findings[0].message, hostile);
});

test('ordinary non-ASCII text is printed as it is', async () => {
  const reports = { B: report('violations', [finding('profile.unknown-term', 'error', 'le terme été ✓ 日本')]) };
  const r = await cli(['--strict', 'b.json'], { files: { 'b.json': 'B' }, reports });
  assert.match(r.out, /le terme été ✓ 日本/);
});

// The command as a process, with a stub in place of the wasm module (a copy
// of the script next to a prose_wasm.js that reads every policy as conforming
// unless it contains "VIOLATION"), for what only a real process shows: how
// bytes are decoded, what a closed pipe does, what an uncaught error exits with.
const scratch = mkdtempSync(join(tmpdir(), 'ds-prose-cli-test-'));
after(() => rmSync(scratch, { recursive: true, force: true }));
copyFileSync(new URL('../ds-prose-validate.mjs', import.meta.url), join(scratch, 'ds-prose-validate.mjs'));
writeFileSync(join(scratch, 'prose_wasm_bg.wasm'), '');
writeFileSync(join(scratch, 'prose_wasm.js'), `
export function initSync() {}
export class ProseValidator {
  validate(json) {
    const bad = json.includes('VIOLATION');
    // Findings that are not a list: a report the CLI cannot print, a fault of the engine.
    const findings = json.includes('MALFORMED') ? 5 : [];
    return JSON.stringify({ verdict: bad ? 'violations' : 'conforms', counts: { error: 0, warning: 0, info: 0 }, findings, seen: json });
  }
  free() {}
}
`);
const bin = join(scratch, 'ds-prose-validate.mjs');
const file = (name, bytes) => {
  const p = join(scratch, name);
  writeFileSync(p, bytes);
  return p;
};
const proc = (args, input) => {
  const r = spawnSync(process.execPath, [bin, ...args], { input, encoding: 'utf8' });
  return { code: r.status, out: r.stdout, err: r.stderr };
};

test('a policy that is not valid UTF-8 is unreadable input (exit 2), not validated with replacement characters', () => {
  const bad = file('bad8.json', Buffer.concat([Buffer.from('{"uid":"urn:p'), Buffer.from([0xff, 0xfe]), Buffer.from('"}')]));
  const r = proc(['--strict', '--format', 'json', bad]);
  assert.equal(r.code, 2, r.out + r.err);
  assert.match(r.err, /bad8\.json/);
  assert.match(r.err, /utf-?8/i);
  assert.equal(JSON.parse(r.out)[0].report, null);
  const viaStdin = proc(['--strict', '-'], Buffer.from([0x7b, 0xc3, 0x28, 0x7d]));
  assert.equal(viaStdin.code, 2);
  assert.match(viaStdin.err, /<stdin>/);
});

test('a UTF-8 byte order mark is not part of the policy; a profile may carry one too', () => {
  const bom = Buffer.from([0xef, 0xbb, 0xbf]);
  const ok = file('bom.json', Buffer.concat([bom, Buffer.from('{"uid":"urn:p"}')]));
  const prof = file('bom-profile.jsonld', Buffer.concat([bom, Buffer.from('{}')]));
  const r = proc(['--strict', '--format', 'json', '--profile', prof, ok]);
  assert.equal(r.code, 0, r.out + r.err);
  assert.equal(JSON.parse(r.out)[0].report.seen, '{"uid":"urn:p"}');
});

test('a closed output pipe does not turn a pass into the violations exit code', async () => {
  const ok = file('ok.json', '{"uid":"urn:ok"}');
  const child = spawn(process.execPath, [bin, '--strict', ...Array(5000).fill(ok)], { stdio: ['ignore', 'pipe', 'pipe'] });
  let err = '';
  child.stderr.on('data', (d) => { err += d; });
  child.stdout.once('data', () => child.stdout.destroy());
  const code = await new Promise((resolve) => child.on('close', resolve));
  assert.notEqual(code, 1, `exit 1 means violations: ${err}`);
  assert.equal(code, 0, err);
  assert.doesNotMatch(err, /EPIPE|Unhandled/);
});

test('an uncaught error is exit 2 with a message, never the violations code', () => {
  const odd = file('odd.json', '{"uid":"MALFORMED"}');
  const r = spawnSync(process.execPath, [bin, '--strict', odd], { encoding: 'utf8' });
  assert.equal(r.status, 2, r.stderr);
  assert.match(r.stderr, /^ds-prose-validate: .*not a function/m);
  assert.doesNotMatch(r.stderr, /node:internal|at .*\.mjs/, 'a message, not a stack trace');
});

test('- more than once is a usage error: standard input can be read once', async () => {
  for (const argv of [['--strict', '-', '-'], ['--strict', '-', 'a.json', '--', '-']]) {
    const r = await cli(argv, { stdin: 'STDIN', files: { 'a.json': 'A' } });
    assert.equal(r.code, 2, argv.join(' '));
    assert.match(r.err, /standard input/);
    assert.equal(r.calls.length, 0);
    assert.equal(r.out, '');
  }
});
