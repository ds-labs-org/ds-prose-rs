import type { ValidateOptions } from '@ds-labs/prose-wasm';

/** What `reportChanged` reads of a `ValidationReport`. */
export interface ReportLike {
  verdict: string;
  findings?: readonly { fingerprint: string; severity?: string }[];
}

/**
 * Whether `next` is a report the host has not seen: the verdict, or the set
 * of findings (by fingerprint, with their severity), differs. Wording, order
 * and the rest of the report do not count, so a policy edit that changes
 * nothing the validator found is not an event. No report to no report is not
 * a change; a first report, or one that goes away, is.
 */
export function reportChanged(prev: ReportLike | null | undefined, next: ReportLike | null | undefined): boolean {
  if (!prev || !next) return !!prev !== !!next;
  if (prev.verdict !== next.verdict) return true;
  const a = findingKeys(prev);
  const b = findingKeys(next);
  return a.length !== b.length || a.some((k, i) => k !== b[i]);
}

function findingKeys(r: ReportLike): string[] {
  return (r.findings ?? []).map((f) => `${f.fingerprint}\u0000${f.severity ?? ''}`).sort();
}

/**
 * The options the wasm validator is given for the `strict` and `validation`
 * inputs: `validation` as given (copied) when set, since it takes precedence;
 * else strict mode when `strict` is on; else `null`, no validation.
 */
export function validationOptions(strict: boolean, validation: ValidateOptions | null | undefined): ValidateOptions | null {
  if (validation != null) return JSON.parse(JSON.stringify(validation)) as ValidateOptions;
  return strict ? { strict: true } : null;
}

/**
 * The profile texts for the `profile` and `profiles` inputs. A list wins over
 * the single profile (an empty one clears it); blank texts are dropped; `null`
 * means no profile input at all.
 */
export function profileTexts(profile: string | null | undefined, profiles: readonly string[] | null | undefined): string[] | null {
  if (profiles != null) return profiles.filter((t) => t.trim() !== '');
  return profile == null || profile.trim() === '' ? null : [profile];
}

/**
 * A tracker of the value last handed to the wasm, one per component. It
 * returns true when `value` differs from the last one (compared as JSON), so
 * an unchanged input is not handed over again. `null` and `undefined` are
 * "none", which is what a fresh mount has.
 */
export function createChangeTracker<T>(): (value: T | null | undefined) => boolean {
  let applied = 'null';
  return (value) => {
    const next = value == null ? 'null' : JSON.stringify(value);
    if (next === applied) return false;
    applied = next;
    return true;
  };
}

/** The report text `ProseHandle.report()` returns, parsed; `null` for none. */
export function parseReport<R = unknown>(text: string | null | undefined): R | null {
  return text == null ? null : (JSON.parse(text) as R);
}
