/** How loud a rule is. `off` switches it off. */
export type RuleLevel = "off" | "info" | "warning" | "error";

/** The severity of a finding. */
export type Severity = "error" | "warning" | "info";

/**
 * The outcome of a validation. `incomplete` means no error was found but
 * something could not be checked, such as a declared profile that is not
 * loaded: it is never a pass.
 */
export type Verdict = "conforms" | "violations" | "incomplete";

/** A rule's frozen id. The catalogue is `rules()`. */
export type RuleId =
  | "profile.not-loaded"
  | "profile.import-not-loaded"
  | "profile.undeclared-in-namespace"
  | "profile.unknown-term"
  | "profile.wrong-kind"
  | "profile.not-declared"
  | "profile.conflict-term"
  | "profile.unbound-prefix"
  | "profile.deprecated-term"
  | "odrl.policy-uid"
  | "odrl.no-rule"
  | "odrl.offer-assigner"
  | "odrl.agreement-parties"
  | "odrl.rule-target"
  | "odrl.rule-action"
  | "odrl.constraint-shape"
  | "odrl.set-operator-cardinality"
  | "odrl.consequence-nesting"
  | "odrl.inheritance-cycle"
  | "prose.operator-not-allowed"
  | "prose.value-not-allowed"
  | "prose.iri-expected"
  | "prose.unit-not-allowed"
  | "prose.datatype-lexical"
  | "prose.datatype-conflict"
  | "prose.ill-typed-literal"
  | "input.unreadable"
  | "input.unchecked"
  | "input.reader-note"
  | "heuristic.ordered-on-unordered";

/**
 * What to check and how loudly. Every field is optional; an options object
 * of `{}` is strict mode with the profile and prose rules.
 */
export interface ValidateOptions {
  /** Strict (the default) reads the vocabulary as closed; `false` is the advisory reading. */
  strict?: boolean;
  /** Also check the Information Model's structural MUSTs (`odrl.*`). Off by default. */
  structural?: boolean;
  /** Make what could not be checked an error rather than a gap. */
  failClosed?: boolean;
  /** Make a deprecated term an error rather than a warning. */
  denyDeprecated?: boolean;
  /** The ODRL terms defined without a profile: the Core and Common vocabularies (default), or Core only. */
  odrl?: "common" | "core";
  /** Check only the rules of this ruleset and earlier, so a pinned check does not change with a release. */
  ruleset?: number;
  /** A level per rule, over everything else. `input.unreadable` stays an error. */
  rules?: Partial<Record<RuleId, RuleLevel>>;
}

/** A change that would clear a finding. */
export interface Fix {
  label: string;
  /** Unambiguous: the only reasonable reading of what was meant. */
  safe: boolean;
  /** The text to write instead. */
  replacement: string;
}

/** One thing the validator found. */
export interface Finding {
  rule: RuleId;
  tier: "profile" | "odrl" | "prose" | "input" | "heuristic";
  basis: "standard" | "standard-derived" | "convention" | "heuristic" | "input";
  severity: Severity;
  message: string;
  target: { kind: "doc" | "node" | "slot" | "list"; path: string };
  /** An RFC 6901 pointer into the input; not filled in yet. */
  pointer: string | null;
  term: { written: string; iri: string } | null;
  /** The IRI of the profile the finding is about. */
  profile: string | null;
  expected: {
    kinds: string[];
    operators: string[];
    /** At most 8 of the allowed values. */
    values: string[];
    /** How many values the profile allows; present only when `values` was cut short. */
    valuesTotal?: number;
    units: string[];
    datatype: string | null;
    closed: boolean;
  } | null;
  fixes: Fix[];
  /** A short stable name for the finding; reformatting the input does not change it. */
  fingerprint: string;
}

/** The report, schema `ds-prose/validation-report@1`. */
export interface ValidationReport {
  schema: "ds-prose/validation-report@1";
  tool: { name: string; version: string };
  mode: "strict" | "advisory";
  tiers: string[];
  ruleset: number;
  verdict: Verdict;
  vocabulary: {
    odrl: "common" | "core";
    profiles: { iri: string | null; title: string | null; version: string | null }[];
  };
  /** What could not be checked. */
  gaps: { rule: RuleId; detail: string }[];
  counts: { error: number; warning: number; info: number };
  findings: Finding[];
}

/** One entry of `rules()`. */
export interface RuleInfo {
  id: RuleId;
  tier: "profile" | "odrl" | "prose" | "input" | "heuristic";
  title: string;
  basis: "standard" | "standard-derived" | "convention" | "heuristic" | "input";
  /** Where the rule comes from, e.g. a section of the ODRL Information Model. */
  source: string;
  sinceRuleset: number;
  advisory: RuleLevel;
  strict: RuleLevel;
  /** The strict level on a `prose:closed` left operand, where it differs. */
  strictClosed: RuleLevel | null;
  /** A finding is a gap: it makes the verdict incomplete, not a violation. */
  gap: boolean;
  /** Whether an editor may refuse an edit that makes it. */
  refusable: boolean;
}
