import {
  ChangeDetectionStrategy,
  Component,
  ElementRef,
  NgZone,
  OnChanges,
  OnDestroy,
  inject,
  input,
  output,
  signal,
} from '@angular/core';
import type { EditLimits, ProseEditorHandle, ValidateOptions, ValidationReport } from '@ds-labs/prose-wasm';
import { ProfileSummary, parseSummary } from './profile-sync';
import { createChangeTracker, parseReport, profileTexts, reportChanged, validationOptions } from './validation-sync';
import { PROSE_WASM_MODULE, PROSE_WASM_URL, type ProseWasmModule, loadWasm } from './wasm-runtime';

/**
 * `<ds-odrl-prose-editor [json]="policy" (jsonChange)="policy = $event" />`
 * edits ODRL JSON-LD as plain-language sentences, using the Yew editor of
 * `prose-yew`: click a word to type, pick from the suggestions, add and
 * remove parts with the + and − buttons.
 *
 * The component is controlled. `json` is the document; `jsonChange` emits the
 * JSON-LD the editor writes after each committed edit. Bind back exactly the
 * text it emitted: the editor recognises its own echo and keeps the undo
 * history and the focus. Binding another text replaces the document (and
 * clears the history). Reformatting the emitted text before binding it back
 * counts as another text.
 *
 * The editor reads and writes the bare keys of ODRL JSON-LD (`permission`,
 * `constraint`, ...). A host whose documents use prefixed keys (`odrl:permission`)
 * converts them before binding.
 *
 * The component owns the host element's children and the editor injects its
 * own stylesheet (`BASE_CSS`); `articleClass` adds classes to the rendered
 * `<article class="ds-prose prose-edit">`.
 *
 * `profile` / `profiles` take the text of ODRL profiles, as for
 * `ds-odrl-prose`. `limits` sets how many items each list may hold
 * (`{ policyAssigner: { max: 0 } }` removes the assigner button). When
 * validation is on (`validation`, or `strict`) the editor marks issues inline
 * and shows a problems panel with fix buttons; edits are flagged, never
 * refused. `validationChange` emits the report when the verdict or the
 * findings change, also after edits; the `report` signal holds the latest.
 *
 * If `json` is not a readable ODRL policy, `jsonError` emits: at the first
 * mount nothing is mounted and the next change of an input tries again; later
 * the document on screen stays. If the wasm cannot be loaded, the host shows
 * an error (`role="alert"`, class `prose-error`) and `loadFailed` emits.
 */
