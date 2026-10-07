import {
  ChangeDetectionStrategy,
  Component,
  ElementRef,
  NgZone,
  OnChanges,
  OnDestroy,
  InjectionToken,
  inject,
  input,
  output,
} from '@angular/core';
import init, { ProseHandle } from '../wasm/prose_wasm.js';
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
const loadWasm = createWasmLoader((url) => init({ module_or_path: new URL(url, document.baseURI) }));

/**
 * `<ds-odrl-prose [json]="policy" />` renders ODRL JSON-LD as plain-language
 * prose using the Yew component from `prose-yew`.
 *
 * The component owns the host element's children (Yew keeps its own virtual
 * DOM there); style it through the `prose-*` classes or the `class` input.
 *
 * If the wasm cannot be loaded, the host shows an error (`role="alert"`,
 * class `prose-error`), `loadFailed` emits the error, and the next change of
 * an input tries again.
 */
@Component({
  selector: 'ds-odrl-prose',
  standalone: true,
  template: '',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class OdrlProseComponent implements OnChanges, OnDestroy {
  /** The ODRL policy as JSON-LD text. */
  readonly json = input.required<string>();
  /** Extra class(es) for the rendered `<article class="ds-prose">`. */
  readonly articleClass = input<string>();
  /** Emits when the wasm module could not be loaded. */
  readonly loadFailed = output<unknown>();

  private readonly host = inject<ElementRef<HTMLElement>>(ElementRef).nativeElement;
  private readonly zone = inject(NgZone);
  private readonly wasmUrl = inject(PROSE_WASM_URL);
  private handle?: ProseHandle;
  private destroyed = false;

  ngOnChanges(): void {
    void this.sync();
  }

  ngOnDestroy(): void {
    this.destroyed = true;
    this.handle?.destroy();
    this.handle?.free();
    this.handle = undefined;
  }

  private async sync(): Promise<void> {
    try {
      await loadWasm(this.wasmUrl);
    } catch (error) {
      if (!this.destroyed) this.showLoadError(error);
      return;
    }
    if (this.destroyed) return;
    // Only the error message can be in the host before the first mount.
    if (!this.handle) this.host.replaceChildren();
    const json = this.json();
    const cls = this.articleClass() ?? null;
    // Yew schedules its own work; keep it out of Angular's change detection.
    this.zone.runOutsideAngular(() => {
      if (this.handle) this.handle.update(json, cls);
      else this.handle = new ProseHandle(this.host, json, cls);
    });
  }

  private showLoadError(error: unknown): void {
    console.error('ds-odrl-prose: could not load the wasm module', error);
    // A mounted Yew tree owns the host's children: leave it alone.
    if (!this.handle) {
      const message = document.createElement('p');
      message.className = 'prose-error';
      message.setAttribute('role', 'alert');
      message.textContent =
        `The ODRL prose renderer could not be loaded (${this.wasmUrl}). Check that the file is served; it is tried again when the input changes.`;
      this.host.replaceChildren(message);
    }
    this.loadFailed.emit(error);
  }
}
