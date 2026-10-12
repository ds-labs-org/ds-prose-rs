/**
 * A loaded profile, as `ProseHandle.setProfile` describes it. `counts` has
 * one entry per kind of term present (`action`, `leftOperand`, `operator`,
 * `rightOperand`, `conflictTerm`, ...).
 */
export interface ProfileSummary {
  iri: string | null;
  title: string | null;
  version: string | null;
  terms: number;
  counts: Record<string, number>;
  warnings: string[];
}

/**
 * A tracker of the profile text last handed to the wasm, one per component.
 * It returns true when `text` must be handed over now: only when it changed,
 * so a policy change alone does not re-read a large profile, and a profile
 * that failed is not reported again until it changes. `null`, `undefined`
 * and blank text all mean no profile, which is what a fresh mount has.
 */
export function createProfileTracker(): (text: string | null | undefined) => boolean {
  let applied: string | null = null;
  return (text) => {
    const next = text == null || text.trim() === '' ? null : text;
    if (next === applied) return false;
    applied = next;
    return true;
  };
}

/** The summary `setProfile` returns, or null for `"null"` (no profile). */
export function parseSummary(json: string): ProfileSummary | null {
  return JSON.parse(json) as ProfileSummary | null;
}
