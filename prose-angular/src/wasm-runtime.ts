import { InjectionToken } from '@angular/core';
import type * as ProseWasm from '@ds-labs/prose-wasm';
import { createWasmLoader } from './wasm-loader';

/** The JavaScript module of `@ds-labs/prose-wasm` (either build). */
export type ProseWasmModule = typeof ProseWasm;

/**
 * Where the browser fetches `prose_wasm_bg.wasm` from, relative to the page's
 * base href. The build does not emit the file for you: copy it into your
 * assets (see README) or provide this token. With the Turtle build (see
 * `PROSE_WASM_MODULE`), point it at `prose_wasm_turtle_bg.wasm`.
 */
export const PROSE_WASM_URL = new InjectionToken<string>('PROSE_WASM_URL', {
  providedIn: 'root',
  factory: () => 'prose_wasm_bg.wasm',
});

/**
 * Which build of `@ds-labs/prose-wasm` runs, as a function that imports it.
 * The default reads profiles written in JSON-LD. To read Turtle profiles as
 * well, provide `() => import('@ds-labs/prose-wasm/turtle')`, together with
 * `PROSE_WASM_URL` set to `prose_wasm_turtle_bg.wasm`.
 */
export const PROSE_WASM_MODULE = new InjectionToken<() => Promise<ProseWasmModule>>('PROSE_WASM_MODULE', {
  providedIn: 'root',
  factory: () => () => import('@ds-labs/prose-wasm'),
});

// The wasm module is fetched and instantiated once per page. A failed load is
// not remembered, so the next input change retries it.
export const loadWasm: (from: { module: () => Promise<ProseWasmModule>; url: string }) => Promise<ProseWasmModule> =
  createWasmLoader(async ({ module, url }) => {
    const wasm = await module();
    await wasm.default({ module_or_path: new URL(url, document.baseURI) });
    return wasm;
  });
