// Run with `npm test`. Node strips the TypeScript types on import.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  createChangeTracker, parseReport, profileTexts, reportChanged, validationOptions,
} from '../src/validation-sync.ts';

const finding = (rule, fingerprint, severity = 'error') => ({ rule, fingerprint, severity, message: `${rule} said so` });
const report = (verdict, findings = []) => ({ schema: 'ds-prose/validation-report@1', verdict, findings });

test('the first report is a change; no report to none is not', () => {
  assert.equal(reportChanged(null, null), false);
  assert.equal(reportChanged(undefined, null), false);
  assert.equal(reportChanged(null, report('conforms')), true);
  assert.equal(reportChanged(undefined, report('conforms')), true);
});

test('a report that goes away is a change', () => {
  assert.equal(reportChanged(report('conforms'), null), true);
});

test('the same verdict and findings is no change, in any order, whatever the wording', () => {
  const a = report('violations', [finding('profile.unknown-term', 'aaaa'), finding('prose.unit-not-allowed', 'bbbb')]);
  const b = report('violations', [
    { ...finding('prose.unit-not-allowed', 'bbbb'), message: 'worded differently' },
    finding('profile.unknown-term', 'aaaa'),
  ]);
  assert.equal(reportChanged(a, b), false);
  assert.equal(reportChanged(b, a), false);
  assert.equal(reportChanged(a, a), false);
});

test('a different verdict is a change', () => {
  assert.equal(reportChanged(report('conforms'), report('incomplete')), true);
});

test('a finding that appears, goes or is replaced is a change', () => {
  const one = report('violations', [finding('profile.unknown-term', 'aaaa')]);
  const two = report('violations', [finding('profile.unknown-term', 'aaaa'), finding('profile.wrong-kind', 'bbbb')]);
  const other = report('violations', [finding('profile.unknown-term', 'cccc')]);
  assert.equal(reportChanged(one, two), true);
  assert.equal(reportChanged(two, one), true);
  assert.equal(reportChanged(one, other), true);
});

test('a finding that gets louder or quieter is a change', () => {
  const warn = report('conforms', [finding('profile.deprecated-term', 'aaaa', 'warning')]);
  const err = report('violations', [finding('profile.deprecated-term', 'aaaa', 'error')]);
  assert.equal(reportChanged(warn, { ...warn, findings: err.findings }), true, 'same verdict, louder finding');
  assert.equal(reportChanged(warn, err), true);
});

test('reports with no findings array compare as empty', () => {
  assert.equal(reportChanged({ verdict: 'conforms' }, { verdict: 'conforms', findings: [] }), false);
});

test('strict alone is strict options; validation takes precedence; neither is off', () => {
  assert.equal(validationOptions(false, null), null);
  assert.equal(validationOptions(false, undefined), null);
  assert.deepEqual(validationOptions(true, null), { strict: true });
  assert.deepEqual(validationOptions(false, { structural: true }), { structural: true });
  assert.deepEqual(validationOptions(true, { strict: false }), { strict: false });
  assert.deepEqual(validationOptions(true, {}), {}, 'an options object is used as given, even when empty');
});

test('options are copied, not shared with the host', () => {
  const given = { rules: { 'prose.unit-not-allowed': 'off' } };
  const got = validationOptions(false, given);
  assert.deepEqual(got, given);
  assert.notEqual(got, given);
});

test('profiles: the list wins over the single profile; blank means none', () => {
  assert.equal(profileTexts(null, null), null);
  assert.equal(profileTexts('', undefined), null);
  assert.equal(profileTexts('  \n', null), null);
  assert.deepEqual(profileTexts('{"a":1}', null), ['{"a":1}']);
  assert.deepEqual(profileTexts('{"a":1}', ['x', 'y']), ['x', 'y']);
  assert.deepEqual(profileTexts(null, []), [], 'an empty list clears: it is not "no list"');
  assert.deepEqual(profileTexts(null, ['x', '', '  ', 'y']), ['x', 'y'], 'blank texts are dropped');
});

test('a change tracker fires once per distinct value, starting from none', () => {
  const changed = createChangeTracker();
  assert.equal(changed(null), false, 'a fresh mount has none');
  assert.equal(changed({ strict: true }), true);
  assert.equal(changed({ strict: true }), false, 'an equal value in a new object is no change');
  assert.equal(changed({ strict: false }), true);
  assert.equal(changed(null), true);
  assert.equal(changed(null), false);
  assert.equal(changed(['a']), true);
  assert.equal(changed(['a']), false);
  assert.equal(changed([]), true);
});

test('each tracker keeps its own state', () => {
  const a = createChangeTracker();
  const b = createChangeTracker();
  assert.equal(a(['x']), true);
  assert.equal(b(['x']), true);
});

test('a report is parsed from its text; null stays null', () => {
  assert.equal(parseReport(null), null);
  assert.equal(parseReport(undefined), null);
  assert.deepEqual(parseReport('{"verdict":"conforms","findings":[]}'), { verdict: 'conforms', findings: [] });
});
