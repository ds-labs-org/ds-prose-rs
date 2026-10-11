import { InjectionToken } from '@angular/core';
import init from '@ds-labs/prose-wasm';
import { createWasmLoader } from './wasm-loader';

/**
 * Where the browser fetches `prose_wasm_bg.wasm` from, relative to the page's
 * base href. The build does not emit the file for you: copy it into your
 * assets (see README) or provide this token.
 */
export const PROSE_WASM_URL = new InjectionToken<string>('PROSE_WASM_URL', {
  providedIn: 'root',
  factory: () => 'prose_wasm_bg.wasm',
});

// The wasm module is fetched and instantiated once per page. A failed load is
// not remembered, so the next input change retries it.
export const loadWasm = createWasmLoader((url) => init({ module_or_path: new URL(url, document.baseURI) }));
