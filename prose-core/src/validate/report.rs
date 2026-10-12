//! The report as JSON, schema `ds-prose/validation-report@1`. The same
//! bytes in every host: no timestamps, findings in document order, object
//! keys in a fixed order. A later `@1` may only add keys; removing or
//! renaming one is `@2`.
use super::{Basis, Expected, Finding, Level, Mode, Options, RULES, Report, Tier, Verdict};
use crate::edit::{IssueTarget, Severity};
use crate::profile::{OdrlVocab, TermKind};
use serde_json::{Value, json};

/// The schema the JSON follows.
pub const SCHEMA: &str = "ds-prose/validation-report@1";

/// How many findings of each severity a report holds.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub error: usize,
    pub warning: usize,
    pub info: usize,
}

impl Counts {
    pub fn total(&self) -> usize {
        self.error + self.warning + self.info
    }
}

/// A loaded profile, as the report names it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ProfileRef {
    pub(crate) iri: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) version: Option<String>,
}

impl Report {
    pub fn counts(&self) -> Counts {
        let mut c = Counts::default();
        for f in &self.findings {
            match f.severity {
                Severity::Error => c.error += 1,
                Severity::Warning => c.warning += 1,
                Severity::Info => c.info += 1,
            }
        }
        c
    }

    /// The report as `ds-prose/validation-report@1` JSON.
    pub fn to_json(&self) -> Value {
        let c = self.counts();
        json!({
            "schema": SCHEMA,
            "tool": {"name": "ds-prose-rs", "version": env!("CARGO_PKG_VERSION")},
            "mode": match self.mode {
                Mode::Strict => "strict",
                Mode::Advisory => "advisory",
            },
            "tiers": tiers(&self.options),
            "ruleset": self.options.ruleset,
            "verdict": match self.verdict {
                Verdict::Conforms => "conforms",
                Verdict::Violations => "violations",
                Verdict::Incomplete => "incomplete",
            },
            "vocabulary": {
                "odrl": match self.options.odrl {
                    OdrlVocab::Core => "core",
                    OdrlVocab::Common => "common",
                },
                "profiles": self.profiles.iter().map(|p| json!({
                    "iri": p.iri, "title": p.title, "version": p.version
                })).collect::<Vec<_>>(),
            },
            "gaps": self.gaps.iter().map(|f| json!({
                "rule": f.rule.as_str(),
                "detail": f.term.as_ref().map_or(f.message.as_str(), |t| t.written.as_str()),
            })).collect::<Vec<_>>(),
            "counts": {"error": c.error, "warning": c.warning, "info": c.info},
            "findings": self.findings.iter().map(finding).collect::<Vec<_>>(),
        })
    }
}

impl Finding {
    /// A short, stable name for this finding: FNV-1a 64 over the rule id,
    /// the target path and the term as written (the message when there is
    /// no term), as 16 hex digits. Reformatting the input or reordering its
    /// keys does not change it; moving the finding or changing the term
    /// does.
    pub fn fingerprint(&self) -> String {
        let path = target_path(&self.target);
        let term = self
            .term
            .as_ref()
            .map_or(self.message.as_str(), |t| t.written.as_str());
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for part in [
            self.rule.as_str().as_bytes(),
            path.as_bytes(),
            term.as_bytes(),
        ]
        .iter()
        .enumerate()
        {
            if part.0 > 0 {
                h = fnv_byte(h, 0);
            }
            for &b in *part.1 {
                h = fnv_byte(h, b);
            }
        }
        format!("{h:016x}")
    }
}

fn fnv_byte(h: u64, b: u8) -> u64 {
    (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
}

/// The tiers with a rule that is not off, in catalogue order.
fn tiers(o: &Options) -> Vec<&'static str> {
    [
        (Tier::Profile, "profile"),
        (Tier::Odrl, "odrl"),
        (Tier::Prose, "prose"),
        (Tier::Input, "input"),
        (Tier::Heuristic, "heuristic"),
    ]
    .into_iter()
    .filter(|(t, _)| {
        RULES.iter().any(|r| {
            r.tier == *t
                && (o.level_of(r.id, false) != Level::Off || o.level_of(r.id, true) != Level::Off)
        })
    })
    .map(|(_, name)| name)
    .collect()
}

fn target_kind(t: &IssueTarget) -> &'static str {
    match t {
        IssueTarget::Node(_) => "node",
        IssueTarget::Slot(_) => "slot",
        IssueTarget::List(_) => "list",
        IssueTarget::Doc => "doc",
    }
}

fn target_path(t: &IssueTarget) -> String {
    match t {
        IssueTarget::Node(n) => n.to_string(),
        IssueTarget::Slot(s) => s.to_string(),
        IssueTarget::List(l) => l.to_string(),
        IssueTarget::Doc => String::new(),
    }
}

fn finding(f: &Finding) -> Value {
    let info = f.rule.info();
    json!({
        "rule": f.rule.as_str(),
        "tier": match info.tier {
            Tier::Profile => "profile",
            Tier::Odrl => "odrl",
            Tier::Prose => "prose",
            Tier::Input => "input",
            Tier::Heuristic => "heuristic",
        },
        "basis": match info.basis {
            Basis::Standard => "standard",
            Basis::StandardDerived => "standard-derived",
            Basis::Convention => "convention",
            Basis::Heuristic => "heuristic",
            Basis::Input => "input",
        },
        "severity": match f.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        },
        "message": f.message,
        "target": {"kind": target_kind(&f.target), "path": target_path(&f.target)},
        "pointer": f.pointer,
        "term": f.term.as_ref().map(|t| json!({"written": t.written, "iri": t.iri})),
        "profile": f.profile,
        "expected": f.expected.as_ref().map(expected),
        "fixes": f.fixes.iter().map(|x| json!({
            "label": x.label, "safe": x.safe, "replacement": x.replacement
        })).collect::<Vec<_>>(),
        "fingerprint": f.fingerprint(),
    })
}

fn expected(e: &Expected) -> Value {
    json!({
        "kinds": e.kinds.iter().map(|k| kind_name(*k)).collect::<Vec<_>>(),
        "operators": e.operators,
        "values": e.values,
        "units": e.units,
        "datatype": e.datatype,
        "closed": e.closed,
    })
}

/// A kind as the report names it: the ODRL class or property it stands
/// for, in lower camel case.
fn kind_name(k: TermKind) -> &'static str {
    match k {
        TermKind::Action => "action",
        TermKind::LeftOperand => "leftOperand",
        TermKind::Operator => "operator",
        TermKind::RightOperand => "rightOperand",
        TermKind::ConflictTerm => "conflictTerm",
        TermKind::PolicyClass => "policyClass",
        TermKind::RuleClass => "ruleClass",
        TermKind::AssetClass => "assetClass",
        TermKind::PartyClass => "partyClass",
        TermKind::PartyFunction => "partyFunction",
        TermKind::AssetRelation => "assetRelation",
        TermKind::LogicalOperand => "logicalOperand",
    }
}
