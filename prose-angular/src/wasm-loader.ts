/** Loads and instantiates the wasm module. Injected so it can be tested. */
export type WasmInit<A, T> = (arg: A) => Promise<T>;

/**
 * A loader that runs `init` at most once at a time and shares the result, so
 * every component on a page waits on one fetch. A failed load is forgotten:
 * the callers waiting on it all see the failure, and the next call tries again
 * (the file may have been missing only for a moment).
 */
export function createWasmLoader<A, T>(init: WasmInit<A, T>): (arg: A) => Promise<T> {
  let ready: Promise<T> | undefined;
  return (arg) => {
    if (!ready) {
      const attempt: Promise<T> = init(arg).catch((error: unknown) => {
        if (ready === attempt) ready = undefined;
        throw error;
      });
      ready = attempt;
    }
    return ready;
  };
}
