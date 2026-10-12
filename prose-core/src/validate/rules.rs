//! The rule catalogue: every finding the validator can make, which tier it
//! belongs to, the authority behind it and its level in each mode.
use std::fmt;
use std::str::FromStr;

/// How loud a rule is. `Off` rules make no finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Level {
    Off,
    Info,
    Warning,
    Error,
}

/// The family a rule belongs to; its id starts with the tier's name.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tier {
    /// Vocabulary closure: ODRL IM §2.1 and §3.2 read under a closed world.
    Profile,
    /// Structural requirements the ODRL Information Model states as MUST.
    Odrl,
    /// ds-prose conventions, such as the `prose:` hints.
    Prose,
    /// What the reader could not read or check.
    Input,
    /// Likely mistakes no standard rules out; opt-in only.
    Heuristic,
}

/// The authority a rule rests on.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Basis {
    /// A requirement the standard states.
    Standard,
    /// A requirement the standard implies once the vocabulary is closed.
    StandardDerived,
    /// A ds-prose convention, not a standard.
    Convention,
    Heuristic,
    /// About the input, not the policy.
    Input,
}

/// One rule of the catalogue. Ids are frozen once released; see
/// [`RuleId::as_str`] for the id each variant has.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RuleId {
    ProfileNotLoaded,
    ProfileImportNotLoaded,
    ProfileUndeclaredInNamespace,
    ProfileUnknownTerm,
    ProfileWrongKind,
    ProfileNotDeclared,
    ProfileConflictTerm,
    ProfileUnboundPrefix,
    ProfileDeprecatedTerm,
    OdrlPolicyUid,
    OdrlNoRule,
    OdrlOfferAssigner,
    OdrlAgreementParties,
    OdrlRuleTarget,
    OdrlRuleAction,
    OdrlConstraintShape,
    OdrlSetOperatorCardinality,
    OdrlConsequenceNesting,
    OdrlInheritanceCycle,
    ProseOperatorNotAllowed,
    ProseValueNotAllowed,
    ProseIriExpected,
    ProseUnitNotAllowed,
    ProseDatatypeLexical,
    ProseDatatypeConflict,
    ProseIllTypedLiteral,
    InputUnreadable,
    InputUnchecked,
    InputReaderNote,
    HeuristicOrderedOnUnordered,
}

impl RuleId {
    /// The rule's frozen id, e.g. `profile.not-loaded`.
    pub fn as_str(self) -> &'static str {
        self.info().id_str
    }

    pub fn tier(self) -> Tier {
        self.info().tier
    }

    /// The rule's catalogue entry.
    pub fn info(self) -> &'static RuleInfo {
        RULES
            .iter()
            .find(|r| r.id == self)
            .expect("every rule id is in RULES")
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A text that names no rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownRule(pub String);

impl fmt::Display for UnknownRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "no rule is called \"{}\"", self.0)
    }
}

impl std::error::Error for UnknownRule {}

impl FromStr for RuleId {
    type Err = UnknownRule;
    fn from_str(s: &str) -> Result<RuleId, UnknownRule> {
        RULES
            .iter()
            .find(|r| r.id_str == s)
            .map(|r| r.id)
            .ok_or_else(|| UnknownRule(s.to_string()))
    }
}

/// What the catalogue says about one rule.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleInfo {
    pub id: RuleId,
    id_str: &'static str,
    pub tier: Tier,
    pub title: &'static str,
    pub basis: Basis,
    /// Where the rule comes from, e.g. a section of the ODRL Information
    /// Model.
    pub source: &'static str,
    /// The first ruleset the rule is in; see [`super::Options::ruleset`].
    pub since_ruleset: u32,
    pub advisory: Level,
    pub strict: Level,
    /// The strict level on a `prose:closed` left operand, where it differs.
    pub strict_closed: Option<Level>,
    /// A finding of this rule is a gap: something left unchecked, which
    /// makes the verdict incomplete rather than a violation.
    pub gap: bool,
    /// Whether an editor may refuse an edit that makes this finding.
    pub refusable: bool,
}

