import type { EditLimits, OutOfScopeMode, OverlaySummary, RemovedSection, ShapesSummary, TermInput, ValidateOptions, ValidationReport } from '@ds-labs/prose-wasm';
import type { ProfileSummary } from './profile-sync';

// No value imports from the sibling modules: `npm test` runs this file under
// Node's type stripping, which does not resolve extensionless imports.

/** As `createChangeTracker` of validation-sync: true when the value differs from the last one handed over. */
function tracker<T>(): ((value: T | null | undefined) => boolean) & { revert(): void } {
  let applied = 'null';
  let before = applied;
  const changed = (value: T | null | undefined): boolean => {
    const next = value == null ? 'null' : JSON.stringify(value);
    if (next === applied) return false;
    before = applied;
    applied = next;
    return true;
  };
  /** The last change did not take: the value is handed over again next time. */
  changed.revert = () => {
    applied = before;
  };
  return changed;
}

/** What the wasm handle throws for a document, or limits, its scope refuses. */
function isOutOfScope(error: unknown): boolean {
  return (error as { name?: unknown } | null)?.name === 'OutOfScopeError';
}

/** What `EditorSession` calls on a `ProseEditorHandle`. */
export interface EditorHandleLike {
  update(json: string, cls?: string | null): void;
  setProfile(text?: string | null): string;
  setProfiles(texts: string[]): string;
  /** Operator overlays (one text each) or none (`[]`); returns the summaries as JSON text, one per overlay. */
  setOverlays(texts: string[]): string;
  /** Companion SHACL shapes (text) or none; returns the summary as JSON text, `"null"` after clearing. */
  setShapes(text: string | null): string;
  setLimits(limits: EditLimits | null): void;
  setValidation(options: ValidateOptions | null): void;
  setTermInput(mode: TermInput): void;
  setRefuseTerms(on: boolean): void;
  setOutOfScope(mode: OutOfScopeMode): void;
  /** What the last load took out, as JSON text. */
  removed(): string;
  report(): string | null;
  destroy(): void;
  free(): void;
}

/** Where a committed edit goes: the report first, then the text. */
export interface EditorSink {
  publish(report: ValidationReport | null): void;
  emit(text: string): void;
}

/** The inputs of one `apply`, already normalised by the component. */
export interface EditorInputs {
  json: string;
  cls: string | null;
  /** The `profile` input is used (not `profiles`): load it with `setProfile`. */
  single: boolean;
  texts: string[] | null;
  /** The text of companion SHACL shapes, or none; blank text is none. */
  shapes?: string | null;
  /** The texts of operator overlays, or none; blank texts are dropped. */
  overlays?: readonly string[] | null;
  options: ValidateOptions | null;
  limits: EditLimits | null;
  /** How the term slots take their value; `'free'` (the default) when left out. */
  termInput?: TermInput;
  /** What to do with a section whose limit is `max: 0`; `'show'` (the default) when left out. */
  outOfScope?: OutOfScopeMode;
  /** Refuse an edit that writes a term the validation does not allow, rather than flag it; `false` (the default) when left out. */
  refuseTerms?: boolean;
}

/** What one `apply` found, for the component to emit. */
export interface EditorOutcome {
  summaries?: ProfileSummary[];
  /** What the overlays of this `apply` hold, one summary each, for the component to emit; absent when nothing changed or they were cleared. */
  overlaysSummary?: OverlaySummary[];
  /** What the shapes of this `apply` hold, for the component to emit; absent when nothing changed or they were cleared. */
  shapesSummary?: ShapesSummary;
  jsonError?: unknown;
  profileError?: unknown;
  /** The overlays could not be read; the previous overlays stay. */
  overlaysError?: unknown;
  /** The shapes could not be read; the previous shapes stay. */
  shapesError?: unknown;
  validationError?: unknown;
  report?: string | null;
  /** What the scope took out of the document in this `apply` (`outOfScope: 'strip'`); absent when nothing. */
  removed?: RemovedSection[];
}

/**
 * The decisions of `ds-odrl-prose-editor` without Angular: when to mount, to
 * update or to skip (the echo of an emitted text), which input to hand over
 * (each only when it changed), what a failed mount leaves, what a change
 * does, and what a destroy releases. The component owns the wasm loading, the
 * zone and the outputs.
 */
export class EditorSession<H extends EditorHandleLike = EditorHandleLike> {
  handle?: H;
  /** The text the handle holds as far as the session knows: the last one given to it or emitted by it. */
  lastJson?: string;
  lastClass: string | null = null;
  destroyed = false;
  /** `to.publish` and `to.emit` run in Angular's zone: the component supplies them. */
  private readonly to: EditorSink;

  constructor(to?: EditorSink) {
    this.to = to ?? { publish: () => {}, emit: () => {} };
  }

