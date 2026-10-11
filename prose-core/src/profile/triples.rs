//! The private triple list every profile syntax is read into. Term semantics
//! work on this list only, so a second syntax cannot change what a term means.
use super::text::invisible;
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

/// The most warnings one profile keeps; past it, one more line says so.
/// Each quotes the input, and a host lists them all.
const MAX_WARNINGS: usize = 100;

/// Warnings in first-seen order, each once, at most [`MAX_WARNINGS`].
#[derive(Default)]
pub(crate) struct Notes {
    pub list: Vec<String>,
    seen: HashSet<String>,
}

impl Notes {
    /// Adds `msg` once. Warnings quote the text they are about, so every
    /// invisible character in one is written as `\u{..}`.
    pub fn warn(&mut self, msg: impl Into<String>) {
        if self.list.len() >= MAX_WARNINGS {
            if self.list.len() == MAX_WARNINGS {
                self.list.push("more warnings were not listed".to_string());
            }
            return;
        }
        let msg: String = msg.into();
        let msg = match msg.chars().any(invisible) {
            false => msg,
            true => msg.chars().map(escaped).collect(),
        };
        if self.seen.insert(msg.clone()) {
            self.list.push(msg);
        }
    }
}

/// `c`, or `\u{..}` for an invisible one.
fn escaped(c: char) -> String {
    match invisible(c) {
        true => format!("\\u{{{:x}}}", c as u32),
        false => c.to_string(),
    }
}

/// Whether `iri` holds only what an IRI may (RFC 3987): no whitespace, no
/// invisible character (bidi controls among them) and none of
/// `<>"{}|\^` and the backquote. A label made from such an IRI could spoof
/// or hide text, so it is skipped, not cleaned.
fn iri_ok(iri: &str) -> bool {
    !iri.chars().any(|c| {
        c.is_whitespace()
            || invisible(c)
            || matches!(c, '<' | '>' | '"' | '{' | '}' | '|' | '\\' | '^' | '`')
    })
}

/// `iri` for a warning: what it cannot hold percent-encoded, or as
/// `\u{..}` when invisible; at most 200 characters.
fn shown(iri: &str) -> String {
    let mut out = String::new();
    for c in iri.chars().take(200) {
        match c {
            c if invisible(c) => out.push_str(&escaped(c)),
            c if c.is_whitespace() || "<>\"{}|\\^`".contains(c) => {
                let mut b = [0; 4];
                for byte in c.encode_utf8(&mut b).bytes() {
                    out.push_str(&format!("%{byte:02X}"));
                }
            }
            c => out.push(c),
        }
    }
    if iri.chars().nth(200).is_some() {
        out.push('\u{2026}');
    }
    out
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
        let datatype = match &o {
            Object::Literal { datatype, .. } => datatype.as_deref(),
            Object::Iri(_) => None,
        };
        let o_iri = match &o {
            Object::Iri(i) => Some(i.as_str()),
            Object::Literal { .. } => None,
        };
        if let Some(bad) = [Some(s), Some(p), o_iri, datatype]
            .into_iter()
            .flatten()
            .find(|i| !iri_ok(i))
        {
            self.parsed.notes.warn(format!(
                "skipped the IRI {}: it holds characters an IRI cannot hold (whitespace, invisible characters or markup delimiters)",
                shown(bad)
            ));
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
        if !iri_ok(prefix) || !iri_ok(ns) {
            return;
        }
        if self.bound.insert(prefix.to_string()) {
            self.parsed
                .prefixes
                .push((prefix.to_string(), ns.to_string()));
        }
    }
}
