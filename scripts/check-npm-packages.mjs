#!/usr/bin/env node
// Checks the packed tarballs in dist-npm/tarballs before anything is
// published: the right files, the metadata a registry page needs, and none of
// the ways an npm package silently ends up empty or wrong.
import { execFileSync } from 'node:child_process';
import { readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const dir = process.env.TARBALLS_DIR ?? join(resolve(dirname(fileURLToPath(import.meta.url)), '..'), 'dist-npm', 'tarballs');
const problems = [];
const need = (cond, message) => { if (!cond) problems.push(message); };

const tar = (file, args) => execFileSync('tar', [...args, '-f', join(dir, file)], { encoding: 'utf8', maxBuffer: 64 << 20 });
// "^X.Y.Z" the way npm reads it, for the simple release versions used here.
function satisfiesCaret(range, version) {
  const r = /^\^(\d+)\.(\d+)\.(\d+)$/.exec(range);
  const v = /^(\d+)\.(\d+)\.(\d+)$/.exec(version);
  if (!r || !v) return false;
  const [R, V] = [r.slice(1).map(Number), v.slice(1).map(Number)];
  const atLeast = V[0] > R[0] || (V[0] === R[0] && (V[1] > R[1] || (V[1] === R[1] && V[2] >= R[2])));
  if (R[0] > 0) return V[0] === R[0] && atLeast;
  if (R[1] > 0) return V[0] === 0 && V[1] === R[1] && atLeast;
  return V[0] === 0 && V[1] === 0 && V[2] === R[2];
}
const manifests = {};

const files = readdirSync(dir).filter((f) => f.endsWith('.tgz'));
need(files.length === 2, `expected 2 tarballs, found ${files.length}: ${files.join(', ')}`);

for (const file of files) {
  const listed = tar(file, ['-tz']).split('\n').filter(Boolean).map((p) => p.replace(/^package\//, ''));
  const pkg = JSON.parse(tar(file, ['-xzO', 'package/package.json']));
  const has = (name) => listed.includes(name);
  const tag = `${pkg.name}@${pkg.version}`;
  manifests[pkg.name] = pkg;

  for (const f of ['package.json', 'README.md', 'LICENSE']) need(has(f), `${tag}: missing ${f}`);
  need(pkg.license === 'Apache-2.0', `${tag}: license is ${pkg.license}`);
  need(pkg.repository?.url?.includes('ds-labs-org/ds-prose-rs'), `${tag}: no repository`);
  need(!pkg.private, `${tag}: is marked private`);
  need(pkg.publishConfig?.access === 'public', `${tag}: publishConfig.access is not public`);
  need(!pkg.publishConfig?.provenance, `${tag}: publishConfig.provenance breaks a local publish; CI requests it`);
  need(!listed.some((p) => p === '.gitignore' || p.startsWith('node_modules/')), `${tag}: ships .gitignore or node_modules`);
  need(!listed.some((p) => p.endsWith('.ts') && !p.endsWith('.d.ts')), `${tag}: ships TypeScript source`);

  if (pkg.name === '@ds-labs/prose-wasm') {
    need(has('prose_wasm_bg.wasm'), `${tag}: no wasm`);
    need(has('prose_wasm.js') && has('prose_wasm.d.ts'), `${tag}: no JS glue or types`);
    const wasmBytes = Number(execFileSync('sh', ['-c', `tar -xzOf '${join(dir, file)}' package/prose_wasm_bg.wasm 2>/dev/null | wc -c`], { encoding: 'utf8' }));
    need(wasmBytes > 100_000, `${tag}: wasm is only ${wasmBytes} bytes`);
    need(pkg.dsProse?.['prose-yew'], `${tag}: dsProse.prose-yew not recorded`);
  } else if (pkg.name === '@ds-labs/prose-angular') {
    need(pkg.dependencies?.['@ds-labs/prose-wasm'], `${tag}: does not depend on @ds-labs/prose-wasm`);
    need(pkg.peerDependencies?.['@angular/core'], `${tag}: no @angular/core peer dependency`);
    need(listed.some((p) => p.startsWith('fesm2022/') && p.endsWith('.mjs')), `${tag}: no FESM bundle`);
    need(has('index.d.ts'), `${tag}: no type declarations`);
    need(!listed.some((p) => p.includes('prose_wasm')), `${tag}: carries its own wasm (it belongs to @ds-labs/prose-wasm)`);
  } else {
    problems.push(`unexpected package ${pkg.name}`);
  }
  console.log(`${tag}: ${listed.length} files`);
}

// The wasm package packed here must satisfy the range the Angular package
// declares: otherwise installing the tarballs together would quietly fetch a
// second copy from the registry and the test would no longer be representative.
const ng = manifests['@ds-labs/prose-angular'];
const wasm = manifests['@ds-labs/prose-wasm'];
if (ng && wasm) {
  const range = ng.dependencies?.['@ds-labs/prose-wasm'];
  need(satisfiesCaret(range ?? '', wasm.version), `prose-angular declares @ds-labs/prose-wasm ${range}, but prose-wasm ${wasm.version} was packed: update the range in prose-angular/package.json`);
}

if (problems.length) {
  console.error('\npackage check FAILED:\n- ' + problems.join('\n- '));
  process.exit(1);
}
console.log('package check ok');