  private readonly profilesChanged = tracker<string[]>();
  private readonly optionsChanged = tracker<ValidateOptions>();
  private readonly limitsChanged = tracker<EditLimits>();
  private readonly overlaysChanged = tracker<string[]>();
  private readonly shapesChanged = tracker<string>();
  /** The handle starts with `'show'`: `setOutOfScope` is called only for a change from it. */
  private outOfScope: OutOfScopeMode = 'show';
  /** The handle starts with free text: `setTermInput` is called only for a change from it. */
  private termInput: TermInput = 'free';
  /** The handle starts flagging: `setRefuseTerms` is called only for a change from it. */
  private refuseTerms = false;

  /**
   * Brings the handle in line with the inputs. `mount` makes the handle, and
   * throws on an unreadable `json`: nothing is mounted then, no other input
   * is consumed, and the next `apply` starts again. Returns `null` after
   * `destroy()`.
   */
  apply(inputs: EditorInputs, mount: (json: string, cls: string | null, onChange: (text: string, report: string | null) => void) => H): EditorOutcome | null {
    if (this.destroyed) return null;
    const out: EditorOutcome = {};
    let handle = this.handle;
    if (!handle) {
      try {
        handle = mount(inputs.json, inputs.cls, (text, report) => this.changed(text, report));
      } catch (error) {
        out.jsonError = error;
        return out;
      }
      this.handle = handle;
      this.lastJson = inputs.json;
      this.lastClass = inputs.cls;
      // The scope is part of loading the document: a document it refuses is not mounted.
      const refused = this.scope(handle, inputs, out);
      if (refused) {
        handle.destroy();
        handle.free();
        this.handle = undefined;
        this.lastJson = undefined;
        this.lastClass = null;
        this.limitsChanged.revert();
        this.outOfScope = 'show';
        out.jsonError = refused.error;
        return out;
      }
    } else if (inputs.json !== this.lastJson || inputs.cls !== this.lastClass) {
      try {
        handle.update(inputs.json, inputs.cls);
        this.lastJson = inputs.json;
        this.lastClass = inputs.cls;
        this.collect(handle, out);
      } catch (error) {
        // The document on screen stays.
        out.jsonError = error;
      }
      const refused = this.scope(handle, inputs, out);
      if (refused) out.jsonError ??= refused.error;
    } else {
      const refused = this.scope(handle, inputs, out);
      if (refused) out.jsonError = refused.error;
    }
    // The profiles go first, so the validation below uses them.
    if (this.profilesChanged(inputs.texts)) {
      try {
        out.summaries = inputs.single
          ? [JSON.parse(handle.setProfile(inputs.texts?.[0] ?? null)) as ProfileSummary | null].filter((x) => x != null)
          : (JSON.parse(handle.setProfiles(inputs.texts ?? [])) as ProfileSummary[]);
      } catch (error) {
        out.profileError = error;
      }
    }
    // The overlays and the shapes follow the profiles they add hints to (the
    // order does not matter to the wasm; the shapes win over an overlay), and
    // come before the validation that reads them.
    const overlays = inputs.overlays == null ? [] : inputs.overlays.filter((t) => t.trim() !== '');
    if (this.overlaysChanged(overlays.length > 0 ? overlays : null)) {
      try {
        const summary = JSON.parse(handle.setOverlays(overlays)) as OverlaySummary[];
        if (summary.length > 0) out.overlaysSummary = summary;
      } catch (error) {
        out.overlaysError = error;
      }
    }
    const shapes = inputs.shapes == null || inputs.shapes.trim() === '' ? null : inputs.shapes;
    if (this.shapesChanged(shapes)) {
      try {
        const summary = JSON.parse(handle.setShapes(shapes)) as ShapesSummary | null;
        if (summary) out.shapesSummary = summary;
      } catch (error) {
        out.shapesError = error;
      }
    }
    const termInput = inputs.termInput ?? 'free';
    if (termInput !== this.termInput) {
      try {
        handle.setTermInput(termInput);
        this.termInput = termInput;
      } catch (error) {
        // The previous mode stays; as for the limits, a wrong name is a
        // programming error of the host.
        console.error('ds-odrl-prose-editor: the termInput is not readable', error);
      }
    }
    const refuseTerms = inputs.refuseTerms ?? false;
    if (refuseTerms !== this.refuseTerms) {
      try {
        handle.setRefuseTerms(refuseTerms);
        this.refuseTerms = refuseTerms;
      } catch (error) {
        console.error('ds-odrl-prose-editor: refuseTerms could not be set', error);
      }
    }
    if (this.optionsChanged(inputs.options)) {
      try {
        handle.setValidation(inputs.options);
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
  }

  /**
   * Hands over the scope: the mode, then the limits (which are what strips or
   * refuses). Returns the refusal when the handle says the document is out of
   * scope; the mode or the limits that caused it are tried again on the next
   * `apply`. Any other failure is a programming error of the host.
   */
  private scope(handle: H, inputs: EditorInputs, out: EditorOutcome): { error: unknown } | undefined {
    const mode = inputs.outOfScope ?? 'show';
    if (mode !== this.outOfScope) {
      try {
        handle.setOutOfScope(mode);
        this.outOfScope = mode;
        this.collect(handle, out);
      } catch (error) {
        if (isOutOfScope(error)) return { error };
        // The previous mode stays; a wrong name is a programming error of the host.
        console.error('ds-odrl-prose-editor: the outOfScope is not readable', error);
      }
    }
    if (this.limitsChanged(inputs.limits)) {
      try {
        handle.setLimits(inputs.limits ?? null);
        this.collect(handle, out);
      } catch (error) {
        if (isOutOfScope(error)) {
          this.limitsChanged.revert();
          return { error };
        }
        // The previous limits stay; there is no output for this, a host
        // error in the limits is a programming error.
        console.error('ds-odrl-prose-editor: the limits are not readable', error);
      }
    }
    return undefined;
  }

  /** Adds what the last load took out to the outcome. Read only while a scope is in effect. */
  private collect(handle: H, out: EditorOutcome): void {
    if (this.outOfScope === 'show') return;
    const removed = JSON.parse(handle.removed()) as RemovedSection[];
    if (removed.length > 0) out.removed = [...(out.removed ?? []), ...removed];
  }

  /**
   * The editor committed an edit: keep the text, publish the report, then
   * emit the text. Runs inside the wasm call, so it must not call the handle.
   */
  changed(text: string, report: string | null): void {
    this.lastJson = text;
    this.to.publish(report == null ? null : (JSON.parse(report) as ValidationReport));
    this.to.emit(text);
  }

  /** Releases the handle, if one is mounted. Safe to call more than once. */
  destroy(): void {
    this.destroyed = true;
    this.handle?.destroy();
    this.handle?.free();
    this.handle = undefined;
  }
}

/** What a preset sets: the editor's inputs, each of which a host can still set itself. */
export interface Preset {
  validation: ValidateOptions;
  termInput: TermInput;
  limits: EditLimits;
  refuseTerms: boolean;
}

/**
 * `preset="dataspace"`, for a dataspace whose members write only policies the
 * dataspace can evaluate: validation is strict (a closed vocabulary: ODRL
 * plus the loaded profiles) with the structural tier on, so a missing target
 * or action is an error too; the term slots are dropdowns; an edit that
 * writes a term the profiles do not define is refused; groups nest two deep
 * at most; and the lints for a group of one condition and an action listed
 * twice are warnings. The values the profile and the shapes allow come from
 * the loaded profiles, not from here.
 */
export const DATASPACE_PRESET: Preset = {
  validation: {
    strict: true,
    structural: true,
    rules: {
      'heuristic.single-child-group': 'warning',
      'heuristic.duplicate-action': 'warning',
    },
  },
  termInput: 'select',
  limits: { logicalDepth: { max: 2 } },
  refuseTerms: true,
};

const PRESETS: Record<string, Preset> = { dataspace: DATASPACE_PRESET };

/** The inputs of the editor that a preset sets, as the host gave them: `undefined` or `null` is not given. */
export interface PresetInputs {
  strict: boolean | undefined;
  validation: ValidateOptions | null | undefined;
  termInput: TermInput | undefined;
  limits: EditLimits | null | undefined;
  refuseTerms: boolean | undefined;
}

/** What the editor is given once the preset and the host's own inputs are laid together. */
export interface ResolvedInputs {
  options: ValidateOptions | null;
  termInput: TermInput;
  limits: EditLimits | null;
  refuseTerms: boolean;
}

/**
 * The inputs the editor uses: the preset's values, over which each input the
 * host gave wins. `strict` sets `options.strict`; `validation` wins key by key,
 * its `rules` added to the preset's; `limits` win list by list. Without a
 * preset, `strict` is `{ strict: true }` and `validation` stands alone, as
 * before. Throws for a preset it does not know. Returns copies.
 */
export function resolvePreset(preset: string | null | undefined, given: PresetInputs): ResolvedInputs {
  const base = preset == null ? undefined : PRESETS[preset];
  if (preset != null && !base) throw new Error(`preset must be one of ${Object.keys(PRESETS).join(', ')}, not ${JSON.stringify(preset)}`);
  const copy = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T;
  let options: ValidateOptions | null;
  if (!base) {
    options = given.validation != null ? copy(given.validation) : given.strict ? { strict: true } : null;
  } else {
    const own = given.validation != null ? copy(given.validation) : {};
    const rules: NonNullable<ValidateOptions['rules']> = { ...base.validation.rules, ...own.rules };
    options = { ...copy(base.validation), ...own, rules };
    if (given.strict !== undefined && own.strict === undefined) options.strict = given.strict;
  }
  const limits = base || given.limits != null ? { ...copy(base?.limits ?? {}), ...copy(given.limits ?? {}) } : null;
  return {
    options,
    termInput: given.termInput ?? base?.termInput ?? 'free',
    limits: limits && Object.keys(limits).length > 0 ? limits : null,
    refuseTerms: given.refuseTerms ?? base?.refuseTerms ?? false,
  };
}
