import type { EditLimits, TermInput, ValidateOptions, ValidationReport } from '@ds-labs/prose-wasm';
import type { ProfileSummary } from './profile-sync';

// No value imports from the sibling modules: `npm test` runs this file under
// Node's type stripping, which does not resolve extensionless imports.

/** As `createChangeTracker` of validation-sync: true when the value differs from the last one handed over. */
function tracker<T>(): (value: T | null | undefined) => boolean {
  let applied = 'null';
  return (value) => {
    const next = value == null ? 'null' : JSON.stringify(value);
    if (next === applied) return false;
    applied = next;
    return true;
  };
}

/** What `EditorSession` calls on a `ProseEditorHandle`. */
export interface EditorHandleLike {
  update(json: string, cls?: string | null): void;
  setProfile(text?: string | null): string;
  setProfiles(texts: string[]): string;
  setLimits(limits: EditLimits | null): void;
  setValidation(options: ValidateOptions | null): void;
  setTermInput(mode: TermInput): void;
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
  options: ValidateOptions | null;
  limits: EditLimits | null;
  /** How the term slots take their value; `'free'` (the default) when left out. */
  termInput?: TermInput;
}

/** What one `apply` found, for the component to emit. */
export interface EditorOutcome {
  summaries?: ProfileSummary[];
  jsonError?: unknown;
  profileError?: unknown;
  validationError?: unknown;
  report?: string | null;
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
  /** The handle starts with free text: `setTermInput` is called only for a change from it. */
  private termInput: TermInput = 'free';

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
    } else if (inputs.json !== this.lastJson || inputs.cls !== this.lastClass) {
      try {
        handle.update(inputs.json, inputs.cls);
        this.lastJson = inputs.json;
        this.lastClass = inputs.cls;
      } catch (error) {
        // The document on screen stays.
        out.jsonError = error;
      }
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
    if (this.limitsChanged(inputs.limits)) {
      try {
        handle.setLimits(inputs.limits ?? null);
      } catch (error) {
        // The previous limits stay; there is no output for this, a host
        // error in the limits is a programming error.
        console.error('ds-odrl-prose-editor: the limits are not readable', error);
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
