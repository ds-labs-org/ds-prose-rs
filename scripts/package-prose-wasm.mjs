#!/usr/bin/env node
// Builds @ds-labs/prose-wasm into dist-npm/prose-wasm, ready for `npm pack` or
// `npm publish`: the wasm-bindgen output (web target) plus a package.json,
// README and LICENSE written here, so the package is not at the mercy of
// wasm-pack's defaults, and the ds-prose-validate command copied in.
//
// Two builds of the same API, with the size-tuned `wasm-release` profile:
//   prose_wasm.js, prose_wasm_bg.wasm                 JSON-LD profiles only
//                                                     (the package's main entry)
//   prose_wasm_turtle.js, prose_wasm_turtle_bg.wasm   JSON-LD and Turtle
//                                                     (the `./turtle` entry)
// Each glue file goes with its own wasm: wasm-bindgen numbers the exports it
// generates, so the two are not interchangeable.
//
//   node scripts/package-prose-wasm.mjs [version]
//
// The version normally equals the Rust workspace's (they are released
// together). The workspace version the wasm was built from is recorded in
// package.json as dsProse.prose-yew.
import { execFileSync } from 'node:child_process';
import { chmodSync, copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const out = join(root, 'dist-npm', 'prose-wasm');
const source = join(root, 'prose-wasm', 'npm');

const own = JSON.parse(readFileSync(join(source, 'package.json'), 'utf8'));
const version = process.argv[2] || own.version;
if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version)) {
  console.error(`not a semver version: ${version}`);
  process.exit(2);
}
const cargo = readFileSync(join(root, 'Cargo.toml'), 'utf8');
const workspaceVersion = /\[workspace\.package\][^[]*?\nversion\s*=\s*"([^"]+)"/s.exec(cargo)?.[1];
if (!workspaceVersion) {
  console.error('could not read the workspace version from Cargo.toml');
  process.exit(2);
}

const turtleOut = join(root, 'dist-npm', 'prose-wasm-turtle');
const wasmPack = (dir, name, ...cargoArgs) =>
  execFileSync(
    'wasm-pack',
    ['build', join(root, 'prose-wasm'), '--profile', 'wasm-release', '--no-opt', '--target', 'web',
      '--out-dir', dir, '--out-name', name, ...(cargoArgs.length ? ['--', ...cargoArgs] : [])],
    { stdio: 'inherit' },
  );
for (const dir of [out, turtleOut]) {
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
}
wasmPack(out, 'prose_wasm', '--no-default-features');
wasmPack(turtleOut, 'prose_wasm_turtle');

// Both entries declare the same API (up to the numbered internal exports).
const types = (dir, name) =>
  readFileSync(join(dir, `${name}.d.ts`), 'utf8').replaceAll(name, 'NAME').replace(/__wasm_bindgen_func_elem_\d+/g, 'ELEM');
if (types(out, 'prose_wasm') !== types(turtleOut, 'prose_wasm_turtle')) {
  console.error('prose_wasm.d.ts and prose_wasm_turtle.d.ts declare different APIs');
  process.exit(1);
}
for (const f of ['prose_wasm_turtle.js', 'prose_wasm_turtle.d.ts', 'prose_wasm_turtle_bg.wasm', 'prose_wasm_turtle_bg.wasm.d.ts']) {
  copyFileSync(join(turtleOut, f), join(out, f));
}
rmSync(turtleOut, { recursive: true, force: true });
// wasm-pack writes a .gitignore of "*", which makes npm leave everything out.
rmSync(join(out, '.gitignore'), { force: true });
rmSync(join(out, 'package.json'), { force: true });
rmSync(join(out, 'README.md'), { force: true });

const pkg = { ...own, version, dsProse: { 'prose-yew': workspaceVersion } };
writeFileSync(join(out, 'package.json'), JSON.stringify(pkg, null, 2) + '\n');
copyFileSync(join(source, 'README.md'), join(out, 'README.md'));
copyFileSync(join(root, 'LICENSE'), join(out, 'LICENSE'));
// The command line validator; it loads the wasm module that sits next to it.
copyFileSync(join(root, 'scripts', 'ds-prose-validate.mjs'), join(out, 'ds-prose-validate.mjs'));
chmodSync(join(out, 'ds-prose-validate.mjs'), 0o755);
console.log(`packaged ${pkg.name}@${version} (prose-yew ${workspaceVersion}) in ${out}`);
