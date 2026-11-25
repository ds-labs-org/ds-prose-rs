// Run with `npm test`. Node strips the TypeScript types on import.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createWasmLoader } from '../src/wasm-loader.ts';

test('concurrent callers share one load', async () => {
  let calls = 0;
  const load = createWasmLoader(async () => { calls++; });
  await Promise.all([load('a.wasm'), load('a.wasm'), load('a.wasm')]);
  assert.equal(calls, 1);
});

test('a successful load is not repeated', async () => {
  let calls = 0;
  const load = createWasmLoader(async () => { calls++; });
  await load('a.wasm');
  await load('a.wasm');
  assert.equal(calls, 1);
});

test('every concurrent caller sees a failure', async () => {
  const load = createWasmLoader(async () => { throw new Error('404'); });
  const results = await Promise.allSettled([load('a.wasm'), load('a.wasm')]);
  assert.deepEqual(results.map((r) => r.status), ['rejected', 'rejected']);
});

test('a failed load is not cached: the next call retries and can succeed', async () => {
  let calls = 0;
  const load = createWasmLoader(async () => {
    calls++;
    if (calls === 1) throw new Error('404');
  });
  await assert.rejects(load('a.wasm'), /404/);
  await load('a.wasm'); // must retry, not replay the first rejection
  assert.equal(calls, 2);
});
