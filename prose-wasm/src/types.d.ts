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
  | "odrl.constraint-incomplete"
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
  | "heuristic.ordered-on-unordered"
  | "heuristic.single-child-group"
  | "heuristic.duplicate-action";

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
    /**
     * What gave the hint, when it was not the loaded profile: `"overlay"` for
     * an operator overlay (`setOverlays`), `"shapes"` for the companion SHACL
     * shapes (`setShapes`), `"host"` for a runtime value source the host
     * passed (`setValueSource`). Absent when the profile did. Precedence:
     * profile < overlay < shapes < host.
     */
    source?: "overlay" | "shapes" | "host";
    /** The `sh:pattern` a value did not match. */
    pattern?: string;
    /** The `sh:minInclusive` a value was below. */
    minInclusive?: string;
    /** The `sh:maxInclusive` a value was above. */
    maxInclusive?: string;
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

/** A list the editor limits: `permission` is the rule list, `policyAssigner` the assigner list of the policy, `ruleAssigner` that of a rule, and so on. */
export type LimitName =
  | "policies"
  | "permission"
  | "prohibition"
  | "obligation"
  | "duty"
  | "remedy"
  | "consequence"
  | "policyAssigner"
  | "policyAssignee"
  | "policyTarget"
  | "policyAction"
  | "policyUid"
  | "ruleAssigner"
  | "ruleAssignee"
  | "ruleTarget"
  | "ruleAction"
  | "actionRefinement"
  | "entityRefinement"
  | "constraints"
  | "logicalChildren"
  | "logicalDepth"
  | "rightOperandValues"
  | "profile"
  | "inheritFrom";

/**
 * How many items a list may hold. A given entry replaces the editor's
 * default for that list: `min` defaults to 0 and `max` to `null`
 * (unbounded). `{ max: 0 }` removes the list's add button, and hides a list
 * that is empty. On `policyUid` it hides the policy's identifier (an empty
 * one is not shown, a present one is read-only) and a new policy gets none.
 * The `odrl.policy-uid` validation rule still fires on a policy without an
 * identifier: a host that caps `policyUid` at 0 sets it to `"off"` in
 * `ValidateOptions.rules`.
 *
 * `logicalDepth` is not a list: its `max` is how deep `and`/`or`/`xone` groups
 * may nest (`{ max: 2 }`: a group may hold a group, and no deeper; `{ max: 0 }`
 * allows none). It limits adding and grouping, and the buttons go; a document
 * that already nests deeper is kept as written, and `min` is not used.
 */
export interface LimitSpec {
  min?: number;
  max?: number | null;
}

/** The lists to limit, by name; a list not named keeps its default. */
export type EditLimits = Partial<Record<LimitName, LimitSpec>>;

/**
 * How the editor's term slots take their value (`setTermInput`): `"free"`
 * (the default) is typed text with suggestions; `"select"` is a dropdown of
 * the terms the loaded profiles define.
 */
export type TermInput = "free" | "select";

/**
 * What the editor does with a section whose limit is `max: 0` when a
 * document holds one (`setOutOfScope`): `"show"` (the default) keeps and
 * shows it; `"strip"` drops it on load and reports it (`removed()`);
 * `"reject"` refuses the document with an `OutOfScopeError` whose `paths`
 * name the offending sections.
 */
export type OutOfScopeMode = "show" | "strip" | "reject";

/** A section the editor dropped under `"strip"`. */
export interface RemovedSection {
  /** Where it sat: keys and indexes from the document root, such as `permission[0].duty[1].assigner`. */
  path: string;
  /** The key as written: `assigner`, `odrl:assigner` or a full IRI. */
  key: string;
  /** The `EditLimits` name of the list that left it out, such as `ruleAssigner`. */
  limit: LimitName;
}

/**
 * What `setShapes` returns for loaded companion SHACL shapes. Only a subset of
 * SHACL is read: a property shape on `odrl:rightOperand` in a constraint shape
 * selected by `odrl:leftOperand sh:hasValue X` (`sh:in`, `sh:class`, `sh:node`,
 * `sh:datatype`, `sh:pattern`, `sh:minInclusive`, `sh:maxInclusive`), `sh:in` on
 * `odrl:operator` and `odrl:unit` there, and `sh:in` on `odrl:action` or
 * `odrl:leftOperand`. Everything else is ignored and listed in `warnings`.
 */
export interface ShapesSummary {
  /** How many left operands a shape selects. */
  constraints: number;
  /** The actions (expanded IRIs) the shapes close the policy to; `null` when they do not. */
  actions: string[] | null;
  /** The left operands the shapes close the policy to; `null` when they do not. */
  leftOperands: string[] | null;
  /**
   * What was not read, or read with a guess, then what the shapes say that
   * the loaded profiles make unusable: an `sh:class` with more members (or a
   * deeper hierarchy) than the limits keep closes nothing. That second part
   * depends on the profiles, so it is only complete once they are loaded
   * (`ProseHandle.shapesSummary()` and `ProseEditorHandle.shapesSummary()`
   * return it again for the profiles loaded now, and `shapesLoaded` of the
   * Angular components emits it again when the profiles change).
   */
  warnings: string[];
}

/**
 * What `setOverlays` returns, as a list with one entry per overlay. An overlay
 * is an annotation-only graph of `prose:` triples (`prose:operator`,
 * `prose:rightOperand`, `prose:unit`, `prose:datatype`, `prose:closed`,
 * `prose:plural`) about terms the loaded profiles declare. It merges onto
 * those terms and never adds or relabels one.
 */
export interface OverlaySummary {
  /** How many terms the overlay names. */
  terms: number;
  /** What was not read, or cannot be used: a term no loaded profile declares as a left operand is ignored. */
  warnings: string[];
}

/**
 * One value of a runtime value source: a string (what is written into the
 * policy, shown as it is), or an object with a `label` and a `group` the
 * editor shows it under (an `<optgroup>` heading in a select).
 */
export type ValueSourceValue = string | { value: string; label?: string; group?: string };

/**
 * Runtime value sources (`setValueSource`, `setValueSources`): values the
 * host takes from the dataspace's registry or catalog (participant DIDs,
 * credential types, asset ids, purposes), passed per left operand (the key is
 * its IRI) or per party role (`"assigner"`, `"assignee"`, `"target"`; the
 * target is the asset). An entry is a list, which only suggests, or
 * `{ values, closed }`. The editor never fetches: the host does, and caches.
 *
 * Precedence: profile < overlay < shapes < host, for the values alone. A
 * closed source binds in strict validation, and a finding names `"host"` as
 * its source. A value the source no longer lists stays in the document and is
 * flagged; it is never dropped. An empty list is no source.
 */
export type ValueSources = Record<
  string,
  ValueSourceValue[] | { values: ValueSourceValue[]; closed?: boolean }
>;

/** What `setValueSource` returns as JSON text (and `setValueSources`, one per key, in an object). */
export interface ValueSourceSummary {
  /** How many values the source lists. */
  values: number;
  closed: boolean;
  /** What cannot be used: a left operand no loaded profile declares is ignored. */
  warnings: string[];
}
