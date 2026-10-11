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

/// One rule of the catalogue. Ids are frozen once released.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RuleId {
    ProfileNotLoaded,
    ProfileUndeclaredInNamespace,
    ProfileDeprecatedTerm,
    ProseOperatorNotAllowed,
    ProseDatatypeLexical,
    InputUnreadable,
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
    /// The first ruleset the rule is in.
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

/// Every rule, in catalogue order.
pub static RULES: &[RuleInfo] = &[
    RuleInfo::new(
        RuleId::ProfileNotLoaded,
        "profile.not-loaded",
        Tier::Profile,
        "A declared profile is not loaded",
        Basis::StandardDerived,
        "ODRL IM 3.2",
        (Warning, Error, None),
        true,
        false,
    ),
    RuleInfo::new(
        RuleId::ProfileUndeclaredInNamespace,
        "profile.undeclared-in-namespace",
        Tier::Profile,
        "A term in a loaded profile's namespace that the profile does not declare",
        Basis::StandardDerived,
        "ODRL IM 2.1, 3.2",
        (Warning, Error, None),
        false,
        true,
    ),
    RuleInfo::new(
        RuleId::ProfileDeprecatedTerm,
        "profile.deprecated-term",
        Tier::Profile,
        "A deprecated term",
        Basis::Convention,
        "owl:deprecated",
        (Warning, Warning, None),
        false,
        false,
    ),
    RuleInfo::new(
        RuleId::ProseOperatorNotAllowed,
        "prose.operator-not-allowed",
        Tier::Prose,
        "An operator outside the left operand's prose:operator",
        Basis::Convention,
        "ds-prose profile extension",
        (Info, Info, Some(Error)),
        false,
        true,
    ),
    RuleInfo::new(
        RuleId::ProseDatatypeLexical,
        "prose.datatype-lexical",
        Tier::Prose,
        "A literal outside the left operand's prose:datatype",
        Basis::Convention,
        "ds-prose profile extension",
        (Info, Info, Some(Error)),
        false,
        false,
    ),
    RuleInfo::new(
        RuleId::InputUnreadable,
        "input.unreadable",
        Tier::Input,
        "The input is not a policy",
        Basis::Input,
        "ds-prose reader",
        (Error, Error, None),
        false,
        false,
    ),
];
