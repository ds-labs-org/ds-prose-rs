// Run with `npm test`. Node strips the TypeScript types on import.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createProfileTracker, parseSummary } from '../src/profile-sync.ts';

test('no profile on a fresh mount needs no call', () => {
  const changed = createProfileTracker();
  assert.equal(changed(null), false);
  assert.equal(changed(undefined), false);
});

test('a new text is handed over once', () => {
  const changed = createProfileTracker();
  assert.equal(changed('{"a":1}'), true);
  assert.equal(changed('{"a":1}'), false, 'a policy change alone does not re-read the profile');
  assert.equal(changed('{"a":2}'), true);
});

test('going back to no profile clears it', () => {
  const changed = createProfileTracker();
  changed('{"a":1}');
  assert.equal(changed(null), true);
  assert.equal(changed(null), false);
});

test('an empty or blank text means no profile', () => {
  const changed = createProfileTracker();
  assert.equal(changed(''), false);
  assert.equal(changed('  \n'), false);
  changed('{"a":1}');
  assert.equal(changed(''), true);
  assert.equal(changed(null), false);
});

test('each component tracks its own profile', () => {
  const a = createProfileTracker();
  const b = createProfileTracker();
  assert.equal(a('x'), true);
  assert.equal(b('x'), true);
});

test('the summary is parsed; a cleared profile has none', () => {
  const s = parseSummary('{"iri":"https://e.org/p","title":"P","version":null,"terms":2,"counts":{"action":2},"warnings":[]}');
  assert.deepEqual(s, { iri: 'https://e.org/p', title: 'P', version: null, terms: 2, counts: { action: 2 }, warnings: [] });
  assert.equal(parseSummary('null'), null);
});
