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
} from '@angular/core';
import init, { ProseHandle } from '../wasm/prose_wasm.js';

/**
 * Where the browser fetches `prose_wasm_bg.wasm` from, relative to the page's
 * base href. The build does not emit the file for you: copy it into your
 * assets (see README) or provide this token.
 */
export const PROSE_WASM_URL = new InjectionToken<string>('PROSE_WASM_URL', {
  providedIn: 'root',
  factory: () => 'prose_wasm_bg.wasm',
});

// The wasm module is fetched and instantiated once per page.
let ready: Promise<unknown> | undefined;
const loadWasm = (url: string) => (ready ??= init({ module_or_path: new URL(url, document.baseURI) }));

/**
 * `<ds-odrl-prose [json]="policy" />` renders ODRL JSON-LD as plain-language
 * prose using the Yew component from `prose-yew`.
 *
 * The component owns the host element's children (Yew keeps its own virtual
 * DOM there); style it through the `prose-*` classes or the `class` input.
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
    await loadWasm(this.wasmUrl);
    if (this.destroyed) return;
    const json = this.json();
    const cls = this.articleClass() ?? null;
    // Yew schedules its own work; keep it out of Angular's change detection.
    this.zone.runOutsideAngular(() => {
      if (this.handle) this.handle.update(json, cls);
      else this.handle = new ProseHandle(this.host, json, cls);
    });
  }
}
