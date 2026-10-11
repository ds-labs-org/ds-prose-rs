//! The private triple list every profile syntax is read into. Term semantics
//! work on this list only, so a second syntax cannot change what a term means.
use super::ProfileLimits;
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
    counts: HashMap<(String, String), usize>,
    /// The prefixes in `parsed.prefixes`, for a constant-time check.
    bound: HashSet<String>,
    limits: &'a ProfileLimits,
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
        }
    }

    pub fn warn(&mut self, msg: impl Into<String>) {
        self.parsed.notes.warn(msg);
    }

    pub fn push(&mut self, s: &str, p: &str, o: Object) {
        let n = self
            .counts
            .entry((s.to_string(), p.to_string()))
            .or_insert(0);
        *n += 1;
        if *n > self.limits.max_values_per_key {
            let max = self.limits.max_values_per_key;
            self.parsed.notes.warn(format!(
                "only the first {max} values of <{p}> on one node are read"
            ));
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