impl RuleInfo {
    #[allow(clippy::too_many_arguments)]
    const fn new(
        id: RuleId,
        id_str: &'static str,
        tier: Tier,
        title: &'static str,
        basis: Basis,
        source: &'static str,
        levels: (Level, Level, Option<Level>),
        gap: bool,
        refusable: bool,
    ) -> RuleInfo {
        RuleInfo {
            id,
            id_str,
            tier,
            title,
            basis,
            source,
            since_ruleset: 1,
            advisory: levels.0,
            strict: levels.1,
            strict_closed: levels.2,
            gap,
            refusable,
        }
    }
}

/// The ruleset this build implements.
pub const RULESET: u32 = 1;

use Level::*;

/// One catalogue row: id, frozen id text, tier, basis, source, title, the
/// advisory, strict and closed-operand strict levels, gap, refusable.
macro_rules! rule {
    ($id:ident, $text:literal, $tier:ident, $basis:ident, $source:literal, $title:literal,
     $advisory:ident, $strict:ident, $closed:expr, gap = $gap:literal, refusable = $refusable:literal) => {
        RuleInfo::new(
            RuleId::$id,
            $text,
            Tier::$tier,
            $title,
            Basis::$basis,
            $source,
            ($advisory, $strict, $closed),
            $gap,
            $refusable,
        )
    };
}

