/** Loads and instantiates the wasm module. Injected so it can be tested. */
export type WasmInit = (url: string) => Promise<unknown>;

/**
 * A loader that runs `init` at most once at a time and shares the result, so
 * every component on a page waits on one fetch. A failed load is forgotten:
 * the callers waiting on it all see the failure, and the next call tries again
 * (the file may have been missing only for a moment).
 */
export function createWasmLoader(init: WasmInit): (url: string) => Promise<unknown> {
  let ready: Promise<unknown> | undefined;
  return (url) => {
    if (!ready) {
      const attempt: Promise<unknown> = init(url).catch((error: unknown) => {
        if (ready === attempt) ready = undefined;
        throw error;
      });
      ready = attempt;
    }
    return ready;
  };
}
