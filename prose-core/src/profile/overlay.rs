//! The operator overlay: a small annotation-only graph in the `prose:`
//! namespace, published by a dataspace's governance next to its profile and
//! versioned with it. It holds `prose:` triples (`prose:operator`,
//! `prose:rightOperand`, `prose:unit`, `prose:datatype`, `prose:closed`,
//! `prose:plural`) about terms declared elsewhere.
//!
//! A second profile that names the same IRI is a "declared by both"
//! conflict. An overlay is not a second declaration: it merges its hints onto
//! the left operands the loaded profiles declare, through
//! [`Wording::with_overlay`](super::Wording::with_overlay), and never adds or
//! relabels a term. Triples about an IRI no loaded profile declares as a left
//! operand are ignored with a warning
//! ([`Wording::overlay_warnings`](super::Wording::overlay_warnings)), and so
//! is anything else the text says (a label, a type): only `prose:` triples
//! are read.
//!
//! An overlay lists exactly the values it means, as `sh:in` does: no
//! descendants of a listed value are added. Without `prose:closed true` its
//! hints are suggestions, as a profile's are; with it they are binding in
//! strict validation, and the overlay closes the left operand.
use super::triples::{Notes, Object, Parsed};
use super::{OperandHints, ProfileError, ProfileLimits, ProfileVocab, ns};
use serde_json::Value;
use std::collections::HashMap;

/// What one overlay document says about each left operand it names.
#[derive(Clone, Debug, Default)]
pub struct Overlay {
    terms: Vec<(String, OperandHints)>,
    warnings: Vec<String>,
}

impl PartialEq for Overlay {
    /// The overlay as a graph: the order of the terms and of their lists, and
    /// the warnings, are left out.
    fn eq(&self, other: &Overlay) -> bool {
        fn sorted(v: &[String]) -> Vec<&str> {
            let mut v: Vec<&str> = v.iter().map(String::as_str).collect();
            v.sort_unstable();
            v
        }
        self.terms.len() == other.terms.len()
            && self.terms.iter().all(|(iri, a)| {
                other.hints(iri).is_some_and(|b| {
                    sorted(&a.operators) == sorted(&b.operators)
                        && sorted(&a.values) == sorted(&b.values)
                        && sorted(&a.units) == sorted(&b.units)
                        && a.datatype == b.datatype
                        && a.closed == b.closed
                        && a.plural == b.plural
                })
            })
    }
}

impl Overlay {
    /// Reads an overlay written as JSON-LD (or, with the `turtle` feature, as
    /// Turtle), with the default limits. The syntax is told from the first
    /// characters, as for a profile.
    pub fn parse(text: &str) -> Result<Overlay, OverlayError> {
        Overlay::parse_with(text, &ProfileLimits::default())
    }

    /// As [`Overlay::parse`], with explicit limits.
    pub fn parse_with(text: &str, limits: &ProfileLimits) -> Result<Overlay, OverlayError> {
        interpret(ProfileVocab::read_triples(text, None, limits)?, limits)
    }

    /// As [`Overlay::parse`], for a value that is already parsed.
    pub fn from_value(v: &Value, limits: &ProfileLimits) -> Result<Overlay, OverlayError> {
        interpret(super::jsonld_triples(v, limits)?, limits)
    }

    /// The terms the overlay names, with the hints it gives each, in
    /// first-seen order. Only the facets the overlay gives are set.
    pub fn terms(&self) -> impl Iterator<Item = (&str, &OperandHints)> {
        self.terms.iter().map(|(iri, h)| (iri.as_str(), h))
    }

    /// What the overlay says about the term with this expanded IRI.
    pub fn hints(&self, iri: &str) -> Option<&OperandHints> {
        self.terms.iter().find(|(i, _)| i == iri).map(|(_, h)| h)
    }

    /// What was not read: lower-case fragments, each once. Which terms no
    /// loaded profile declares depends on the profiles; see
    /// [`Wording::overlay_warnings`](super::Wording::overlay_warnings).
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Whether the overlay names no term.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }
}

fn prose(local: &str) -> String {
    format!("{}{local}", ns::PROSE)
}

