//! The private triple list every profile syntax is read into. Term semantics
//! work on this list only, so a second syntax cannot change what a term means.
use super::{ProfileError, ProfileLimits};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Object {
    Iri(String),
    /// `datatype` is `None` for a plain or language-tagged string.
    Literal {
        value: String,
        lang: Option<String>,
        datatype: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Triple {
    pub s: String,
    pub p: String,
    pub o: Object,
}

/// Warnings in first-seen order, each once.
#[derive(Default)]
pub(crate) struct Notes {
    pub list: Vec<String>,
    seen: HashSet<String>,
}

impl Notes {
    pub fn warn(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if self.seen.insert(msg.clone()) {
            self.list.push(msg);
        }
    }
}

/// What a syntax front end hands to term semantics.
pub(crate) struct Parsed {
    pub triples: Vec<Triple>,
    /// (prefix, namespace) as the document bound them, first binding first.
    pub prefixes: Vec<(String, String)>,
    pub notes: Notes,
}

/// Collects triples and caps the values one subject has for one property.
pub(crate) struct Sink<'a> {
    pub parsed: Parsed,
    /// Subject -> property -> values seen. Nested, so a lookup borrows the
    /// strings instead of copying them.
    counts: HashMap<String, HashMap<String, usize>>,
    /// The prefixes in `parsed.prefixes`, for a constant-time check.
    bound: HashSet<String>,
    limits: &'a ProfileLimits,
    /// Bytes expanded and stored so far, against `max_expanded_bytes`.
    expanded: usize,
}

impl<'a> Sink<'a> {
    pub fn new(limits: &'a ProfileLimits) -> Sink<'a> {
        Sink {
            parsed: Parsed {
                triples: Vec::new(),
                prefixes: Vec::new(),
                notes: Notes::default(),
            },
            counts: HashMap::new(),
            bound: HashSet::new(),
            limits,
            expanded: 0,
        }
    }

    pub fn warn(&mut self, msg: impl Into<String>) {
        self.parsed.notes.warn(msg);
    }

    /// Counts `bytes` against `max_expanded_bytes`.
    pub fn charge(&mut self, bytes: usize) {
        self.expanded = self.expanded.saturating_add(bytes);
    }

    /// Past `max_expanded_bytes`: the reader stops, and the profile is
    /// refused.
    pub fn over(&self) -> bool {
        self.expanded > self.limits.max_expanded_bytes
    }

    /// The error for a reader that stopped because of [`Sink::over`].
    pub fn too_expanded() -> ProfileError {
        ProfileError::LimitExceeded("max_expanded_bytes")
    }

    pub fn push(&mut self, s: &str, p: &str, o: Object) {
        if self.over() {
            return;
        }
        // Looking a value up costs its subject and property, kept or not:
        // a value past the cap below was still read, so that much counts.
        self.charge(s.len() + p.len());
        if self.over() {
            return;
        }
        let per_s = match self.counts.get_mut(s) {
            Some(m) => m,
            None => self.counts.entry(s.to_string()).or_default(),
        };
        let n = match per_s.get_mut(p) {
            Some(n) => n,
            None => per_s.entry(p.to_string()).or_insert(0),
        };
        *n += 1;
        if *n > self.limits.max_values_per_key {
            let max = self.limits.max_values_per_key;
            self.parsed.notes.warn(format!(
                "only the first {max} values of <{p}> on one node are read"
            ));
            return;
        }
        // A kept value also costs what it holds.
        let o_len = match &o {
            Object::Iri(i) => i.len(),
            Object::Literal {
                value,
                lang,
                datatype,
            } => {
                value.len()
                    + lang.as_deref().map_or(0, str::len)
                    + datatype.as_deref().map_or(0, str::len)
            }
        };
        self.charge(o_len);
        if self.over() {
            return;
        }
        self.parsed.triples.push(Triple {
            s: s.to_string(),
            p: p.to_string(),
            o,
        });
    }

    pub fn bind(&mut self, prefix: &str, ns: &str) {
        if self.bound.insert(prefix.to_string()) {
            self.parsed
                .prefixes
                .push((prefix.to_string(), ns.to_string()));
        }
    }
}
