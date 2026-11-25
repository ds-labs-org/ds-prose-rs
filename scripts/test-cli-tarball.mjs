#!/usr/bin/env node
// Installs the packed @ds-labs/prose-wasm tarball into a scratch directory and
// runs the `ds-prose-validate` command it installs, the way a CI job would:
// every exit code (0 conforms, 1 violations, 2 unreadable or usage, 3
// incomplete), the real wasm engine, the bin link npm makes.
//
//   node scripts/test-cli-tarball.mjs        (after scripts/build-npm-packages.sh)
//
// TARBALLS_DIR overrides where the tarball is looked for.
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const dir = process.env.TARBALLS_DIR ?? join(root, 'dist-npm', 'tarballs');
const tarball = readdirSync(dir).find((f) => /^ds-labs-prose-wasm-.*\.tgz$/.test(f));
assert.ok(tarball, `no prose-wasm tarball in ${dir}`);

const tests = join(root, 'prose-core', 'tests');
const profile = (name) => join(tests, 'profiles', name);
const policy = (name) => join(tests, 'strict', name);

const scratch = mkdtempSync(join(tmpdir(), 'ds-prose-cli-'));
let failures = 0;
try {
  // A package with no dependencies installs without the network.
  writeFileSync(join(scratch, 'package.json'), '{"private":true}');
  execFileSync('npm', ['install', '--no-audit', '--no-fund', '--ignore-scripts', join(dir, tarball)], { cwd: scratch, stdio: 'pipe' });
  const bin = join(scratch, 'node_modules', '.bin', 'ds-prose-validate');
  const garbage = join(scratch, 'not-a-policy.txt');
  writeFileSync(garbage, 'this is not JSON');
  const html = join(scratch, 'landing.html');
  writeFileSync(html, '<!doctype html><p>a landing page</p>');

  // The command is run through the link npm made, as `npx` or a script does.
  const cli = (...args) => {
    const r = spawnSync(bin, args, { cwd: scratch, encoding: 'utf8' });
    return { code: r.status, out: r.stdout, err: r.stderr };
  };
  const check = (name, args, code, then) => {
    try {
      const r = cli(...args);
      assert.equal(r.code, code, `exit ${r.code}, wanted ${code}\n${r.out}${r.err}`);
      then?.(r);
      console.log(`ok   ${name}`);
    } catch (e) {
      failures++;
      console.log(`FAIL ${name}\n${String(e.message).replace(/^/gm, '     ')}`);
    }
  };

  const research = ['--profile', profile('research.jsonld'), '--strict'];
  check('0: a policy that conforms', [...research, policy('clean.json')], 0, (r) => assert.match(r.out, /clean\.json: conforms/));
  check('0: several policies, two profiles', ['-p', profile('research.ttl'), '-p', profile('other.jsonld'), '--strict', policy('mixed-two-profiles.json'), policy('clean.json')], 0);
  check('0: advisory mode never fails a policy with foreign terms', [policy('foreign-term.json')], 0);
  check('1: a policy with violations', ['--profile', profile('research-closed.jsonld'), '--strict', policy('violating.json')], 1, (r) => {
    assert.match(r.out, /violations \(\d+ errors?/);
    assert.match(r.out, /prose\.operator-not-allowed|profile\.unknown-term/);
  });
  check('1: --structural finds what the Information Model requires', ['--strict', '--structural', policy('structural-bad.json')], 1, (r) => assert.match(r.out, /odrl\./));
  check('1: violations are not softened by --allow-incomplete', ['--allow-incomplete', ...research, policy('foreign-term.json')], 1);
  check('3: incomplete, a declared profile is not loaded', [...research, policy('not-loaded.json')], 3, (r) => assert.match(r.out, /not-loaded\.json: incomplete/));
  check('3: strict with no profile cannot certify a profiled policy', ['--strict', policy('clean.json')], 3);
  check('0: --allow-incomplete accepts incomplete', ['--allow-incomplete', ...research, policy('not-loaded.json')], 0);
  check('2: no arguments', [], 2, (r) => assert.match(r.err, /Usage/));
  check('2: unknown option', ['--frobnicate', policy('clean.json')], 2, (r) => assert.match(r.err, /--frobnicate/));
  check('2: a policy file that is missing', [...research, join(scratch, 'missing.json')], 2, (r) => assert.match(r.err, /missing\.json/));
  check('2: a policy that is not JSON', [...research, garbage], 2, (r) => assert.match(r.out, /input\.unreadable/));
  check('2: a profile file that is missing', ['--profile', join(scratch, 'missing.ttl'), policy('clean.json')], 2, (r) => assert.match(r.err, /missing\.ttl/));
  check('2: a profile that is a web page, named by file', ['--profile', profile('research.jsonld'), '--profile', html, '--strict', policy('clean.json')], 2, (r) => assert.match(r.err, /landing\.html/));
  check('2: unreadable outranks violations and incomplete', [...research, policy('foreign-term.json'), policy('not-loaded.json'), garbage], 2);
  check('--format json: one {file, report, error} per policy', [...research, '--format', 'json', policy('clean.json'), policy('not-loaded.json')], 3, (r) => {
    const out = JSON.parse(r.out);
    assert.deepEqual(out.map((x) => x.report.verdict), ['conforms', 'incomplete']);
    assert.equal(out[0].report.schema, 'ds-prose/validation-report@1');
    assert.equal(out[0].error, null);
  });
  const stdin = spawnSync(bin, [...research, '-'], { cwd: scratch, encoding: 'utf8', input: execFileSync('cat', [policy('clean.json')]) });
  try {
    assert.equal(stdin.status, 0, stdin.stdout + stdin.stderr);
    assert.match(stdin.stdout, /<stdin>: conforms/);
    console.log('ok   - reads standard input');
  } catch (e) {
    failures++;
    console.log(`FAIL - reads standard input\n${String(e.message).replace(/^/gm, '     ')}`);
  }
} finally {
  rmSync(scratch, { recursive: true, force: true });
}
if (failures) {
  console.error(`\n${failures} CLI check(s) failed`);
  process.exit(1);
}
console.log('cli check ok');