fn interpret(parsed: Parsed, limits: &ProfileLimits) -> Result<Overlay, OverlayError> {
    let Parsed {
        triples,
        notes: Notes { list: warnings, .. },
        ..
    } = parsed;
    let annotations: [(&str, String); 6] = [
        ("operator", prose("operator")),
        ("rightOperand", prose("rightOperand")),
        ("unit", prose("unit")),
        ("datatype", prose("datatype")),
        ("closed", prose("closed")),
        ("plural", prose("plural")),
    ];
    let mut out = Overlay {
        terms: Vec::new(),
        warnings,
    };
    let mut at: HashMap<&str, usize> = HashMap::new();
    let mut unread: Vec<String> = Vec::new();
    for t in &triples {
        let Some(local) = t.p.strip_prefix(ns::PROSE) else {
            // Anything else is not the overlay's to say; a label or a type
            // would add or rename a term, so say it is ignored.
            let what = if t.p == ns::RDF_TYPE {
                Some("a type")
            } else if t.p.starts_with(ns::RDFS) || t.p.starts_with(ns::SKOS) {
                Some("a label or a note")
            } else {
                None
            };
            if let Some(what) = what {
                let msg = format!(
                    "ignored {what} on <{}>: an overlay never adds or relabels a term",
                    t.s
                );
                if !unread.contains(&msg) {
                    unread.push(msg);
                }
            }
            continue;
        };
        if t.s.starts_with("_:") {
            continue;
        }
        let Some((_, _)) = annotations.iter().find(|(l, _)| *l == local) else {
            let msg = format!(
                "ignored prose:{local}: an overlay reads only operator, rightOperand, unit, datatype, closed and plural"
            );
            if !unread.contains(&msg) {
                unread.push(msg);
            }
            continue;
        };
        let i = *at.entry(&t.s).or_insert_with(|| {
            out.terms.push((t.s.clone(), OperandHints::default()));
            out.terms.len() - 1
        });
        let h = &mut out.terms[i].1;
        match (&t.o, local) {
            (Object::Iri(iri), "operator") if !h.operators.contains(iri) => {
                h.operators.push(iri.clone());
            }
            (Object::Iri(iri), "rightOperand") if !h.values.contains(iri) => {
                h.values.push(iri.clone());
            }
            (Object::Iri(iri), "unit") if !h.units.contains(iri) => h.units.push(iri.clone()),
            (Object::Iri(iri), "datatype") if h.datatype.is_none() => {
                h.datatype = Some(iri.clone());
            }
            (Object::Literal { value, .. }, "closed") => {
                h.closed |= matches!(value.trim(), "true" | "1");
            }
            (Object::Literal { value, .. }, "plural") => {
                h.plural |= matches!(value.trim(), "true" | "1");
            }
            _ => {}
        }
    }
    if out.terms.is_empty() {
        return Err(OverlayError::NotOverlay(
            "no prose: annotation found: nothing in this text is in the ds-prose extension namespace"
                .to_string(),
        ));
    }
    if out.terms.len() > limits.max_terms {
        return Err(OverlayError::LimitExceeded("max_terms"));
    }
    for h in out.terms.iter_mut().map(|(_, h)| h) {
        // An overlay lists exactly its values, like `sh:in`.
        h.value_tree = h.values.iter().map(|v| (v.clone(), 0)).collect();
    }
    for msg in unread {
        if !out.warnings.contains(&msg) {
            out.warnings.push(msg);
        }
    }
    Ok(out)
}

/// Why a text could not be read as an overlay.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OverlayError {
    /// The text is larger than the byte limit.
    TooLarge {
        /// The size of the text.
        bytes: usize,
        /// The limit it exceeded.
        limit: usize,
    },
    /// The text is not valid JSON; carries the parser's message.
    Json(String),
    /// The text looks like an HTML page, not overlay.
    LooksLikeHtml,
    /// The text is Turtle and the `turtle` feature is off, or its format is unknown.
    UnsupportedFormat,
    /// A limit on the number of entries was exceeded; names the limit.
    LimitExceeded(&'static str),
    /// The text was read as Turtle (the `turtle` feature) and is not valid
    /// Turtle.
    Turtle(String),
    /// The text was read, and holds no `prose:` annotation; the reason says
    /// what it is instead.
    NotOverlay(String),
}

impl From<ProfileError> for OverlayError {
    fn from(e: ProfileError) -> OverlayError {
        match e {
            ProfileError::TooLarge { bytes, limit } => OverlayError::TooLarge { bytes, limit },
            ProfileError::Json(e) => OverlayError::Json(e),
            ProfileError::LooksLikeHtml => OverlayError::LooksLikeHtml,
            ProfileError::UnsupportedFormat => OverlayError::UnsupportedFormat,
            ProfileError::LimitExceeded(what) => OverlayError::LimitExceeded(what),
            ProfileError::Turtle(e) => OverlayError::Turtle(e),
            ProfileError::ContextOnly => OverlayError::NotOverlay(
                "this document holds only a @context: it maps short names to IRIs but holds no prose: annotation".to_string(),
            ),
            ProfileError::JsonSchema => {
                OverlayError::NotOverlay("this is a JSON Schema, not an overlay".to_string())
            }
            ProfileError::LooksLikePolicy => {
                OverlayError::NotOverlay("this is an ODRL policy, not an overlay".to_string())
            }
            _ => OverlayError::NotOverlay("this is not an overlay".to_string()),
        }
    }
}

impl std::fmt::Display for OverlayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OverlayError::TooLarge { bytes, limit } => write!(
                f,
                "the overlay is {bytes} bytes, more than the limit of {limit}; load a smaller overlay, or raise ProfileLimits::max_bytes for one you trust"
            ),
            OverlayError::Json(e) => write!(
                f,
                "not valid JSON: {e}. An overlay is read as JSON-LD, or as Turtle in the build that reads it"
            ),
            OverlayError::LooksLikeHtml => write!(
                f,
                "this looks like an HTML page, not an overlay: the server probably sent its landing page. Download the overlay as JSON-LD or Turtle instead"
            ),
            OverlayError::UnsupportedFormat => write!(
                f,
                "this is not JSON-LD, and this build does not read it as Turtle: convert the overlay to JSON-LD, or use the build that reads Turtle"
            ),
            OverlayError::Turtle(e) => write!(f, "not valid Turtle: {e}"),
            OverlayError::LimitExceeded(what) => write!(
                f,
                "the overlay goes past the reader's limit {what}, so none of it was read; raise ProfileLimits::{what} for an overlay you trust"
            ),
            OverlayError::NotOverlay(why) => write!(
                f,
                "{why}. An overlay is a graph of prose: annotations (prose:operator, prose:rightOperand, prose:unit, prose:closed ...) about terms a profile declares; load a profile as a profile"
            ),
        }
    }
}

impl std::error::Error for OverlayError {}
