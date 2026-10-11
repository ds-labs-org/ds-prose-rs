//! Validation of a policy against the loaded ODRL profiles, in one of two
//! modes.
//!
//! - **Advisory** (the default) is what [`crate::edit::check_profile`] has
//!   always said: a profile widens and annotates, and what it says about a
//!   policy is a warning or a note, never an error.
//! - **Strict** reads the vocabulary as closed: ODRL plus the loaded
//!   profiles is every term there is, so a term that resolves nowhere is an
//!   error. While a declared profile is not loaded the vocabulary is
//!   incomplete, and the verdict says so instead of guessing.
//!
//! Every finding names its rule ([`RuleId`]); the catalogue ([`RULES`]) says
//! which tier a rule belongs to and the authority behind it, so a reader
//! can tell what the standard requires from what is a ds-prose convention.
use crate::edit::{EditDoc, Issue, IssueTarget, NodeId, Severity};
use crate::profile::{OdrlVocab, Wording};
use std::collections::BTreeMap;
use std::sync::Arc;

mod check;
mod inherit;
mod lexical;
mod rules;

pub use rules::{Basis, Level, RULES, RULESET, RuleId, RuleInfo, Tier, UnknownRule};

/// How a policy is read against its profiles.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Profiles widen and annotate; nothing is an error.
    #[default]
    Advisory,
    /// The vocabulary is closed: ODRL plus the loaded profiles.
    Strict,
}

/// What to check and how loudly. A cheap value: compare or hash it as a
/// memo key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Options {
    mode: Mode,
    structural: bool,
    fail_closed: bool,
    deny_deprecated: bool,
    odrl: OdrlVocab,
    ruleset: u32,
    /// Per-rule levels set with [`Options::level`]; the last one set wins.
    levels: BTreeMap<RuleId, Level>,
}

impl Default for Options {
    fn default() -> Options {
        Options::advisory()
    }
}

impl Options {
    /// What [`crate::edit::check_profile`] says.
    pub fn advisory() -> Options {
        Options {
            mode: Mode::Advisory,
            structural: false,
            fail_closed: false,
            deny_deprecated: false,
            odrl: OdrlVocab::Common,
            ruleset: RULESET,
            levels: BTreeMap::new(),
        }
    }

    /// The closed world over ODRL and the loaded profiles: the `profile.*`
    /// and `prose.*` rules at their strict levels, and what could not be
    /// checked reported as a gap. The structural `odrl.*` tier stays off
    /// until [`Options::structural`] switches it on.
    pub fn strict() -> Options {
        Options {
            mode: Mode::Strict,
            ..Options::advisory()
        }
    }

    /// Switches the structural `odrl.*` tier (the Information Model's own
    /// MUSTs) on or off, in strict mode. Off by default: a catalog offer is
    /// often written without the assigner or target the model requires.
    pub fn structural(mut self, on: bool) -> Options {
        self.structural = on;
        self
    }

    /// In strict mode, makes `input.unchecked` an error that fails the
    /// policy, rather than a gap.
    pub fn fail_closed(mut self, on: bool) -> Options {
        self.fail_closed = on;
        self
    }

    /// In strict mode, makes a deprecated term an error.
    pub fn deny_deprecated(mut self, on: bool) -> Options {
        self.deny_deprecated = on;
        self
    }

    /// Which ODRL terms strict mode counts as defined without a profile:
    /// the Core and Common Vocabularies (the default), or the Core
    /// Vocabulary only.
    pub fn odrl_vocabulary(mut self, vocab: OdrlVocab) -> Options {
        self.odrl = vocab;
        self
    }

    pub fn odrl(&self) -> OdrlVocab {
        self.odrl
    }

    /// Checks only the rules of ruleset `n` and earlier (see
    /// [`RuleInfo::since_ruleset`]), so a pinned check does not change when
    /// a later release adds a rule.
    pub fn ruleset(mut self, n: u32) -> Options {
        self.ruleset = n;
        self
    }

    /// Sets `rule`'s level, over everything else. `input.unreadable` stays
    /// an error whatever it is set to.
    pub fn level(mut self, rule: RuleId, level: Level) -> Options {
        self.levels.insert(rule, level);
        self
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// The level `rule` has under these options, on a left operand that is
    /// `closed` by its profile or not: a level set with [`Options::level`];
    /// else off for a rule newer than the ruleset, or a structural rule
    /// that is not switched on; else the catalogue's level for the mode.
    pub fn level_of(&self, rule: RuleId, closed: bool) -> Level {
        if rule == RuleId::InputUnreadable {
            return Level::Error;
        }
        if let Some(&l) = self.levels.get(&rule) {
            return l;
        }
        let info = rule.info();
        if info.since_ruleset > self.ruleset {
            return Level::Off;
        }
        match self.mode {
            Mode::Advisory => info.advisory,
            Mode::Strict => {
                if info.tier == Tier::Odrl && !self.structural {
                    return Level::Off;
                }
                match rule {
                    RuleId::ProfileDeprecatedTerm if self.deny_deprecated => Level::Error,
                    RuleId::InputUnchecked if self.fail_closed => Level::Error,
                    _ => match info.strict_closed {
                        Some(l) if closed => l,
                        _ => info.strict,
                    },
                }
            }
        }
    }
}

/// A validator prepared once for a wording and options: the index of the
/// loaded profiles is built here, not on every check. Cheap to clone.
///
/// Two validators are equal when they share the same wording allocation and
/// have equal options, so comparing them costs nothing.
#[derive(Clone, Debug)]
pub struct Validator {
    wording: Arc<Wording>,
    index: Arc<check::Index>,
    options: Options,
}

impl PartialEq for Validator {
    fn eq(&self, other: &Validator) -> bool {
        Arc::ptr_eq(&self.wording, &other.wording) && self.options == other.options
    }
}

impl Validator {
    pub fn new(wording: Arc<Wording>, options: Options) -> Validator {
        let index = Arc::new(check::Index::new(&wording));
        Validator {
            wording,
            index,
            options,
        }
    }

