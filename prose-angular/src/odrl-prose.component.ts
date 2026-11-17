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
import type { OverlaySummary, ProseHandle, ShapesSummary, ValidateOptions, ValidationReport, ValueSourceSummary, ValueSources } from '@ds-labs/prose-wasm';
import { ProfileSummary, parseSummary } from './profile-sync';
import { StandaloneReporter, createChangeTracker, overlayTexts, parseReport, profileTexts, reportChanged, shapesText, validationOptions, valueSourcesOf } from './validation-sync';
import { PROSE_WASM_MODULE, PROSE_WASM_URL, type ProseWasmModule, loadWasm } from './wasm-runtime';

export { PROSE_WASM_MODULE, PROSE_WASM_URL };

/**
 * `<ds-odrl-prose [json]="policy" />` renders ODRL JSON-LD as plain-language
 * prose using the Yew component from `prose-yew`.
 *
 * The component owns the host element's children (Yew keeps its own virtual
 * DOM there); style it through the `prose-*` classes or the `class` input.
 *
 * `[profile]` takes the text of an ODRL profile (JSON-LD, or Turtle with the
 * Turtle build, see `PROSE_WASM_MODULE`; as the host fetched it, the
 * component does no network access). The prose then uses
 * the profile's labels, and a glossary of its terms follows each policy.
 * `profileLoaded` emits the profile's summary; an unreadable profile emits
 * `profileError` and the previous profile stays.
 *
 * `[profiles]` takes several profiles at once (a list wins over `profile`).
 *
 * `[shapes]` takes the text of the dataspace's companion SHACL shapes
 * (JSON-LD, or Turtle with the Turtle build). With validation on, they add
 * to the terms the profiles declare: a value a shape's `sh:in` does not list
 * is a finding whose `expected.source` is `"shapes"`, and `sh:in` on
 * `odrl:action` or `odrl:leftOperand` closes those lists for the whole
 * policy. Only a subset of SHACL is read: `shapesLoaded` emits what was, with
 * the `warnings` for what was not; unreadable shapes emit `shapesError` and
 * the previous shapes stay.
 *
 * `[overlays]` takes the texts of the dataspace's operator overlays: small
 * annotation-only graphs of `prose:` hints (`prose:operator`,
 * `prose:rightOperand`, `prose:unit`, `prose:closed` ...) about terms the
 * profiles declare, for a dataspace that publishes no SHACL. They merge onto
 * those terms and never add or relabel one; the shapes, when given, win over
 * them. A value an overlay closes and a policy does not list is a finding
 * whose `expected.source` is `"overlay"`. `overlaysLoaded` emits one
 * `OverlaySummary` per overlay (`terms`, `warnings`); unreadable overlays emit
 * `overlaysError` and the previous overlays stay.
 *
 * `[valueSources]` takes the values the host took from the dataspace's
 * registry or catalog (participant DIDs, credential types, asset ids,
 * purposes), keyed by left operand IRI or party role (`assigner`,
 * `assignee`, `target`), each a list of values or `{ values, closed }`. They
 * sit on top of the profiles, overlays and shapes; a value a closed source
 * does not list is a finding whose `expected.source` is `"host"`, and it is
 * never dropped from the policy. `valueSourcesLoaded` emits one
 * `ValueSourceSummary` per key; unreadable sources emit `valueSourcesError`
 * and the previous sources stay.
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
  /** The ODRL profile the prose is worded with, as JSON-LD text (or Turtle, with the Turtle build). */
  readonly profile = input<string | null>(null);
  /** Several ODRL profiles, as JSON-LD (or Turtle) text each; wins over `profile`. */
  readonly profiles = input<string[] | null>(null);
  /** The text of the dataspace's companion SHACL shapes, JSON-LD (or Turtle, with the Turtle build); blank is none. */
  readonly shapes = input<string | null>(null);
  /** The texts of the dataspace's operator overlays, JSON-LD (or Turtle, with the Turtle build) each; blank texts are dropped. */
  readonly overlays = input<string[] | null>(null);
  /** Values the host took from its registry or catalog, keyed by left operand IRI or party role (`assigner`, `assignee`, `target`); each a list of values or `{ values, closed }`. */
  readonly valueSources = input<ValueSources | null>(null);
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
  /** Emits one summary per overlay, each time the `overlays` input changes to overlays that were read; `warnings` lists what was not used. */
  readonly overlaysLoaded = output<OverlaySummary[]>();
  /** Emits when an `overlays` text is not a readable overlay; the previous overlays stay. */
  readonly overlaysError = output<unknown>();
  /** Emits one summary per key of the `valueSources` input, each time it changes (or the profiles do) to sources that were read; `warnings` lists what was not used. */
  readonly valueSourcesLoaded = output<Record<string, ValueSourceSummary>>();
  /** Emits when the `valueSources` input cannot be read; the previous sources stay. */
  readonly valueSourcesError = output<unknown>();
  /** Emits what the `shapes` input holds, each time it changes to shapes that were read; `warnings` lists what was not read. */
  readonly shapesLoaded = output<ShapesSummary>();
  /** Emits when the `shapes` input is not readable shapes; the previous shapes stay. */
  readonly shapesError = output<unknown>();
  /** Emits the report when the verdict or the findings change. */
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
  private handle?: ProseHandle;
  private readonly profilesChanged = createChangeTracker<string[]>();
  private readonly handleOptionsChanged = createChangeTracker<ValidateOptions>();
  private readonly shapesChanged = createChangeTracker<string>();
  private readonly overlaysChanged = createChangeTracker<string[]>();
  // The overlays the handle read last, for the validator built beside it.
  private appliedOverlays: string[] = [];
  private readonly sourcesChanged = createChangeTracker<ValueSources>();
  // The value sources the handle read last, for the validator built beside it.
  private appliedSources: ValueSources | null = null;
  // The shapes the handle read last, for the validator built beside it.
  private appliedShapes: string | null = null;
  // The profiles the handle holds now, and the validator built from them for
  // a report the handle is not asked for.
  private appliedProfiles: string[] = [];
  private readonly standalone = new StandaloneReporter((profiles, options, shapes, overlays, sources) => {
    const validator = new this.wasm!.ProseValidator(profiles, options);
    if (shapes != null || overlays.length > 0 || sources != null) {
      try {
        if (overlays.length > 0) validator.setOverlays(overlays);
        if (shapes != null) validator.setShapes(shapes);
        if (sources != null) validator.setValueSources(sources);
      } catch (error) {
        validator.free();
        throw error;
      }
    }
    return validator;
  });
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
    const shapes = shapesText(this.shapes());
    const overlays = overlayTexts(this.overlays());
    const sources = valueSourcesOf(this.valueSources());
    const options = validationOptions(this.strict(), this.validation());
    const show = this.showViolations() && options != null;
    // Yew schedules its own work; keep it out of Angular's change detection.
    const outcome = this.zone.runOutsideAngular(() => {
      const mounted = this.handle;
      const handle = mounted ?? new this.wasm!.ProseHandle(this.host, json, cls);
      this.handle = handle;
      const out: {
        summaries?: ProfileSummary[];
        shapesSummary?: ShapesSummary;
        shapesError?: unknown;
        overlaysSummary?: OverlaySummary[];
        overlaysError?: unknown;
        valueSourcesSummary?: Record<string, ValueSourceSummary>;
        valueSourcesError?: unknown;
        profileError?: unknown;
        validationError?: unknown;
        report?: string | null;
      } = {};
      // The profiles go first, so the validation and the update below use them.
      let profilesSet = false;
      if (this.profilesChanged(texts)) {
        profilesSet = true;
        try {
          out.summaries = single
            ? [parseSummary(handle.setProfile(texts?.[0] ?? null))].filter((x) => x != null)
            : (JSON.parse(handle.setProfiles(texts ?? [])) as ProfileSummary[]);
          this.appliedProfiles = texts ?? [];
        } catch (error) {
          out.profileError = error;
        }
      }
      // The overlays add to the profiles above, and the shapes below win over them.
      if (this.overlaysChanged(overlays.length > 0 ? overlays : null)) {
        try {
          const summary = JSON.parse(handle.setOverlays(overlays)) as OverlaySummary[];
          if (summary.length > 0) out.overlaysSummary = summary;
          this.appliedOverlays = overlays;
        } catch (error) {
          out.overlaysError = error;
        }
      } else if (profilesSet && this.appliedOverlays.length > 0) {
        // Their warnings name terms no profile declares: new profiles make
        // the ones reported before stale.
        try {
          const summary = JSON.parse(handle.overlaySummaries()) as OverlaySummary[];
          if (summary.length > 0) out.overlaysSummary = summary;
        } catch (error) {
          out.overlaysError = error;
        }
      }
      // The shapes add to the profiles above; an unreadable text keeps the previous shapes.
      if (this.shapesChanged(shapes)) {
        try {
          const summary = JSON.parse(handle.setShapes(shapes)) as ShapesSummary | null;
          if (summary) out.shapesSummary = summary;
          this.appliedShapes = shapes;
        } catch (error) {
          out.shapesError = error;
        }
      }
      // The host's value sources sit on top of the rest; unreadable ones keep the previous sources.
      if (this.sourcesChanged(sources)) {
        try {
          const summary = JSON.parse(handle.setValueSources(sources)) as Record<string, ValueSourceSummary>;
          if (Object.keys(summary).length > 0) out.valueSourcesSummary = summary;
          this.appliedSources = sources;
        } catch (error) {
          out.valueSourcesError = error;
        }
      } else if (profilesSet && this.appliedSources != null) {
        // Their warnings name left operands no profile declares: new profiles make the ones reported before stale.
        try {
          const summary = JSON.parse(handle.valueSourceSummaries()) as Record<string, ValueSourceSummary>;
          if (Object.keys(summary).length > 0) out.valueSourcesSummary = summary;
        } catch (error) {
          out.valueSourcesError = error;
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
          const standalone = this.standalone.report(this.appliedProfiles, options, json, this.appliedShapes, this.appliedOverlays, this.appliedSources);
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
    if (outcome.overlaysError !== undefined) this.overlaysError.emit(outcome.overlaysError);
    if (outcome.overlaysSummary) this.overlaysLoaded.emit(outcome.overlaysSummary);
    if (outcome.shapesError !== undefined) this.shapesError.emit(outcome.shapesError);
    if (outcome.shapesSummary) this.shapesLoaded.emit(outcome.shapesSummary);
    if (outcome.valueSourcesError !== undefined) this.valueSourcesError.emit(outcome.valueSourcesError);
    if (outcome.valueSourcesSummary) this.valueSourcesLoaded.emit(outcome.valueSourcesSummary);
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
