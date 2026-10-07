#!/usr/bin/env node
// Builds @ds-labs/prose-wasm into dist-npm/prose-wasm, ready for `npm pack` or
// `npm publish`: the wasm-bindgen output (web target) plus a package.json,
// README and LICENSE written here, so the package is not at the mercy of
// wasm-pack's defaults.
//
//   node scripts/package-prose-wasm.mjs [version]
//
// The version is independent of the Rust workspace's. The workspace version
// the wasm was built from is recorded in package.json as dsProse.prose-yew.
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
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

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
execFileSync(
  'wasm-pack',
  ['build', join(root, 'prose-wasm'), '--release', '--target', 'web', '--out-dir', out, '--out-name', 'prose_wasm'],
  { stdio: 'inherit' },
);
// wasm-pack writes a .gitignore of "*", which makes npm leave everything out.
rmSync(join(out, '.gitignore'), { force: true });
rmSync(join(out, 'package.json'), { force: true });
rmSync(join(out, 'README.md'), { force: true });

const pkg = { ...own, version, dsProse: { 'prose-yew': workspaceVersion } };
writeFileSync(join(out, 'package.json'), JSON.stringify(pkg, null, 2) + '\n');
copyFileSync(join(source, 'README.md'), join(out, 'README.md'));
copyFileSync(join(root, 'LICENSE'), join(out, 'LICENSE'));
console.log(`packaged ${pkg.name}@${version} (prose-yew ${workspaceVersion}) in ${out}`);
