//! Wording with loaded profiles: a term a loaded profile declares reads with
//! the profile's label; every other term, and every ODRL term, reads as the
//! `words` module phrases it.
use crate::Gloss;
use crate::profile::{Prefixes, ProfileVocab, Term, TermKind, Wording};
use crate::words::{self, local_name};
use std::collections::HashMap;

const ODRL_NS: &str = "http://www.w3.org/ns/odrl/2/";

/// The wording one reading uses. Without a profile it is `words`, exactly.
pub(crate) struct WordCtx<'a> {
    /// `None` when no profile is loaded.
    wording: Option<&'a Wording>,
    prefixes: Prefixes,
    /// Namespace -> the first loaded profile that declares a term in it.
    /// ODRL's own namespace is left out.
    namespaces: HashMap<&'a str, &'a ProfileVocab>,
}

impl<'a> WordCtx<'a> {
    /// The built-in wording.
    pub(crate) fn plain() -> WordCtx<'static> {
        WordCtx {
            wording: None,
            prefixes: Prefixes::default(),
            namespaces: HashMap::new(),
        }
    }

    /// `w`, with a policy's compact terms expanded by `prefixes`.
    pub(crate) fn new(w: &'a Wording, prefixes: Prefixes) -> WordCtx<'a> {
        let mut namespaces = HashMap::new();
        for p in w.profiles() {
            for t in p.terms() {
                let ns = namespace(&t.iri);
                if ns != ODRL_NS {
                    namespaces.entry(ns).or_insert(p);
                }
            }
        }
        WordCtx {
            wording: (!w.is_empty()).then_some(w),
            prefixes,
            namespaces,
        }
    }

    /// Whether a profile is loaded.
    pub(crate) fn profiled(&self) -> bool {
        self.wording.is_some()
    }

    pub(crate) fn set_prefixes(&mut self, prefixes: Prefixes) {
        self.prefixes = prefixes;
    }

    fn langs(&self) -> &[String] {
        self.wording.map_or(&[], |w| w.languages())
    }

    /// `written` as an expanded IRI.
    pub(crate) fn expand(&self, written: &str) -> String {
        match self.wording {
            Some(w) => w.expand(&self.prefixes, written),
            None => self.prefixes.expand(written),
        }
    }

    /// The loaded term `written` names, and its profile.
    pub(crate) fn resolve(&self, written: &str) -> Option<(&'a Term, &'a ProfileVocab)> {
        let w = self.wording?;
        w.lookup(&self.expand(written))
    }

    /// The loaded term of `kind` that `written` names, unless it is an ODRL
    /// term: those keep their built-in phrasing, whatever a profile says.
    fn worded(&self, written: &str, kind: TermKind) -> Option<String> {
        let (t, _) = self.resolve(written)?;
        if t.iri.starts_with(ODRL_NS) || !t.kinds.contains(&kind) {
            return None;
        }
        Some(t.display_label(self.langs()))
    }

    /// An action as a base-form verb phrase.
    pub(crate) fn action(&self, raw: &str) -> String {
        match self.worded(raw, TermKind::Action) {
            Some(label) => lower_first(&label),
            None => words::action(raw),
        }
    }

    /// A left operand as a noun phrase.
    pub(crate) fn operand(&self, raw: &str) -> String {
        match self.worded(raw, TermKind::LeftOperand) {
            Some(label) => {
                let label = lower_first(&label);
                let article = ["the ", "a ", "an "]
                    .iter()
                    .any(|a| label.to_lowercase().starts_with(a));
                if article {
                    label
                } else {
                    format!("the {label}")
                }
            }
            None => words::operand(raw),
        }
    }

    /// An operator as the verb phrase that follows `left`. A label that is
    /// already a verb phrase ("Has part") is kept; otherwise it follows
    /// "is" ("Greater than" reads "is greater than").
    pub(crate) fn operator_for(&self, left: &str, op: &str) -> String {
        match self.worded(op, TermKind::Operator) {
            Some(label) => {
                let label = lower_first(&label);
                let verb = label.split_whitespace().next().unwrap_or("");
                match verb {
                    "is" | "has" | "are" | "does" | "contains" | "matches" => label,
                    _ => format!("is {label}"),
                }
            }
            None => words::operator_for(left, op),
        }
    }

    /// A unit, after its value: a profile term's label as written.
    pub(crate) fn unit(&self, raw: &str) -> String {
        match self.resolve(raw) {
            Some((t, _)) if !t.iri.starts_with(ODRL_NS) => t.display_label(self.langs()),
            _ => words::humanise(local_name(raw)),
        }
    }

    /// A right-operand IRI as a profile value's label.
    pub(crate) fn value(&self, raw: &str) -> Option<String> {
        self.worded(raw, TermKind::RightOperand)
            .map(|l| lower_first(&l))
    }

    /// The label of a policy class, for the heading.
    pub(crate) fn policy_class(&self, raw: &str) -> Option<String> {
        self.worded(raw, TermKind::PolicyClass)
    }

    /// The label of a party function, written as a rule property.
    pub(crate) fn party_function(&self, raw: &str) -> Option<String> {
        self.worded(raw, TermKind::PartyFunction)
    }

    /// The loaded profile `id` names, if one is loaded.
    pub(crate) fn profile(&self, id: &str) -> Option<&'a ProfileVocab> {
        let w = self.wording?;
        let want = self.expand(id);
        let want = want.trim_end_matches(['#', '/']);
        w.profiles()
            .find(|p| p.iri().map(|i| i.trim_end_matches(['#', '/'])) == Some(want))
    }

    /// Whether the profile `id` is covered by the loaded ones (ODRL's core
    /// always is). True when no profile is loaded: nothing is said then.
    pub(crate) fn covers(&self, id: &str) -> bool {
        self.wording.is_none_or(|w| w.covers(&self.expand(id)))
    }

    /// The loaded profile whose namespace `written` is in, when no loaded
    /// profile declares it: likely a typo, or a term from a newer version.
    pub(crate) fn undeclared_in(&self, written: &str) -> Option<&'a ProfileVocab> {
        let iri = self.expand(written);
        if self.resolve(written).is_some() {
            return None;
        }
        self.namespaces.get(namespace(&iri)).copied()
    }

    /// The glossary entry for `written`, used as a `kind`.
    pub(crate) fn gloss(&self, written: &str, kind: TermKind) -> Option<Gloss> {
        let (t, p) = self.resolve(written)?;
        let kind = if t.kinds.contains(&kind) {
            kind
        } else {
            *t.kinds.first()?
        };
        Some(self.gloss_of(written, t, p, kind))
    }

    fn gloss_of(&self, written: &str, t: &Term, p: &ProfileVocab, kind: TermKind) -> Gloss {
        let langs = self.langs();
        let labelled = |iris: &mut dyn Iterator<Item = &String>| {
            let mut out: Vec<(String, String)> = Vec::new();
            for i in iris {
                if !out.iter().any(|(x, _)| x == i) {
                    out.push((i.clone(), self.label_of(i)));
                }
            }
            out
        };
        Gloss {
            written: written.to_string(),
            iri: t.iri.clone(),
            kind,
            label: t.display_label(langs),
            definition: t.definition.pick(langs).map(str::to_string),
            note: t.note.pick(langs).map(str::to_string),
            broader: labelled(&mut t.included_in.iter().chain(&t.broader)),
            implies: labelled(&mut t.implies.iter()),
            profile: profile_title(p, langs),
            deprecated: t.deprecated,
        }
    }

    /// The label of any IRI: a loaded term's, else its local name.
    fn label_of(&self, iri: &str) -> String {
        match self.wording.and_then(|w| w.lookup(iri)) {
            Some((t, _)) => t.display_label(self.langs()),
            None => words::humanise(local_name(iri)),
        }
    }

    /// The profile's title in the preferred language, for prose.
    pub(crate) fn title(&self, p: &ProfileVocab) -> Option<String> {
        p.title().pick(self.langs()).map(str::to_string)
    }
}

