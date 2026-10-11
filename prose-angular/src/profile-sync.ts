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

/** Not implemented yet. */
export function createProfileTracker(): (text: string | null | undefined) => boolean {
  return () => false;
}

/** Not implemented yet. */
export function parseSummary(_json: string): ProfileSummary | null {
  return null;
}
