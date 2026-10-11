// Run with `node --test scripts/test/*.test.mjs`. The CLI's argument handling
// and exit codes, with a fake engine in place of the wasm module; the real one
// is exercised on the packed tarball by scripts/test-cli-tarball.mjs.
import assert from 'node:assert/strict';
import { test } from 'node:test';
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
async function cli(argv, { files = {}, reports = {}, stdin = '', engineError, ctorError } = {}) {
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
          return { validate: (json) => reports[json] ?? report('conforms') };
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