/// How a warning names a profile: its IRI, else its title.
pub(crate) fn profile_name(p: &ProfileVocab) -> String {
    match (p.iri(), p.title().pick(&[])) {
        (Some(i), _) => format!("<{i}>"),
        (None, Some(t)) => format!("\"{t}\""),
        (None, None) => "(unnamed)".to_string(),
    }
}

fn profile_title(p: &ProfileVocab, langs: &[String]) -> Option<String> {
    p.title().pick(langs).or(p.iri()).map(str::to_string)
}

/// The IRI up to and including its last `#` or `/`.
fn namespace(iri: &str) -> &str {
    match iri.rfind(['#', '/']) {
        Some(i) => &iri[..=i],
        None => iri,
    }
}

/// The first letter lower-cased, unless the label starts with an acronym
/// ("EU region" stays).
fn lower_first(label: &str) -> String {
    let mut chars = label.chars();
    match (chars.next(), chars.next()) {
        (Some(_), Some(second)) if second.is_uppercase() => label.to_string(),
        (Some(first), _) => first.to_lowercase().chain(label.chars().skip(1)).collect(),
        (None, _) => String::new(),
    }
}

/// [`crate::wording::gloss`]: `written` as a policy with `prefixes` writes
/// it, explained by the first loaded profile that declares it. Without a
/// use to go by, a term is glossed as its first kind.
pub(crate) fn gloss(w: &Wording, prefixes: &Prefixes, written: &str) -> Option<Gloss> {
    let ctx = WordCtx {
        wording: (!w.is_empty()).then_some(w),
        prefixes: prefixes.clone(),
        namespaces: HashMap::new(),
    };
    let (t, p) = ctx.resolve(written)?;
    let kind = *t.kinds.first()?;
    Some(ctx.gloss_of(written, t, p, kind))
}

/// [`crate::wording::operator_with`].
pub(crate) fn operator_with(w: &Wording, prefixes: &Prefixes, left: &str, op: &str) -> String {
    let ctx = WordCtx {
        wording: (!w.is_empty()).then_some(w),
        prefixes: prefixes.clone(),
        namespaces: HashMap::new(),
    };
    ctx.operator_for(left, op)
}
