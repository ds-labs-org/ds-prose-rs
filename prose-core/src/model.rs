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
    /// The IRIs of constraints the sentence refers to (in a target's or a
    /// party's refinement) and the document does not declare, each once;
    /// the sentence reads "the constraint defined at {IRI} holds" for each.
    /// A rule's own conditions carry theirs on [`Condition::reference`].
    pub references: Vec<String>,
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
    /// The IRI of a constraint the policy refers to and the document does
    /// not declare; `text` reads "the constraint defined at {IRI} holds".
    /// The reader fetches nothing: a host that knows the IRI dereferences
    /// can show it as a link.
    pub reference: Option<String>,
}

impl Condition {
    pub(crate) fn leaf(text: impl Into<String>) -> Self {
        Condition {
            text: text.into(),
            children: Vec::new(),
            reference: None,
        }
    }
}

/// A reading worded with loaded profiles: the [`Document`], plus the terms
/// the profiles explain, one glossary per policy.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Reading {
    pub document: Document,
    /// `glossary[i]` explains the terms of `document.policies[i]`. Empty
    /// glossaries when no profile is loaded.
    pub glossary: Vec<PolicyGlossary>,
}

/// The profile terms one policy uses, each once, in reading order.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PolicyGlossary {
    pub terms: Vec<Gloss>,
}

/// One term of a policy, as a loaded profile explains it. All text is
/// plain text: the profile's markup was stripped when it was read.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Gloss {
    /// The term as the policy writes it, e.g. `ex:fineTune`.
    pub written: String,
    /// The expanded IRI.
    pub iri: String,
    /// What the policy uses it as.
    pub kind: crate::profile::TermKind,
    pub label: String,
    pub definition: Option<String>,
    pub note: Option<String>,
    /// (IRI, label) of the terms it is included in or narrower than.
    pub broader: Vec<(String, String)>,
    /// (IRI, label) of the terms it implies.
    pub implies: Vec<(String, String)>,
    /// The title (else the IRI) of the profile that declares it.
    pub profile: Option<String>,
    pub deprecated: bool,
}