    pub fn options(&self) -> &Options {
        &self.options
    }

    pub fn wording(&self) -> &Arc<Wording> {
        &self.wording
    }

    /// Every finding about `doc`, and the verdict.
    pub fn validate(&self, doc: &EditDoc) -> Report {
        let findings = check::run(doc, &self.wording, &self.index, &self.options);
        Report::new(self.options.mode, findings)
    }

    /// As [`Validator::validate`], for a policy given as JSON-LD text. Never
    /// fails: a text that is not a policy is an `input.unreadable` finding.
    pub fn validate_json(&self, json: &str) -> Report {
        match crate::edit::read_model(json) {
            Ok(doc) => self.validate(&doc),
            Err(e) => Report::new(
                self.options.mode,
                vec![Finding::new(
                    RuleId::InputUnreadable,
                    Severity::Error,
                    IssueTarget::Doc,
                    format!("the input could not be read as a policy: {e}"),
                )],
            ),
        }
    }
}

/// [`Validator::validate`] with a validator built for this call.
pub fn validate(doc: &EditDoc, wording: &Wording, options: &Options) -> Report {
    Validator::new(Arc::new(wording.clone()), options.clone()).validate(doc)
}

/// [`Validator::validate_json`] with a validator built for this call.
pub fn validate_json(json: &str, wording: &Wording, options: &Options) -> Report {
    Validator::new(Arc::new(wording.clone()), options.clone()).validate_json(json)
}

/// A term as the policy wrote it, and the IRI it was read as.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TermRef {
    pub written: String,
    pub iri: String,
}

/// What would have been accepted where a finding was made.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Expected {
    pub kinds: Vec<crate::profile::TermKind>,
    pub operators: Vec<String>,
    pub values: Vec<String>,
    pub units: Vec<String>,
    pub datatype: Option<String>,
    /// The left operand is closed by its profile.
    pub closed: bool,
}

/// A change that would clear a finding.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct Fix {
    pub label: String,
    /// Unambiguous: the only reasonable reading of what was meant.
    pub safe: bool,
    /// The text to write instead.
    pub replacement: String,
    /// The edit, as events with `expect` ids: applied to a document that
    /// has moved on, they fail as stale.
    pub events: Vec<crate::edit::EditEvent>,
}

/// One thing the validator found.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub rule: RuleId,
    pub severity: Severity,
    /// Something left unchecked: it makes the verdict incomplete, not a
    /// violation.
    pub gap: bool,
    pub target: IssueTarget,
    /// The id of the node the target is on, when it is on one.
    pub node: Option<NodeId>,
    pub term: Option<TermRef>,
    /// The IRI of the profile the finding is about.
    pub profile: Option<String>,
    pub expected: Option<Expected>,
    pub message: String,
    pub fixes: Vec<Fix>,
    /// An RFC 6901 pointer into the input; not filled in yet.
    pub pointer: Option<String>,
}

impl Finding {
    pub(crate) fn new(
        rule: RuleId,
        severity: Severity,
        target: IssueTarget,
        message: String,
    ) -> Finding {
        Finding {
            rule,
            severity,
            gap: rule.info().gap,
            target,
            node: None,
            term: None,
            profile: None,
            expected: None,
            message,
            fixes: Vec::new(),
            pointer: None,
        }
    }
}

impl From<&Finding> for Issue {
    fn from(f: &Finding) -> Issue {
        Issue {
            target: f.target.clone(),
            severity: f.severity,
            message: f.message.clone(),
        }
    }
}

/// The outcome of a validation.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// No error, and nothing left unchecked.
    Conforms,
    /// At least one error that does not rest on something unchecked.
    Violations,
    /// No such error, but something could not be checked.
    Incomplete,
}

/// What a validation found.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub mode: Mode,
    pub verdict: Verdict,
    /// Every finding, in document order.
    pub findings: Vec<Finding>,
    /// The findings that are gaps, also in `findings`.
    pub gaps: Vec<Finding>,
}

impl Report {
    fn new(mode: Mode, findings: Vec<Finding>) -> Report {
        let gaps: Vec<Finding> = findings.iter().filter(|f| f.gap).cloned().collect();
        let verdict = if findings
            .iter()
            .any(|f| f.severity == Severity::Error && !f.gap)
        {
            Verdict::Violations
        } else if !gaps.is_empty() {
            Verdict::Incomplete
        } else {
            Verdict::Conforms
        };
        Report {
            mode,
            verdict,
            findings,
            gaps,
        }
    }

    /// Whether the policy conforms: no violation and no gap.
    pub fn conforms(&self) -> bool {
        self.verdict == Verdict::Conforms
    }

    /// The findings as the editor's [`Issue`]s, in order.
    pub fn issues(&self) -> Vec<Issue> {
        self.findings.iter().map(Issue::from).collect()
    }
}
