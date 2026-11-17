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
import type { EditLimits, OutOfScopeMode, OverlaySummary, ProseEditorHandle, RemovedSection, ShapesSummary, TermInput, ValidateOptions, ValidationReport, ValueSourceSummary, ValueSources } from '@ds-labs/prose-wasm';
import { type EditorInputs, EditorSession, resolvePreset } from './editor-sync';
import type { ProfileSummary } from './profile-sync';
import { parseReport, profileTexts, reportChanged } from './validation-sync';
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
 * `ds-odrl-prose`. `overlays` takes the texts of the dataspace's operator
 * overlays (annotation-only graphs of `prose:` hints about terms the profiles
 * declare, for a dataspace that publishes no SHACL): they merge onto those
 * terms like shapes do, never add or relabel one, and the shapes win over
 * them; `overlaysLoaded` emits one summary per overlay. `shapes` takes the text of the dataspace's companion SHACL
 * shapes (JSON-LD, or Turtle with the Turtle build): they add to the terms
 * the profiles declare, so a right operand a shape closes with `sh:in` is a
 * dropdown (with `termInput="select"`) and any other value is an error that
 * names the shapes; `sh:in` on `odrl:action` or `odrl:leftOperand` closes
 * those lists for the whole policy. Only a subset of SHACL is read:
 * `shapesLoaded` emits what was, with the `warnings` for what was not.
 * `valueSources` takes the values the host took from the dataspace's registry
 * or catalog (participant DIDs, credential types, asset ids, purposes), by
 * left operand IRI or party role (`assigner`, `assignee`, `target`): a list
 * of strings or `{ value, label, group }` (which only suggests), or
 * `{ values, closed: true }`, which makes a right operand a dropdown of
 * exactly those values, grouped under their `group`. The editor never
 * fetches: the host does, and caches. A value the source no longer lists
 * stays in the document and is flagged; it is never dropped.
 * `valueSourcesLoaded` emits one summary per key. `limits` sets how many items each list may hold
 * (`{ policyAssigner: { max: 0 } }` removes the assigner button; a list
 * capped at 0 is not shown, and `{ policyUid: { max: 0 } }` hides the policy
 * identifier). When
 * validation is on (`validation`, or `strict`) the editor marks issues inline
 * and shows a problems panel with fix buttons; edits are flagged, never
 * refused. `outOfScope` turns a limit of `max: 0` into the scope of the
 * incoming document: `'strip'` drops those sections from `json` (parties
 * of a policy definition, say) in any spelling of the key, and
 * `outOfScopeRemoved` emits what went; `jsonChange` never holds them.
 * `'reject'` refuses a document that holds one: `jsonError` emits, with the
 * offending `paths`. `'show'` (the default) keeps them.
 * `preset="dataspace"` turns on what a dataspace asks of its members in one
 * switch (see `preset`); each input you set yourself still wins.
 * `termInput="select"` turns the term slots (action, left operand,
 * policy type, and a unit or right operand the profile lists entries for)
 * into dropdowns of the loaded profiles' terms. `validationChange` emits the report when the verdict or the
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
  /**
   * The texts of the dataspace's operator overlays, JSON-LD (or Turtle, with
   * the Turtle build) each; blank texts are dropped. They narrow what the
   * loaded profiles declare and never relabel a term; the shapes win over them.
   */
  readonly overlays = input<string[] | null>(null);
  /**
   * Values the host took from its registry or catalog, keyed by left operand
   * IRI or by party role (`assigner`, `assignee`, `target`); each is a list of
   * values (suggestions) or `{ values, closed }`. They sit on top of the
   * profiles, overlays and shapes. A value a closed source does not list is
   * kept and flagged.
   */
  readonly valueSources = input<ValueSources | null>(null);
  /**
   * The text of the dataspace's companion SHACL shapes, JSON-LD (or Turtle,
   * with the Turtle build); blank is none. They narrow what the loaded
   * profiles declare and never relabel a term.
   */
  readonly shapes = input<string | null>(null);
  /** How many items each list may hold; a list not named keeps its default. */
  readonly limits = input<EditLimits | null>(null);
  /** Validate with a closed vocabulary: ODRL plus the loaded profiles. Default `false`; with a `preset`, the preset's. */
  readonly strict = input<boolean | undefined>(undefined);
  /** The full validation options; they take precedence over `strict`, key by key over a `preset`'s. */
  readonly validation = input<ValidateOptions | null>(null);
  /** `'select'`: the term slots are dropdowns of the profile's terms instead of free text. Default `'free'`; with a `preset`, the preset's. */
  readonly termInput = input<TermInput | undefined>(undefined);
  /**
   * `true`: an edit that writes a term the validation does not allow (see
   * `strict`) is refused, not committed and flagged. Default `false`; with a
   * `preset`, the preset's.
   */
  readonly refuseTerms = input<boolean | undefined>(undefined);
  /**
   * `'dataspace'` sets, for a dataspace whose members write only policies it
   * can evaluate: strict validation (a closed vocabulary) with the structural
   * tier on, `termInput` `'select'`, `refuseTerms`, `limits` of
   * `{ logicalDepth: { max: 2 } }`, and warnings for a group of one condition
   * and an action listed twice. Each of `strict`, `validation`, `termInput`,
   * `refuseTerms` and `limits` you set yourself still wins, key by key. Null
   * (the default) sets nothing.
   */
  readonly preset = input<'dataspace' | null>(null);
  /**
   * What to do with a section whose limit is `max: 0` when `json` holds one:
   * `'show'` keeps it, `'strip'` drops it (see `outOfScopeRemoved`), `'reject'`
   * fails the load through `jsonError` (an `OutOfScopeError` with `paths`).
   */
  readonly outOfScope = input<OutOfScopeMode>('show');
  /** Emits the JSON-LD text the editor writes after each committed edit. */
  readonly jsonChange = output<string>();
  /** Emits what `outOfScope="strip"` dropped from a document, each time something was. */
  readonly outOfScopeRemoved = output<RemovedSection[]>();
  /** Emits when `json` is not a readable ODRL policy, or `outOfScope="reject"` refuses it. */
  readonly jsonError = output<unknown>();
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
  private readonly session = new EditorSession<ProseEditorHandle>({
    // Both run inside the wasm call of an edit, back in Angular's zone.
    publish: (report) => this.publish(report),
    emit: (text) => this.jsonChange.emit(text),
  });

  ngOnChanges(): void {
    void this.sync();
  }

  ngOnDestroy(): void {
    this.session.destroy();
  }

  private async sync(): Promise<void> {
    try {
      this.wasm = await loadWasm({ module: this.wasmModule, url: this.wasmUrl });
    } catch (error) {
      if (!this.session.destroyed) this.showLoadError(error);
      return;
    }
    if (this.session.destroyed) return;
    // Only the error message can be in the host before the first mount.
    if (!this.session.handle) this.host.replaceChildren();
    let resolved: ReturnType<typeof resolvePreset>;
    try {
      resolved = resolvePreset(this.preset(), {
        strict: this.strict(),
        validation: this.validation(),
        termInput: this.termInput(),
        limits: this.limits(),
        refuseTerms: this.refuseTerms(),
      });
    } catch (error) {
      // A preset the editor does not know is ignored; the host's inputs stand.
      console.error('ds-odrl-prose-editor: the preset is not readable', error);
      resolved = resolvePreset(null, {
        strict: this.strict(),
        validation: this.validation(),
        termInput: this.termInput(),
        limits: this.limits(),
        refuseTerms: this.refuseTerms(),
      });
    }
    const inputs: EditorInputs = {
      json: this.json(),
      cls: this.articleClass() ?? null,
      single: this.profiles() == null,
      texts: profileTexts(this.profile(), this.profiles()),
      overlays: this.overlays(),
      shapes: this.shapes(),
      valueSources: this.valueSources(),
      options: resolved.options,
      limits: resolved.limits,
      termInput: resolved.termInput,
      outOfScope: this.outOfScope(),
      refuseTerms: resolved.refuseTerms,
    };
    // Yew schedules its own work; keep it out of Angular's change detection.
    const outcome = this.zone.runOutsideAngular(() =>
      this.session.apply(
        inputs,
        (json, cls, onChange) =>
          new this.wasm!.ProseEditorHandle(this.host, json, cls, (text, report) => this.zone.run(() => onChange(text, report))),
      ),
    );
    if (!outcome) return;
    if (outcome.jsonError !== undefined) this.jsonError.emit(outcome.jsonError);
    if (outcome.removed) this.outOfScopeRemoved.emit(outcome.removed);
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
    console.error('ds-odrl-prose-editor: could not load the wasm module', error);
    // A mounted Yew tree owns the host's children: leave it alone.
    if (!this.session.handle) {
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
