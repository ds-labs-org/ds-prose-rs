//! The loaded profiles and language preferences one reading uses, and how a
//! policy's compact terms expand.
use super::{ProfileVocab, Term};
use serde_json::Value;
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

    pub fn with_profile(self, p: ProfileVocab) -> Wording {
        let _ = p;
        self
    }

    pub fn with_languages(self, langs: Vec<String>) -> Wording {
        let _ = langs;
        self
    }

    pub fn languages(&self) -> &[String] {
        &self.languages
    }

    pub fn profiles(&self) -> impl Iterator<Item = &ProfileVocab> {
        self.profiles.iter().map(|p| &**p)
    }

    pub fn lookup(&self, iri: &str) -> Option<(&Term, &ProfileVocab)> {
        let _ = iri;
        None
    }

    pub fn lookup_written(&self, p: &Prefixes, written: &str) -> Option<(&Term, &ProfileVocab)> {
        self.lookup(&self.expand(p, written))
    }

    pub fn expand(&self, p: &Prefixes, written: &str) -> String {
        let _ = p;
        written.to_string()
    }

    pub fn covers(&self, profile_iri: &str) -> bool {
        let _ = profile_iri;
        false
    }

    pub fn conflicts(&self) -> &[String] {
        &self.conflicts
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}

/// The prefixes and term definitions a policy document's `@context` binds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prefixes {
    entries: Vec<(String, String)>,
}

impl Prefixes {
    pub fn of_document(v: &Value) -> Prefixes {
        let _ = v;
        Prefixes::default()
    }

    pub fn expand(&self, term: &str) -> String {
        term.to_string()
    }
}