/// Every rule, in catalogue order.
///
/// The sources quote the W3C ODRL Information Model 2.2 (IM) and ODRL
/// Vocabulary & Expression 2.2 by section. A `profile.*` rule reads IM 2.1
/// ("An ODRL Policy must either: Only use terms defined in the ODRL Core
/// Vocabulary, or Use an ODRL Profile that declares the supported
/// vocabulary used by expressions in the Policy") and IM 3.2 under a closed
/// world, which the standard does not itself call for; an `odrl.*` rule is
/// a MUST the IM states. A `prose.*` rule is a ds-prose convention.
///
/// A `prose.*` rule that binds (a closed operand, strict mode) reads a
/// loaded term's hints, so it is silent while an `@context` was not read (the
/// prefix may be rebound) and, for `prose.value-not-allowed` alone, while a
/// declared profile is not loaded and the value is an IRI (it may add an IRI
/// narrower than an allowed one, never a literal). The operator, unit, datatype and string-for-IRI rules read
/// the loaded term and the written value only, which a missing profile cannot
/// change, so they hold. `prose.ill-typed-literal` reads no hints.
pub static RULES: &[RuleInfo] = &[
    rule!(
        ProfileNotLoaded,
        "profile.not-loaded",
        Profile,
        StandardDerived,
        "ODRL IM 3.2",
        "A declared profile is not loaded",
        Warning,
        Error,
        None,
        gap = true,
        refusable = false
    ),
    rule!(
        ProfileImportNotLoaded,
        "profile.import-not-loaded",
        Profile,
        StandardDerived,
        "ODRL IM 3.2; owl:imports, prof:isProfileOf",
        "A loaded profile builds on a profile that is not loaded",
        Off,
        Warning,
        None,
        gap = true,
        refusable = false
    ),
    rule!(
        ProfileUndeclaredInNamespace,
        "profile.undeclared-in-namespace",
        Profile,
        StandardDerived,
        "ODRL IM 2.1, 3.2",
        "A term in a loaded profile's namespace that no loaded profile declares",
        Warning,
        Error,
        None,
        gap = false,
        refusable = true
    ),
    rule!(
        ProfileUnknownTerm,
        "profile.unknown-term",
        Profile,
        StandardDerived,
        "ODRL IM 2.1, 3.2",
        "A term that neither ODRL nor a loaded profile defines",
        Off,
        Error,
        None,
        gap = false,
        refusable = true
    ),
    rule!(
        ProfileWrongKind,
        "profile.wrong-kind",
        Profile,
        StandardDerived,
        "ODRL IM 3.3; ODRL Vocabulary 3 (rdfs:range)",
        "A term used where its kind does not belong",
        Off,
        Error,
        None,
        gap = false,
        refusable = true
    ),
    rule!(
        ProfileNotDeclared,
        "profile.not-declared",
        Profile,
        StandardDerived,
        "ODRL IM 2.1, 3.2",
        "A term from a profile the policy does not declare",
        Off,
        Error,
        None,
        gap = false,
        refusable = false
    ),
    rule!(
        ProfileConflictTerm,
        "profile.conflict-term",
        Profile,
        StandardDerived,
        "ODRL IM 2.10 (SHOULD)",
        "A conflict value that is not a conflict strategy",
        Off,
        Error,
        None,
        gap = false,
        refusable = true
    ),
    rule!(
        ProfileUnboundPrefix,
        "profile.unbound-prefix",
        Profile,
        StandardDerived,
        "JSON-LD 1.1 IRI expansion",
        "A compact IRI only a profile's prefixes expand; a JSON-LD processor reads another IRI",
        Off,
        Error,
        None,
        gap = false,
        refusable = true
    ),
    rule!(
        ProfileDeprecatedTerm,
        "profile.deprecated-term",
        Profile,
        Convention,
        "owl:deprecated; ODRL Vocabulary appendix A",
        "A deprecated term",
        Warning,
        Warning,
        None,
        gap = false,
        refusable = false
    ),
    rule!(
        OdrlPolicyUid,
        "odrl.policy-uid",
        Odrl,
        Standard,
        "ODRL IM 2.1",
        "A policy without a uid",
        Off,
        Error,
        None,
        gap = false,
        refusable = false
    ),
    rule!(
        OdrlNoRule,
        "odrl.no-rule",
        Odrl,
        Standard,
        "ODRL IM 2.1",
        "A policy without a permission, prohibition or obligation",
        Off,
        Error,
        None,
        gap = false,
        refusable = false
    ),
    rule!(
        OdrlOfferAssigner,
        "odrl.offer-assigner",
        Odrl,
        Standard,
        "ODRL IM 2.1.2, 2.7.1",
        "An Offer rule without an assigner",
        Off,
        Error,
        None,
        gap = false,
        refusable = false
    ),
    rule!(
        OdrlAgreementParties,
        "odrl.agreement-parties",
        Odrl,
        Standard,
        "ODRL IM 2.1.3, 2.7.1",
        "An Agreement rule without an assigner or an assignee",
        Off,
        Error,
        None,
        gap = false,
        refusable = false
    ),
    rule!(
        OdrlRuleTarget,
        "odrl.rule-target",
        Odrl,
        Standard,
        "ODRL IM 2.6.1, 2.6.2, 2.7.1",
        "A permission or prohibition without a target",
        Off,
        Error,
        None,
        gap = false,
        refusable = false
    ),
    rule!(
        OdrlRuleAction,
        "odrl.rule-action",
        Odrl,
        Standard,
        "ODRL IM 2.6, 2.7.1",
        "A rule without an action",
        Off,
        Error,
        None,
        gap = false,
        refusable = false
    ),
    rule!(
        OdrlConstraintShape,
        "odrl.constraint-shape",
        Odrl,
        Standard,
        "ODRL IM 2.5.1",
        "A constraint without a left operand, an operator, or exactly one of rightOperand and rightOperandReference",
        Off,
        Error,
        None,
        gap = false,
        refusable = false
    ),
    rule!(
        OdrlSetOperatorCardinality,
        "odrl.set-operator-cardinality",
        Odrl,
        Standard,
        "ODRL IM 2.5.1",
        "Several right operand values with an operator that is not set-based",
        Off,
        Error,
        None,
        gap = false,
        refusable = false
    ),
    rule!(
        OdrlConsequenceNesting,
        "odrl.consequence-nesting",
        Odrl,
        Standard,
        "ODRL IM 2.6.3, 2.6.7",
        "A consequence on a consequence, or a remedy that has a consequence",
        Off,
        Error,
        None,
        gap = false,
        refusable = false
    ),
    rule!(
        OdrlInheritanceCycle,
        "odrl.inheritance-cycle",
        Odrl,
        Standard,
        "ODRL IM 2.9",
        "Policies that inherit from each other in a circle",
        Off,
        Error,
        None,
        gap = false,
        refusable = false
    ),
    rule!(
        ProseOperatorNotAllowed,
        "prose.operator-not-allowed",
        Prose,
        Convention,
        "ds-prose profile extension: prose:operator",
        "An operator outside the left operand's prose:operator",
        Info,
        Info,
        Some(Error),
        gap = false,
        refusable = true
    ),
    rule!(
        ProseValueNotAllowed,
        "prose.value-not-allowed",
        Prose,
        Convention,
        "ds-prose profile extension: prose:rightOperand",
        "A value outside the left operand's prose:rightOperand and its narrower values",
        Off,
        Info,
        Some(Error),
        gap = false,
        refusable = false
    ),
    rule!(
        ProseIriExpected,
        "prose.iri-expected",
        Prose,
        Convention,
        "ds-prose profile extension: prose:rightOperand",
        "A plain string that names an allowed value, where an IRI is expected",
        Off,
        Off,
        Some(Error),
        gap = false,
        refusable = false
    ),
    rule!(
        ProseUnitNotAllowed,
        "prose.unit-not-allowed",
        Prose,
        Convention,
        "ds-prose profile extension: prose:unit",
        "A unit outside the left operand's prose:unit",
        Off,
        Info,
        Some(Error),
        gap = false,
        refusable = true
    ),
    rule!(
        ProseDatatypeLexical,
        "prose.datatype-lexical",
        Prose,
        Convention,
        "ds-prose profile extension: prose:datatype",
        "A literal outside the lexical space of the left operand's prose:datatype",
        Info,
        Info,
        Some(Error),
        gap = false,
        refusable = false
    ),
    rule!(
        ProseDatatypeConflict,
        "prose.datatype-conflict",
        Prose,
        Convention,
        "ds-prose profile extension: prose:datatype",
        "A literal typed with another datatype than the left operand's prose:datatype",
        Off,
        Warning,
        Some(Error),
        gap = false,
        refusable = false
    ),
    rule!(
        ProseIllTypedLiteral,
        "prose.ill-typed-literal",
        Prose,
        Convention,
        "XSD 1.1 Part 2 lexical spaces; RDF 1.1 Concepts 3.3 (ill-typed literal)",
        "A typed literal outside its own datatype's lexical space",
        Off,
        Error,
        None,
        gap = false,
        refusable = false
    ),
    rule!(
        InputUnreadable,
        "input.unreadable",
        Input,
        Input,
        "ds-prose reader",
        "The input is not a policy",
        Error,
        Error,
        None,
        gap = false,
        refusable = false
    ),
    rule!(
        InputUnchecked,
        "input.unchecked",
        Input,
        Input,
        "ds-prose reader",
        "Something the validator could not check",
        Off,
        Warning,
        None,
        gap = true,
        refusable = false
    ),
    rule!(
        InputReaderNote,
        "input.reader-note",
        Input,
        Input,
        "ds-prose reader",
        "A note the reader made while reading the policy",
        Off,
        Info,
        None,
        gap = false,
        refusable = false
    ),
    rule!(
        HeuristicOrderedOnUnordered,
        "heuristic.ordered-on-unordered",
        Heuristic,
        Heuristic,
        "none: the ODRL IM states no requirement here",
        "An ordering operator on an IRI or boolean value",
        Off,
        Off,
        None,
        gap = false,
        refusable = false
    ),
];
