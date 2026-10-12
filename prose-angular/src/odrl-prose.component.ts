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
import { ProseHandle, ProseValidator, type ValidateOptions, type ValidationReport } from '@ds-labs/prose-wasm';
import { ProfileSummary, parseSummary } from './profile-sync';
import { StandaloneReporter, createChangeTracker, parseReport, profileTexts, reportChanged, validationOptions } from './validation-sync';
import { PROSE_WASM_URL, loadWasm } from './wasm-runtime';

export { PROSE_WASM_URL };

/**
 * `<ds-odrl-prose [json]="policy" />` renders ODRL JSON-LD as plain-language
 * prose using the Yew component from `prose-yew`.
 *
 * The component owns the host element's children (Yew keeps its own virtual
 * DOM there); style it through the `prose-*` classes or the `class` input.
 *
 * `[profile]` takes the text of an ODRL profile (JSON-LD or Turtle, as the
 * host fetched it; the component does no network access). The prose then uses
 * the profile's labels, and a glossary of its terms follows each policy.
 * `profileLoaded` emits the profile's summary; an unreadable profile emits
 * `profileError` and the previous profile stays.
 *
 * `[profiles]` takes several profiles at once (a list wins over `profile`).
 *
 * `[strict]="true"` validates the policy against the loaded profiles with a
 * closed vocabulary: ODRL plus those profiles. `[validation]` takes the full
 * options (it wins over `strict`). `validationChange` emits the report
 * (schema `ds-prose/validation-report@1`) each time the verdict or the
 * findings change, and the `report` signal always holds the latest. A
 * verdict of `incomplete` is never a pass: something could not be checked,
 * such as a declared profile that is not loaded. `[showViolations]` also
 * shows the findings under the prose.
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
  /** The ODRL profile the prose is worded with, as JSON-LD or Turtle text. */
  readonly profile = input<string | null>(null);
  /** Several ODRL profiles, as JSON-LD or Turtle text each; wins over `profile`. */
  readonly profiles = input<string[] | null>(null);
  /** Validate with a closed vocabulary: ODRL plus the loaded profiles. */
  readonly strict = input(false);
  /** The full validation options; they take precedence over `strict`. */
  readonly validation = input<ValidateOptions | null>(null);
  /** Show the findings and the verdict under the prose. Needs `strict` or `validation`. */
  readonly showViolations = input(false);
  /** Emits when the wasm module could not be loaded. */
  readonly loadFailed = output<unknown>();
  /** Emits the summary of each profile loaded from the `profile` input. */
  readonly profileLoaded = output<ProfileSummary>();
  /** Emits when the `profile` input is not a readable profile; the previous one stays. */
  readonly profileError = output<unknown>();
  /** Emits the report when the verdict or the findings change. */
  readonly validationChange = output<ValidationReport>();
  /** Emits when `validation` is not readable options; the previous options stay. */
  readonly validationError = output<unknown>();
  /** The latest report, or `null` while validation is off. */
  readonly report = signal<ValidationReport | null>(null);

  private readonly host = inject<ElementRef<HTMLElement>>(ElementRef).nativeElement;
  private readonly zone = inject(NgZone);
  private readonly wasmUrl = inject(PROSE_WASM_URL);
  private handle?: ProseHandle;
  private readonly profilesChanged = createChangeTracker<string[]>();
  private readonly handleOptionsChanged = createChangeTracker<ValidateOptions>();
  // The profiles the handle holds now, and the validator built from them for
  // a report the handle is not asked for.
  private appliedProfiles: string[] = [];
  private readonly standalone = new StandaloneReporter((profiles, options) => new ProseValidator(profiles, options));
  // Whether the handle validates (it was given options, with the findings shown).
  private handleValidates = false;
  private destroyed = false;

  ngOnChanges(): void {
    void this.sync();
  }

  ngOnDestroy(): void {
    this.destroyed = true;
    this.handle?.destroy();
    this.handle?.free();
    this.handle = undefined;
    this.standalone.free();
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
    const single = this.profiles() == null;
    const texts = profileTexts(this.profile(), this.profiles());
    const options = validationOptions(this.strict(), this.validation());
    const show = this.showViolations() && options != null;
    // Yew schedules its own work; keep it out of Angular's change detection.
    const outcome = this.zone.runOutsideAngular(() => {
      const mounted = this.handle;
      const handle = mounted ?? new ProseHandle(this.host, json, cls);
      this.handle = handle;
      const out: {
        summaries?: ProfileSummary[];
        profileError?: unknown;
        validationError?: unknown;
        report?: string | null;
      } = {};
      // The profiles go first, so the validation and the update below use them.
      if (this.profilesChanged(texts)) {
        try {
          out.summaries = single
            ? [parseSummary(handle.setProfile(texts?.[0] ?? null))].filter((x) => x != null)
            : (JSON.parse(handle.setProfiles(texts ?? [])) as ProfileSummary[]);
          this.appliedProfiles = texts ?? [];
        } catch (error) {
          out.profileError = error;
        }
      }
      if (this.handleOptionsChanged(show ? options : null)) {
        try {
          handle.setValidation(show ? options : null);
          this.handleValidates = show;
        } catch (error) {
          out.validationError = error;
        }
      }
      if (mounted) handle.update(json, cls);
      if (options) {
        if (this.handleValidates) {
          try {
            out.report = handle.report();
          } catch (error) {
            out.validationError ??= error;
          }
        } else {
          // Unreadable options keep the previous ones; see StandaloneReporter.
          const standalone = this.standalone.report(this.appliedProfiles, options, json);
          out.report = standalone.report;
          if (standalone.error !== undefined) out.validationError ??= standalone.error;
        }
      } else {
        this.standalone.clear();
        out.report = null;
      }
      return out;
    });
    if (outcome.profileError !== undefined) this.profileError.emit(outcome.profileError);
    for (const summary of outcome.summaries ?? []) this.profileLoaded.emit(summary);
    if (outcome.validationError !== undefined) this.validationError.emit(outcome.validationError);
    if (outcome.report !== undefined) this.publish(parseReport<ValidationReport>(outcome.report));
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
