// Run with `npm test`. The decisions of ds-odrl-prose-editor (mount, update
// or skip, which inputs to hand over, what a change does), against a fake handle.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { DATASPACE_PRESET, EditorSession, resolvePreset } from '../src/editor-sync.ts';

const A = '{"@type":"Set","uid":"a"}';
const B = '{"@type":"Set","uid":"b"}';

/** What the wasm handle throws for a document its scope refuses. */
function outOfScope(paths) {
  const e = new Error(`the document holds sections out of scope: ${paths.join(', ')}`);
  e.name = 'OutOfScopeError';
  e.paths = paths;
  return e;
}

/** A handle that records its calls; `bad` makes the constructor or `update` throw for that text. */
function fake(bad = new Set(), scope = {}) {
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
      setLimits: (l) => {
        calls.push(['setLimits', l]);
        if (scope.refuseLimits) throw outOfScope(['permission[0].assignee']);
      },
      setValidation: (o) => calls.push(['setValidation', o]),
      setShapes: (t) => {
        calls.push(['setShapes', t]);
        if (t === 'bad shapes') throw new Error('not shapes');
        return t == null ? 'null' : '{"constraints":2,"actions":null,"leftOperands":null,"warnings":["ignored sh:minCount"]}';
      },
      setRefuseTerms: (on) => calls.push(['setRefuseTerms', on]),
      setTermInput: (m) => {
        calls.push(['setTermInput', m]);
        if (m !== 'free' && m !== 'select') throw new Error(`termInput ${m}`);
      },
      // `scope.refuse` names what `setLimits` / `update` refuse as out of scope; `scope.removed` is what the next load took out.
      setOutOfScope: (m) => {
        calls.push(['setOutOfScope', m]);
        if (scope.refuseMode) throw outOfScope(['assigner']);
      },
      removed: () => {
        calls.push(['removed']);
        const r = scope.removed ?? [];
        scope.removed = [];
        return JSON.stringify(r);
      },
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

test('termInput: free (the default) is never handed over; select is, once, and back to free', () => {
  const s = new EditorSession();
  const f = fake();
  s.apply(inputs(), f.mount);
  s.apply(inputs({ termInput: 'free' }), f.mount);
  assert.ok(!names(f.calls).includes('setTermInput'), 'the default changes nothing');
  f.calls.length = 0;
  s.apply(inputs({ termInput: 'select' }), f.mount);
  s.apply(inputs({ termInput: 'select' }), f.mount);
  assert.deepEqual(f.calls.filter((c) => c[0] === 'setTermInput'), [['setTermInput', 'select']]);
  f.calls.length = 0;
  s.apply(inputs(), f.mount);
  assert.deepEqual(f.calls.filter((c) => c[0] === 'setTermInput'), [['setTermInput', 'free']]);
});

test('termInput: a mode the handle refuses keeps the previous one and is tried again', () => {
  const s = new EditorSession();
  const f = fake();
  const error = console.error;
  console.error = () => {};
  try {
    s.apply(inputs({ termInput: 'select' }), f.mount);
    f.calls.length = 0;
    const out = s.apply(inputs({ termInput: 'dropdown' }), f.mount);
    assert.equal(out.validationError, undefined);
    f.calls.length = 0;
    s.apply(inputs({ termInput: 'dropdown' }), f.mount);
    assert.deepEqual(f.calls.filter((c) => c[0] === 'setTermInput'), [['setTermInput', 'dropdown']]);
    f.calls.length = 0;
    s.apply(inputs({ termInput: 'select' }), f.mount);
    assert.ok(!names(f.calls).includes('setTermInput'), 'select is still the mode');
  } finally {
    console.error = error;
  }
});

const GONE = [{ path: 'assigner', key: 'assigner', limit: 'policyAssigner' }];

test('outOfScope: show (the default) is never handed over and removed() is never read', () => {
  const s = new EditorSession();
  const f = fake();
  const out = s.apply(inputs({ limits: { policyAssigner: { max: 0 } } }), f.mount);
  assert.ok(!names(f.calls).includes('setOutOfScope'));
  assert.ok(!names(f.calls).includes('removed'));
  assert.equal(out.removed, undefined);
});

test('outOfScope: strip is handed over once, before the limits, and what it removed is reported', () => {
  const s = new EditorSession();
  const scope = { removed: [] };
  const f = fake(new Set(), scope);
  const lim = { policyAssigner: { max: 0 } };
  // The limits are what strips: the handle reports it after setLimits.
  f.mount0 = f.mount;
  const mount = (...a) => {
    const h = f.mount0(...a);
    const setLimits = h.setLimits;
    h.setLimits = (l) => {
      setLimits(l);
      scope.removed = GONE;
    };
    return h;
  };
  const out = s.apply(inputs({ outOfScope: 'strip', limits: lim }), mount);
  const order = names(f.calls);
  assert.ok(order.indexOf('setOutOfScope') < order.indexOf('setLimits'), order.join());
  assert.deepEqual(f.calls.find((c) => c[0] === 'setOutOfScope'), ['setOutOfScope', 'strip']);
  assert.deepEqual(out.removed, GONE);
  assert.equal(out.jsonError, undefined);
  f.calls.length = 0;
  const again = s.apply(inputs({ outOfScope: 'strip', limits: lim }), mount);
  assert.ok(!names(f.calls).includes('setOutOfScope'), 'once');
  assert.equal(again.removed, undefined, 'nothing new was removed');
});

test('outOfScope: a later json that loses sections reports them', () => {
  const s = new EditorSession();
  const scope = { removed: [] };
  const f = fake(new Set(), scope);
  s.apply(inputs({ outOfScope: 'strip' }), f.mount);
  scope.removed = GONE;
  const out = s.apply(inputs({ outOfScope: 'strip', json: B }), f.mount);
  assert.deepEqual(f.calls.find((c) => c[0] === 'update'), ['update', B, null]);
  assert.deepEqual(out.removed, GONE);
});

test('outOfScope: reject, a json the handle refuses is a jsonError and the document stays', () => {
  const s = new EditorSession();
  const f = fake(new Set([B]));
  s.apply(inputs({ outOfScope: 'reject' }), f.mount);
  const out = s.apply(inputs({ outOfScope: 'reject', json: B }), f.mount);
  assert.match(String(out.jsonError), /unreadable/);
  assert.equal(s.lastJson, A);
});

test('outOfScope: reject, limits the first document breaks fail the mount as a jsonError', () => {
  const s = new EditorSession();
  const scope = { refuseLimits: true };
  const f = fake(new Set(), scope);
  const lim = { policyAssigner: { max: 0 } };
  const out = s.apply(inputs({ outOfScope: 'reject', limits: lim }), f.mount);
  assert.equal(out.jsonError.name, 'OutOfScopeError');
  assert.deepEqual(out.jsonError.paths, ['permission[0].assignee']);
  assert.equal(s.handle, undefined, 'nothing stays mounted');
  assert.deepEqual(names(f.calls).filter((n) => n === 'destroy' || n === 'free'), ['destroy', 'free']);
  assert.equal(s.lastJson, undefined);
  // The next change of an input tries again, with the same limits and mode.
  scope.refuseLimits = false;
  f.calls.length = 0;
  const out2 = s.apply(inputs({ json: B, outOfScope: 'reject', limits: lim }), f.mount);
  assert.equal(out2.jsonError, undefined);
  assert.equal(s.handle, f.made[1]);
  assert.deepEqual(f.calls.filter((c) => c[0] === 'setOutOfScope'), [['setOutOfScope', 'reject']]);
  assert.ok(names(f.calls).includes('setLimits'));
});

test('outOfScope: reject, limits a later document breaks are a jsonError, the handle stays and the limits are tried again', () => {
  const s = new EditorSession();
  const scope = {};
  const f = fake(new Set(), scope);
  s.apply(inputs({ outOfScope: 'reject' }), f.mount);
  scope.refuseLimits = true;
  const lim = { ruleAssigner: { max: 0 } };
  const out = s.apply(inputs({ outOfScope: 'reject', limits: lim }), f.mount);
  assert.equal(out.jsonError.name, 'OutOfScopeError');
  assert.equal(s.handle, f.made[0]);
  assert.ok(!names(f.calls).includes('destroy'));
  scope.refuseLimits = false;
  f.calls.length = 0;
  s.apply(inputs({ outOfScope: 'reject', limits: lim }), f.mount);
  assert.deepEqual(f.calls.filter((c) => c[0] === 'setLimits'), [['setLimits', lim]]);
});

test('outOfScope: a mode the handle refuses for another reason keeps the previous mode and is tried again', () => {
  const s = new EditorSession();
  const f = fake();
  const error = console.error;
  console.error = () => {};
  try {
    s.apply(inputs(), f.mount);
    f.made[0].setOutOfScope = (m) => {
      f.calls.push(['setOutOfScope', m]);
      throw new Error('outOfScope bogus');
    };
    const out = s.apply(inputs({ outOfScope: 'bogus' }), f.mount);
    assert.equal(out.jsonError, undefined);
    f.calls.length = 0;
    s.apply(inputs({ outOfScope: 'bogus' }), f.mount);
    assert.deepEqual(f.calls.filter((c) => c[0] === 'setOutOfScope'), [['setOutOfScope', 'bogus']]);
  } finally {
    console.error = error;
  }
});

// --- the preset ----------------------------------------------------------------

const none = { strict: undefined, validation: null, termInput: undefined, limits: null, refuseTerms: undefined };

test('no preset leaves every input as the host gave it', () => {
  assert.deepEqual(resolvePreset(null, none), {
    options: null, termInput: 'free', limits: null, refuseTerms: false,
  });
  assert.deepEqual(resolvePreset(undefined, { ...none, strict: true, termInput: 'select', limits: { policies: { max: 1 } } }), {
    options: { strict: true }, termInput: 'select', limits: { policies: { max: 1 } }, refuseTerms: false,
  });
});

test("preset 'dataspace' is strict, structural, select, refusing, two groups deep, with the heuristic lints on", () => {
  const r = resolvePreset('dataspace', none);
  assert.deepEqual(r, {
    options: {
      strict: true,
      structural: true,
      rules: { 'heuristic.single-child-group': 'warning', 'heuristic.duplicate-action': 'warning' },
    },
    termInput: 'select',
    limits: { logicalDepth: { max: 2 } },
    refuseTerms: true,
  });
  assert.deepEqual(r, {
    options: DATASPACE_PRESET.validation, termInput: DATASPACE_PRESET.termInput,
    limits: DATASPACE_PRESET.limits, refuseTerms: DATASPACE_PRESET.refuseTerms,
  });
});

test('every input overrides the one thing the preset sets', () => {
  // The preset is not changed by what is resolved from it.
  const before = JSON.stringify(DATASPACE_PRESET);
  assert.equal(resolvePreset('dataspace', { ...none, strict: false }).options.strict, false);
  assert.equal(resolvePreset('dataspace', { ...none, strict: false }).options.structural, true);
  assert.equal(resolvePreset('dataspace', { ...none, termInput: 'free' }).termInput, 'free');
  assert.equal(resolvePreset('dataspace', { ...none, refuseTerms: false }).refuseTerms, false);
  assert.deepEqual(resolvePreset('dataspace', { ...none, limits: { logicalDepth: { max: 4 }, policies: { max: 1 } } }).limits, {
    logicalDepth: { max: 4 }, policies: { max: 1 },
  });
  assert.deepEqual(resolvePreset('dataspace', { ...none, limits: { policies: { max: 1 } } }).limits, {
    logicalDepth: { max: 2 }, policies: { max: 1 },
  });
  // `validation` wins key by key, and its rules are added to the preset's.
  const o = resolvePreset('dataspace', {
    ...none, validation: { structural: false, rules: { 'heuristic.duplicate-action': 'off', 'prose.value-not-allowed': 'error' } },
  }).options;
  assert.deepEqual(o, {
    strict: true,
    structural: false,
    rules: { 'heuristic.single-child-group': 'warning', 'heuristic.duplicate-action': 'off', 'prose.value-not-allowed': 'error' },
  });
  assert.equal(JSON.stringify(DATASPACE_PRESET), before);
  // What is resolved is a copy.
  const a = resolvePreset('dataspace', none);
  a.options.rules['x'] = 'off';
  a.limits.logicalDepth.max = 9;
  assert.equal(JSON.stringify(DATASPACE_PRESET), before);
});

test('an unknown preset throws and names the presets', () => {
  assert.throws(() => resolvePreset('nope', none), /dataspace/);
});

test('refuseTerms is handed over once, and only a change from the default', () => {
  const s = new EditorSession();
  const f = fake();
  s.apply(inputs(), f.mount);
  assert.ok(!names(f.calls).includes('setRefuseTerms'));
  s.apply(inputs({ refuseTerms: true }), f.mount);
  assert.deepEqual(f.calls.filter((c) => c[0] === 'setRefuseTerms'), [['setRefuseTerms', true]]);
  f.calls.length = 0;
  s.apply(inputs({ refuseTerms: true }), f.mount);
  assert.ok(!names(f.calls).includes('setRefuseTerms'));
  s.apply(inputs({ refuseTerms: false }), f.mount);
  assert.deepEqual(f.calls.filter((c) => c[0] === 'setRefuseTerms'), [['setRefuseTerms', false]]);
});

test('a first apply with the preset hands everything over once', () => {
  const s = new EditorSession();
  const f = fake();
  const r = resolvePreset('dataspace', none);
  s.apply(inputs({ options: r.options, limits: r.limits, termInput: r.termInput, refuseTerms: r.refuseTerms }), f.mount);
  assert.deepEqual(names(f.calls), ['new', 'setLimits', 'setTermInput', 'setRefuseTerms', 'setValidation', 'report']);
});

test('shapes: handed over once, after the profiles and before the validation, and reported', () => {
  const s = new EditorSession();
  const f = fake();
  const out = s.apply(inputs({ texts: ['p'], shapes: 'S', options: { strict: true } }), f.mount);
  assert.deepEqual(names(f.calls), ['new', 'setProfile', 'setShapes', 'setValidation', 'report']);
  assert.deepEqual(f.calls.find((c) => c[0] === 'setShapes'), ['setShapes', 'S']);
  assert.deepEqual(out.shapesSummary, { constraints: 2, actions: null, leftOperands: null, warnings: ['ignored sh:minCount'] });
  assert.equal(out.shapesError, undefined);
  // The same shapes again: nothing is handed over twice.
  f.calls.length = 0;
  const again = s.apply(inputs({ texts: ['p'], shapes: 'S', options: { strict: true } }), f.mount);
  assert.deepEqual(names(f.calls), ['report']);
  assert.equal(again.shapesSummary, undefined);
});

test('shapes: none (null, undefined, blank) on a fresh mount needs no call; going back to none clears them', () => {
  const s = new EditorSession();
  const f = fake();
  s.apply(inputs(), f.mount);
  s.apply(inputs({ shapes: null }), f.mount);
  s.apply(inputs({ shapes: '  \n' }), f.mount);
  assert.ok(!names(f.calls).includes('setShapes'));
  s.apply(inputs({ shapes: 'S' }), f.mount);
  f.calls.length = 0;
  const out = s.apply(inputs({ shapes: '' }), f.mount);
  assert.deepEqual(f.calls.filter((c) => c[0] === 'setShapes'), [['setShapes', null]]);
  assert.equal(out.shapesSummary, undefined, 'clearing reports no summary');
  assert.equal(out.shapesError, undefined);
});

test('shapes: a text the handle refuses is reported once, not retried until it changes', () => {
  const s = new EditorSession();
  const f = fake();
  const out = s.apply(inputs({ shapes: 'bad shapes' }), f.mount);
  assert.match(String(out.shapesError), /not shapes/);
  assert.equal(out.shapesSummary, undefined);
  f.calls.length = 0;
  const again = s.apply(inputs({ shapes: 'bad shapes' }), f.mount);
  assert.deepEqual(names(f.calls), ['report']);
  assert.equal(again.shapesError, undefined);
  const fixed = s.apply(inputs({ shapes: 'S' }), f.mount);
  assert.equal(fixed.shapesError, undefined);
  assert.equal(fixed.shapesSummary.constraints, 2);
});

test('shapes: a failed first mount keeps them unspent for the retry', () => {
  const s = new EditorSession();
  const f = fake(new Set([A]));
  s.apply(inputs({ shapes: 'S' }), f.mount);
  assert.ok(!names(f.calls).includes('setShapes'), 'nothing else was handed over');
  s.apply(inputs({ json: B, shapes: 'S' }), f.mount);
  assert.deepEqual(f.calls.filter((c) => c[0] === 'setShapes'), [['setShapes', 'S']]);
});