@Component({
  selector: 'ds-odrl-prose-editor',
  standalone: true,
  template: '',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class OdrlProseEditorComponent implements OnChanges, OnDestroy {
  /** The ODRL policy as JSON-LD text. Controlled: bind back what `jsonChange` emits. */
  readonly json = input.required<string>();
  /** Extra class(es) for the rendered `<article class="ds-prose prose-edit">`. */
  readonly articleClass = input<string>();
  /** The ODRL profile the editor is worded with, as JSON-LD text (or Turtle, with the Turtle build). */
  readonly profile = input<string | null>(null);
  /** Several ODRL profiles, as JSON-LD (or Turtle) text each; wins over `profile`. */
  readonly profiles = input<string[] | null>(null);
  /** How many items each list may hold; a list not named keeps its default. */
  readonly limits = input<EditLimits | null>(null);
  /** Validate with a closed vocabulary: ODRL plus the loaded profiles. */
  readonly strict = input(false);
  /** The full validation options; they take precedence over `strict`. */
  readonly validation = input<ValidateOptions | null>(null);
  /** Emits the JSON-LD text the editor writes after each committed edit. */
  readonly jsonChange = output<string>();
  /** Emits when `json` is not a readable ODRL policy. */
  readonly jsonError = output<unknown>();
  /** Emits when the wasm module could not be loaded. */
  readonly loadFailed = output<unknown>();
  /** Emits the summary of each profile loaded from the `profile` input. */
  readonly profileLoaded = output<ProfileSummary>();
  /** Emits when the `profile` input is not a readable profile; the previous one stays. */
  readonly profileError = output<unknown>();
  /** Emits the report when the verdict or the findings change, also after an edit. */
  readonly validationChange = output<ValidationReport>();
  /** Emits when `validation` is not readable options; the previous options stay. */
  readonly validationError = output<unknown>();
  /** The latest report, or `null` while validation is off. */
  readonly report = signal<ValidationReport | null>(null);

  private readonly host = inject<ElementRef<HTMLElement>>(ElementRef).nativeElement;
  private readonly zone = inject(NgZone);
  private readonly wasmUrl = inject(PROSE_WASM_URL);
  private readonly wasmModule = inject(PROSE_WASM_MODULE);
  private wasm?: ProseWasmModule;
  private handle?: ProseEditorHandle;
  private readonly profilesChanged = createChangeTracker<string[]>();
  private readonly optionsChanged = createChangeTracker<ValidateOptions>();
  private readonly limitsChanged = createChangeTracker<EditLimits>();
  // The text the handle holds as far as this component knows: the last one
  // given to it or emitted by it.
  private lastJson?: string;
  private lastClass: string | null = null;
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
      this.wasm = await loadWasm({ module: this.wasmModule, url: this.wasmUrl });
    } catch (error) {
      if (!this.destroyed) this.showLoadError(error);
      return;
    }
    if (this.destroyed) return;
    // Only the error message can be in the host before the first mount.
    if (!this.handle) this.host.replaceChildren();
    const json = this.json();
    const cls = this.articleClass() ?? null;
    const single = this.profiles() == null;
    const texts = profileTexts(this.profile(), this.profiles());
    const options = validationOptions(this.strict(), this.validation());
    const limits = this.limits();
    // Yew schedules its own work; keep it out of Angular's change detection.
    const outcome = this.zone.runOutsideAngular(() => {
      const out: {
        summaries?: ProfileSummary[];
        jsonError?: unknown;
        profileError?: unknown;
        validationError?: unknown;
        report?: string | null;
      } = {};
      let handle = this.handle;
      if (!handle) {
        try {
          handle = new this.wasm!.ProseEditorHandle(this.host, json, cls, (text, report) => this.onChange(text, report));
        } catch (error) {
          // Nothing is mounted; the next change of an input tries again.
          out.jsonError = error;
          return out;
        }
        this.handle = handle;
        this.lastJson = json;
        this.lastClass = cls;
      } else if (json !== this.lastJson || cls !== this.lastClass) {
        try {
          handle.update(json, cls);
          this.lastJson = json;
          this.lastClass = cls;
        } catch (error) {
          // The document on screen stays.
          out.jsonError = error;
        }
      }
      // The profiles go first, so the validation below uses them.
      if (this.profilesChanged(texts)) {
        try {
          out.summaries = single
            ? [parseSummary(handle.setProfile(texts?.[0] ?? null))].filter((x) => x != null)
            : (JSON.parse(handle.setProfiles(texts ?? [])) as ProfileSummary[]);
        } catch (error) {
          out.profileError = error;
        }
      }
      if (this.limitsChanged(limits)) {
        try {
          handle.setLimits(limits ?? null);
        } catch (error) {
          // The previous limits stay; there is no output for this, a host
          // error in the limits is a programming error.
          console.error('ds-odrl-prose-editor: the limits are not readable', error);
        }
      }
      if (this.optionsChanged(options)) {
        try {
          handle.setValidation(options);
        } catch (error) {
          out.validationError = error;
        }
      }
      try {
        out.report = handle.report();
      } catch (error) {
        out.validationError ??= error;
      }
      return out;
    });
    if (outcome.jsonError !== undefined) this.jsonError.emit(outcome.jsonError);
    if (outcome.profileError !== undefined) this.profileError.emit(outcome.profileError);
    for (const summary of outcome.summaries ?? []) this.profileLoaded.emit(summary);
    if (outcome.validationError !== undefined) this.validationError.emit(outcome.validationError);
    if (outcome.report !== undefined) this.publish(parseReport<ValidationReport>(outcome.report));
  }

  /** The editor committed an edit. Runs inside the wasm call: no calls back into the handle. */
  private onChange(text: string, report: string | null): void {
    this.zone.run(() => {
      this.lastJson = text;
      this.publish(parseReport<ValidationReport>(report));
      this.jsonChange.emit(text);
    });
  }

  private publish(next: ValidationReport | null): void {
    // Signals and outputs are Angular's: back inside the zone.
    this.zone.run(() => {
      const prev = this.report();
      this.report.set(next);
      if (next && reportChanged(prev, next)) this.validationChange.emit(next);
    });
  }

  private showLoadError(error: unknown): void {
    console.error('ds-odrl-prose-editor: could not load the wasm module', error);
    // A mounted Yew tree owns the host's children: leave it alone.
    if (!this.handle) {
      const message = document.createElement('p');
      message.className = 'prose-error';
      message.setAttribute('role', 'alert');
      message.textContent =
        `The ODRL prose editor could not be loaded (${this.wasmUrl}). Check that the file is served; it is tried again when the input changes.`;
      this.host.replaceChildren(message);
    }
    this.loadFailed.emit(error);
  }
}
