//! The loaded profiles and language preferences one reading uses, and how a
//! policy's compact terms expand.
use super::{ProfileVocab, Term, ns};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;

/// What the prose is worded with: the loaded profiles (first loaded wins on
/// a term two of them declare) and the preferred label languages.
/// `Wording::default()` is the built-in wording, no profile.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wording {
    profiles: Vec<Arc<ProfileVocab>>,
    languages: Vec<String>,
    conflicts: Vec<String>,
}

impl Wording {
    pub fn new() -> Wording {
        Wording::default()
    }

    /// Adds a profile after those already loaded. A term an earlier profile
    /// already declares keeps the earlier definition, and is reported in
    /// [`Wording::conflicts`].
    pub fn with_profile(mut self, p: ProfileVocab) -> Wording {
        for t in p.terms() {
            if let Some((_, first)) = self.lookup(&t.iri) {
                let msg = format!(
                    "\"{}\" is declared by both {} and {}; the first loaded is used",
                    t.iri,
                    name(first),
                    name(&p)
                );
                self.conflicts.push(msg);
            }
        }
        self.profiles.push(Arc::new(p));
        self
    }

    /// The label languages to prefer, most preferred first.
    pub fn with_languages(mut self, langs: Vec<String>) -> Wording {
        self.languages = langs;
        self
    }

    pub fn languages(&self) -> &[String] {
        &self.languages
    }

    /// The loaded profiles, in load order.
    pub fn profiles(&self) -> impl Iterator<Item = &ProfileVocab> {
        self.profiles.iter().map(|p| &**p)
    }

    /// The term with this expanded IRI, and the profile that declares it.
    pub fn lookup(&self, iri: &str) -> Option<(&Term, &ProfileVocab)> {
        self.profiles
            .iter()
            .find_map(|p| p.term(iri).map(|t| (t, &**p)))
    }

    /// [`Wording::lookup`] for a term as a policy writes it.
    pub fn lookup_written(&self, p: &Prefixes, written: &str) -> Option<(&Term, &ProfileVocab)> {
        self.lookup(&self.expand(p, written))
    }

    /// `written` as an IRI: a full IRI as it is; a compact one with the
    /// policy's prefixes, then the loaded profiles' own prefixes in load
    /// order; a bare word or `odrl:` term in the ODRL namespace unless the
    /// policy binds it otherwise. An unbound prefix is kept as written.
    pub fn expand(&self, p: &Prefixes, written: &str) -> String {
        if let Some(iri) = p.try_expand(written) {
            return iri;
        }
        if let Some((prefix, suffix)) = written.split_once(':') {
            for vocab in self.profiles() {
                if let Some(iri) = vocab.prefixes.get(prefix) {
                    return format!("{iri}{suffix}");
                }
            }
        }
        written.to_string()
    }

    /// Whether the profile `profile_iri` is loaded. A trailing `#` or `/` is
    /// ignored; ODRL's core profile is always covered.
    pub fn covers(&self, profile_iri: &str) -> bool {
        let want = trim(profile_iri);
        if want == trim(ns::ODRL) || want == format!("{}core", ns::ODRL) {
            return true;
        }
        self.profiles().any(|p| p.iri().map(trim) == Some(want))
    }

    /// One message for every term two loaded profiles both declare.
    pub fn conflicts(&self) -> &[String] {
        &self.conflicts
    }

    /// Whether no profile is loaded: the wording is the built-in one.
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}

fn trim(iri: &str) -> &str {
    iri.trim_end_matches(['#', '/'])
}

fn name(p: &ProfileVocab) -> String {
    match p.iri() {
        Some(i) => format!("<{i}>"),
        None => "an unnamed profile".to_string(),
    }
}

/// The prefixes and term definitions a policy document's `@context` binds.
/// Only inline contexts are read; the W3C ODRL context, named as a remote
/// context, adds the prefixes it is known to bind.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prefixes {
    /// Prefix or term -> IRI. A map, so a large `@context` costs its size.
    entries: BTreeMap<String, String>,
}

