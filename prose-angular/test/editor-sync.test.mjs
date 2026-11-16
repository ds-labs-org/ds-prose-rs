// Run with `npm test`. The decisions of ds-odrl-prose-editor (mount, update
// or skip, which inputs to hand over, what a change does), against a fake handle.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { EditorSession } from '../src/editor-sync.ts';

const A = '{"@type":"Set","uid":"a"}';
const B = '{"@type":"Set","uid":"b"}';

/** A handle that records its calls; `bad` makes the constructor or `update` throw for that text. */
function fake(bad = new Set()) {
  const calls = [];
  const made = [];
  const mount = (json, cls, onChange) => {
    calls.push(['new', json, cls]);
    if (bad.has(json)) throw new Error(`unreadable ${json}`);
    const handle = {
      onChange,
      update: (j, c) => {
        calls.push(['update', j, c]);
        if (bad.has(j)) throw new Error(`unreadable ${j}`);
      },
      setProfile: (t) => (calls.push(['setProfile', t]), t == null ? 'null' : '{"name":"P","terms":1}'),
      setProfiles: (t) => (calls.push(['setProfiles', t]), '[]'),
      setLimits: (l) => calls.push(['setLimits', l]),
      setValidation: (o) => calls.push(['setValidation', o]),
      report: () => (calls.push(['report']), null),
      destroy: () => calls.push(['destroy']),
      free: () => calls.push(['free']),
    };
    made.push(handle);
    return handle;
  };
  return { calls, made, mount };
}

const inputs = (over = {}) => ({
  json: A, cls: null, single: true, texts: null, options: null, limits: null, ...over,
});
const names = (calls) => calls.map((c) => c[0]);

test('the first apply mounts with the json and the class, and hands the inputs over once', () => {
  const s = new EditorSession();
  const f = fake();
  const out = s.apply(inputs({ cls: 'x', limits: { policies: { max: 1 } }, options: { strict: false } }), f.mount);
  assert.deepEqual(f.calls[0], ['new', A, 'x']);
  assert.deepEqual(names(f.calls), ['new', 'setLimits', 'setValidation', 'report']);
  assert.equal(out.jsonError, undefined);
  // The same inputs again: nothing is handed over twice.
  f.calls.length = 0;
  s.apply(inputs({ cls: 'x', limits: { policies: { max: 1 } }, options: { strict: false } }), f.mount);
  assert.deepEqual(names(f.calls), ['report']);
});

test('the echo of an emitted text does not reach update(); another text does', () => {
  const s = new EditorSession();
  const f = fake();
  s.apply(inputs(), f.mount);
  f.made[0].onChange(B, null);
  f.calls.length = 0;
  s.apply(inputs({ json: B }), f.mount);
  assert.ok(!names(f.calls).includes('update'), 'the echo is skipped');
  s.apply(inputs({ json: A }), f.mount);
  assert.deepEqual(f.calls.find((c) => c[0] === 'update'), ['update', A, null]);
});

test('a class change alone updates the handle', () => {
  const s = new EditorSession();
  const f = fake();
  s.apply(inputs(), f.mount);
  f.calls.length = 0;
  s.apply(inputs({ cls: 'y' }), f.mount);
  assert.deepEqual(f.calls[0], ['update', A, 'y']);
});

test('a failed first mount reports jsonError, mounts nothing and is retried with the trackers unspent', () => {
  const s = new EditorSession();
  const f = fake(new Set([A]));
  const out = s.apply(inputs({ options: { strict: true }, texts: ['p'] }), f.mount);
  assert.match(String(out.jsonError), /unreadable/);
  assert.equal(s.handle, undefined);
  assert.equal(f.made.length, 0);
  assert.deepEqual(names(f.calls), ['new'], 'nothing else was handed over');
  // The next change of an input tries again, with the same profile and options.
  const out2 = s.apply(inputs({ json: B, options: { strict: true }, texts: ['p'] }), f.mount);
  assert.equal(out2.jsonError, undefined);
  assert.equal(s.handle, f.made[0]);
  assert.ok(names(f.calls).includes('setProfile'));
  assert.ok(names(f.calls).includes('setValidation'));
});

test('a later bad json reports jsonError and the document stays', () => {
  const s = new EditorSession();
  const f = fake(new Set(['bad']));
  s.apply(inputs(), f.mount);
  const out = s.apply(inputs({ json: 'bad' }), f.mount);
  assert.match(String(out.jsonError), /unreadable bad/);
  assert.equal(s.lastJson, A);
  // A good text after it is taken.
  f.calls.length = 0;
  s.apply(inputs({ json: B }), f.mount);
  assert.deepEqual(f.calls[0], ['update', B, null]);
});

test('a profile is loaded for single and for several, and its failure is reported', () => {
  const s = new EditorSession();
  const f = fake();
  const out = s.apply(inputs({ texts: ['p1'] }), f.mount);
  assert.deepEqual(out.summaries, [{ name: 'P', terms: 1 }]);
  const out2 = s.apply(inputs({ single: false, texts: ['p1', 'p2'] }), f.mount);
  assert.deepEqual(f.calls.find((c) => c[0] === 'setProfiles'), ['setProfiles', ['p1', 'p2']]);
  assert.deepEqual(out2.summaries, []);
  f.made[0].setProfile = () => {
    throw new Error('no profile');
  };
  const out3 = s.apply(inputs({ texts: ['broken'] }), f.mount);
  assert.match(String(out3.profileError), /no profile/);
});

test('onChange keeps the text, then publishes the report, then emits the text', () => {
  const order = [];
  const s = new EditorSession({
    publish: (r) => order.push(['publish', r?.verdict ?? null, s.lastJson]),
    emit: (t) => order.push(['emit', t, s.lastJson]),
  });
  const f = fake();
  s.apply(inputs(), f.mount);
  const report = JSON.stringify({ schema: 'ds-prose/validation-report@1', verdict: 'conforms', findings: [] });
  f.made[0].onChange(B, report);
  assert.deepEqual(order, [['publish', 'conforms', B], ['emit', B, B]]);
  f.made[0].onChange(A, null);
  assert.deepEqual(order.slice(2), [['publish', null, A], ['emit', A, A]]);
});

test('destroy releases the handle once; a destroy before the apply mounts nothing', () => {
  const s = new EditorSession();
  const f = fake();
  s.apply(inputs(), f.mount);
  s.destroy();
  s.destroy();
  assert.deepEqual(names(f.calls).filter((n) => n === 'destroy' || n === 'free'), ['destroy', 'free']);
  assert.equal(s.handle, undefined);

  const late = new EditorSession();
  const g = fake();
  late.destroy(); // the component went away while the wasm was loading
  assert.equal(late.apply(inputs(), g.mount), null);
  assert.equal(g.made.length, 0);
});
