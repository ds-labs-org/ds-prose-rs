/// The prose reading of one JSON-LD input.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Document {
    pub policies: Vec<Policy>,
    /// Things the reader noticed but did not turn into prose: unknown
    /// properties, unreadable values.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyKind {
    Set,
    Offer,
    Agreement,
    /// Any other policy `@type`, kept as written.
    Other(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Policy {
    pub kind: PolicyKind,
    pub uid: Option<String>,
    /// e.g. "Agreement urn:x:1".
    pub heading: String,
    /// One sentence on what the policy is and who it is between.
    pub intro: String,
    /// Profile, conflict strategy and inheritance, one clause each.
    pub notes: Vec<String>,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleKind {
    Permission,
    Prohibition,
    /// An obligation, or a duty nested inside another rule.
    Obligation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub kind: RuleKind,
    pub uid: Option<String>,
    /// The rule as one sentence, e.g. "Alice may use urn:asset:1."
    pub sentence: String,
    /// Constraints on the action itself (action refinements).
    pub refinements: Vec<Condition>,
    /// Constraints on the rule: it applies only while all of these hold.
    pub conditions: Vec<Condition>,
    pub follow_ups: Vec<FollowUp>,
}

/// Rules that hang off another rule: the duties a permission requires, the
/// remedies of a prohibition, the consequences of an obligation.
#[derive(Debug, Clone, PartialEq)]
pub struct FollowUp {
    /// e.g. "Before this permission can be exercised, these duties must be fulfilled".
    pub label: String,
    pub rules: Vec<Rule>,
}

/// A clause, possibly with sub-clauses (a logical constraint).
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub text: String,
    pub children: Vec<Condition>,
}

impl Condition {
    pub(crate) fn leaf(text: impl Into<String>) -> Self {
        Condition {
            text: text.into(),
            children: Vec::new(),
        }
    }
}