impl Prefixes {
    /// The contexts of `v`, of its `@graph` items, or of each node of a
    /// top-level array, in document order; the first binding of a key wins.
    /// Within one `@context` the later entry wins, as in JSON-LD.
    pub fn of_document(v: &Value) -> Prefixes {
        let mut nodes: Vec<&Value> = Vec::new();
        let tops: Vec<&Value> = match v {
            Value::Array(a) => a.iter().collect(),
            other => vec![other],
        };
        for top in tops {
            nodes.push(top);
            if let Some(g) = top.get("@graph") {
                match g {
                    Value::Array(a) => nodes.extend(a),
                    other => nodes.push(other),
                }
            }
        }
        let mut out = Prefixes::default();
        for n in nodes {
            if let Some(c) = n.get("@context") {
                out.merge(Prefixes::of_context(c));
            }
        }
        out
    }

    /// `term` as an IRI; see [`Wording::expand`]. Without a profile, an
    /// unbound prefix is kept as written.
    pub fn expand(&self, term: &str) -> String {
        self.try_expand(term).unwrap_or_else(|| term.to_string())
    }

    /// `iri` as a policy with these prefixes would write it: an ODRL term
    /// bare, any other as a compact IRI with a prefix bound here, never with
    /// an invented prefix. `None` when no bound prefix fits.
    pub fn compact(&self, iri: &str) -> Option<String> {
        let _ = iri;
        None
    }

    /// `None` for a compact IRI whose prefix nothing here binds.
    pub(crate) fn try_expand(&self, term: &str) -> Option<String> {
        match term.split_once(':') {
            Some((prefix, suffix)) => {
                if prefix == "_" || suffix.starts_with("//") {
                    return Some(term.to_string());
                }
                if let Some(iri) = self.get(prefix) {
                    return Some(format!("{iri}{suffix}"));
                }
                (prefix == "odrl").then(|| ns::odrl(suffix))
            }
            None => Some(
                self.get(term)
                    .map_or_else(|| ns::odrl(term), str::to_string),
            ),
        }
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    /// Adds `other`'s keys that are not bound here yet.
    pub(crate) fn merge(&mut self, other: Prefixes) {
        for (k, v) in other.entries {
            self.entries.entry(k).or_insert(v);
        }
    }

    fn set(&mut self, key: &str, iri: String) {
        self.entries.insert(key.to_string(), iri);
    }

    /// One `@context` value: a string, an object, or an array of them.
    pub(crate) fn of_context(c: &Value) -> Prefixes {
        let mut out = Prefixes::default();
        let list: Vec<&Value> = match c {
            Value::Array(a) => a.iter().collect(),
            other => vec![other],
        };
        for c in list {
            match c {
                Value::String(s) if ns::is_odrl_context(s) => {
                    for (p, iri) in ns::ODRL_CONTEXT {
                        out.set(p, iri.to_string());
                    }
                }
                Value::Object(o) => {
                    for (k, v) in o.iter().filter(|(k, _)| !k.starts_with('@')) {
                        let iri = match v {
                            Value::String(s) => Some(s.as_str()),
                            Value::Object(d) => d.get("@id").and_then(Value::as_str),
                            _ => None,
                        };
                        match iri {
                            Some(i) if !i.starts_with('@') => out.set(k, i.to_string()),
                            _ => {
                                out.entries.remove(k);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        // Values written as compact IRIs ("fineTune": "ex:fineTune"); a few
        // passes resolve chains.
        for _ in 0..4 {
            let snapshot = out.clone();
            for (k, v) in &mut out.entries {
                if let Some((prefix, suffix)) = v.split_once(':')
                    && prefix != k
                    && !suffix.starts_with("//")
                    && let Some(iri) = snapshot.get(prefix)
                {
                    *v = format!("{iri}{suffix}");
                }
            }
        }
        out
    }
}
