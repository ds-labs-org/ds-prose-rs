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
    /// When it was made (`timestamp`), its summary, then profile, conflict
    /// strategy and inheritance, one sentence each.
    pub notes: Vec<String>,
    /// Its own rules, then those it inherits from policies in the same
    /// document (`inheritFrom`), each sentence beginning "Inherited from
    /// {parent}: ". As ODRL IM 2.9 replicates them, an inherited rule names
    /// the child's policy-level parties and assets where its parent has none,
    /// and the child's own rules (and intro, profile and conflict notes) take
    /// what its parents agree on where it states none.
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
    /// The `http(s)` URLs in the plain string right operand of an
    /// `acceptedTerms` constraint (any namespace), in reading order, each
    /// once; `text` reads each verbatim. The reader fetches nothing: a host
    /// that finds one answers can show it as a link.
    pub urls: Vec<String>,
}

impl Condition {
    pub(crate) fn leaf(text: impl Into<String>) -> Self {
        Condition {
            text: text.into(),
            children: Vec::new(),
            reference: None,
            urls: Vec::new(),
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
    /// `mentions[i]` lists the IRIs the text of `document.policies[i]`
    /// names, each once, in reading order: so a host can show each as a
    /// short label and keep the IRI, without parsing the text.
    pub mentions: Vec<Vec<Mention>>,
    /// `names[i]` says where the text of `document.policies[i]` names each
    /// of `mentions[i]`, so a host marks those places and no others: a
    /// literal or a word that reads like a label is not the IRI.
    pub names: Vec<PolicyNames>,
}

/// Where a text names an IRI: bytes `start..end` of the text read
/// `mentions[i][mention]` of its policy ([`Reading::mentions`]), as its
/// label or as the policy writes it there.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameSpan {
    pub start: usize,
    pub end: usize,
    pub mention: usize,
}

/// The [`NameSpan`]s of one [`Policy`]'s texts, shaped as the policy is.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PolicyNames {
    /// In [`Policy::intro`].
    pub intro: Vec<NameSpan>,
    /// `rules[j]` for `rules[j]` of the policy.
    pub rules: Vec<RuleNames>,
}

/// The [`NameSpan`]s of one [`Rule`]'s texts, shaped as the rule is.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuleNames {
    /// In [`Rule::sentence`].
    pub sentence: Vec<NameSpan>,
    pub refinements: Vec<ConditionNames>,
    pub conditions: Vec<ConditionNames>,
    /// `follow_ups[k][l]` for `follow_ups[k].rules[l]` of the rule.
    pub follow_ups: Vec<Vec<RuleNames>>,
}

/// The [`NameSpan`]s of one [`Condition`]'s text and of its sub-clauses.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConditionNames {
    /// In [`Condition::text`].
    pub text: Vec<NameSpan>,
    pub children: Vec<ConditionNames>,
}

/// An IRI the prose names: a party, an asset or a collection of either, or
/// a right operand.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    /// As the policy first writes it: the text reads so there where no
    /// label is declared. Another spelling of the same IRI (`ex:a1`,
    /// `http://example.com/a1`) is the same mention, read as written
    /// there; [`Reading::names`] says where each is.
    pub written: String,
    /// Expanded with the document's prefixes.
    pub iri: String,
    /// The label the document declares for it (`rdfs:label`, `dct:title`,
    /// `skos:prefLabel` or `schema:name` on a node with that identifier),
    /// which the text reads instead of the IRI.
    pub label: Option<String>,
}

impl Mention {
    /// How the text names it: its label, else the IRI as first written.
    pub fn text(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.written)
    }

    /// A short name to show for it: its label, else the IRI's last segment
    /// ([`crate::wording::short_iri`]).
    pub fn short_label(&self) -> &str {
        match &self.label {
            Some(l) => l,
            None => crate::words::short_iri(&self.iri),
        }
    }
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
