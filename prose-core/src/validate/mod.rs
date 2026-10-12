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
//!
//! A verdict is `Conforms`, `Violations` or `Incomplete`; incomplete (a
//! declared profile is not loaded, say) is never a pass and never a guessed
//! error. The `odrl.*` tier of the Information Model's own requirements is
//! off until [`Options::structural`] switches it on. A report serialises as
//! `ds-prose/validation-report@1` ([`Report::to_json`]).
//!
//! ```
//! use prose_core::profile::Wording;
//! use prose_core::validate::{Options, Verdict, validate_json};
//!
//! let policy = r#"{"@type":"Set","profile":"https://example.org/p",
//!   "permission":[{"action":"use","target":"https://example.org/a"}]}"#;
//! // The policy declares a profile that is not loaded: not a pass.
//! let report = validate_json(policy, &Wording::default(), &Options::strict());
//! assert_eq!(report.verdict, Verdict::Incomplete);
//! assert!(!report.conforms());
//! ```
use crate::edit::{EditDoc, Issue, IssueTarget, NodeId, Severity};
use crate::profile::{OdrlVocab, Wording};
use std::collections::BTreeMap;
use std::sync::Arc;

mod check;
mod inherit;
mod lexical;
mod report;
mod rules;

pub use report::{Counts, SCHEMA};
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
        Report::new(&self.options, &self.wording, findings)
    }

    /// Whether strict mode's reducer gate lets `ev` through on `doc`, the
    /// document it would be applied to. `Err` names the finding when `ev`
    /// writes a term into a vocabulary slot (an action, a left operand, an
    /// operator, a unit, a conflict strategy or a policy type) that a
    /// refusable rule ([`RuleInfo::refusable`]) reports as an error there.
    ///
    /// The check is slot-local: it looks at the written value only, never at
    /// what depends on it. Always `Ok` in advisory mode, for the empty
    /// string, for free text, right operands and structural events, and
    /// while the policy's vocabulary is incomplete. It reuses the index
    /// built by [`Validator::new`]. See [`EditDoc::apply_gated`].
    pub fn admissible(
        &self,
        doc: &EditDoc,
        ev: &crate::edit::EditEvent,
    ) -> Result<(), Box<Finding>> {
        check::admissible(doc, &self.wording, &self.index, &self.options, ev)
    }

    /// As [`Validator::validate`], for a policy given as JSON-LD text. Never
    /// fails: a text that is not a policy is an `input.unreadable` finding.
    pub fn validate_json(&self, json: &str) -> Report {
        match crate::edit::read_model(json) {
            Ok(doc) => self.validate(&doc),
            Err(e) => Report::new(
                &self.options,
                &self.wording,
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

/// The most values one finding lists as expected.
pub const MAX_EXPECTED_VALUES: usize = 8;

/// What would have been accepted where a finding was made.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Expected {
    pub kinds: Vec<crate::profile::TermKind>,
    pub operators: Vec<String>,
    /// The values the profile allows, at most [`MAX_EXPECTED_VALUES`] of
    /// them, so a report grows with the findings and not with the profile.
    pub values: Vec<String>,
    /// How many values the profile allows when `values` was cut short, else
    /// 0 (all of them are listed).
    pub values_total: usize,
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
    /// What the report was made with, for its JSON header.
    options: Options,
    profiles: Vec<report::ProfileRef>,
}

impl Report {
    fn new(options: &Options, wording: &Wording, findings: Vec<Finding>) -> Report {
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
            mode: options.mode,
            verdict,
            findings,
            gaps,
            options: options.clone(),
            profiles: wording
                .profiles()
                .map(|p| report::ProfileRef {
                    iri: p.iri().map(str::to_string),
                    title: p.title().pick(&[]).map(str::to_string),
                    version: p.version().map(str::to_string),
                })
                .collect(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{EditEvent, SlotPath, read_model};

    #[test]
    fn admissible_does_not_rebuild_index() {
        let doc = read_model(
            r#"{"@type": "Set", "uid": "urn:p",
                "permission": [{"target": "urn:a", "action": "use"}]}"#,
        )
        .unwrap();
        let slot: SlotPath = "policy[0].permission[0].action[0]#name".parse().unwrap();
        let expect = doc.node(&slot.node).unwrap().id();
        let before = check::index_builds();
        let v = Validator::new(Arc::new(Wording::new()), Options::strict());
        assert_eq!(check::index_builds(), before + 1);
        for value in ["use", "foo:bar", "", "read"] {
            let ev = EditEvent::SetText {
                slot: slot.clone(),
                expect,
                value: value.to_string(),
            };
            let _ = v.admissible(&doc, &ev);
            let mut d = doc.clone();
            let _ = d.apply_gated(&ev, &Default::default(), Some(&v));
            let _ = d.apply_all(std::slice::from_ref(&ev), &Default::default(), Some(&v));
        }
        let _ = v.clone().validate(&doc);
        assert_eq!(check::index_builds(), before + 1);
    }
}
