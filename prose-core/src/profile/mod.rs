//! ODRL profiles: the vocabulary a policy's `profile` points to, read so the
//! prose can use the profile's own labels and definitions.
//!
//! A profile widens and annotates. It never closes an input: terms it does
//! not declare still read and edit as before, and anything a profile says
//! about a policy is advisory. Profile text is third-party input and is made
//! plain text before it is kept.
//!
//! Profiles are read as JSON-LD without a JSON-LD processor: inline
//! `@context` prefixes, `@vocab`, `@base` and simple term definitions are
//! followed, remote contexts are not. With the `turtle` feature, a text
//! that is not JSON is read as Turtle. Every syntax is first read into a
//! private triple list, and term semantics work on that list only, so a
//! Turtle profile and its JSON-LD twin read as equal [`ProfileVocab`]s.
//!
//! ```
//! use prose_core::profile::{ProfileVocab, Wording};
//!
//! let profile = ProfileVocab::parse(r#"{
//!   "@context": { "odrl": "http://www.w3.org/ns/odrl/2/",
//!                 "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
//!                 "ex": "https://example.org/profile/" },
//!   "@id": "ex:fineTune", "@type": "odrl:Action", "rdfs:label": "Fine-tune"
//! }"#)?;
//! let wording = Wording::new().with_profile(profile);
//! let policy = r#"{ "@context": { "ex": "https://example.org/profile/" },
//!   "@type": "Set", "uid": "urn:policy:1",
//!   "permission": [{ "action": "ex:fineTune", "target": "urn:asset:1" }] }"#;
//! let reading = prose_core::render_with(policy, &wording)?;
//! assert!(reading.document.policies[0].rules[0].sentence.contains("may fine-tune"));
//! assert_eq!(reading.glossary[0].terms[0].label, "Fine-tune");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! The ds-prose extension (`prose:operator`, `prose:rightOperand`,
//! `prose:datatype`, `prose:unit` in
//! `https://ds-labs-org.github.io/ds-prose-rs/profile#`) is a ds-prose
//! convention, not ODRL: on a left operand it orders the editor's
//! suggestions ([`OperandHints`]) and never restricts them.
//!
//! Every read is bounded by [`ProfileLimits`]; past a limit the profile is
//! refused, never half read.
use serde_json::Value;

mod build;
mod jsonld;
mod ns;
mod text;
mod triples;
#[cfg(feature = "turtle")]
mod turtle;
mod wording;

pub use text::LangText;
pub use wording::{Prefixes, Wording};

/// The vocabulary one ODRL profile declares.
///
/// Equality compares the vocabulary as a graph, not how it was written. An
/// RDF graph has no order, so the order of the terms, of the language pairs
/// in each [`LangText`] and of every IRI list is left out, and so are the
/// prefixes a document bound and its parse warnings: a compacted and an
/// expanded copy of one profile are equal, and so are a JSON-LD profile and
/// a Turtle twin a tool wrote in its own order. [`OperandHints::value_tree`]
/// is worked out from the rest and is not compared. The derived `==` of
/// [`Term`] and [`LangText`] still compares in order.
#[derive(Clone, Debug, Default)]
pub struct ProfileVocab {
    iri: Option<String>,
    title: LangText,
    description: LangText,
    version: Option<String>,
    imports: Vec<String>,
    terms: Vec<Term>,
    /// Term IRI -> index in `terms`.
    index: std::collections::HashMap<String, usize>,
    /// The prefixes the profile text bound: `Wording` expands a policy's
    /// compact terms with them when the policy does not bind the prefix.
    prefixes: std::collections::HashMap<String, String>,
    warnings: Vec<String>,
}

impl PartialEq for ProfileVocab {
    fn eq(&self, other: &ProfileVocab) -> bool {
        self.iri == other.iri
            && same_text(&self.title, &other.title)
            && same_text(&self.description, &other.description)
            && self.version == other.version
            && same_set(&self.imports, &other.imports)
            && self.terms.len() == other.terms.len()
            // Term IRIs are unique, so equal counts and every term found make
            // a one-to-one match.
            && self
                .terms
                .iter()
                .all(|t| other.term(&t.iri).is_some_and(|u| same_term(t, u)))
    }
}

/// `a` and `b` hold the same (language, text) pairs, in any order.
fn same_text(a: &LangText, b: &LangText) -> bool {
    fn sorted(t: &LangText) -> Vec<(Option<&str>, &str)> {
        let mut v: Vec<_> = t.iter().collect();
        v.sort_unstable();
        v
    }
    sorted(a) == sorted(b)
}

/// `a` and `b` hold the same IRIs, in any order. Every list here is read
/// without duplicates.
fn same_set(a: &[String], b: &[String]) -> bool {
    fn sorted(v: &[String]) -> Vec<&str> {
        let mut v: Vec<&str> = v.iter().map(String::as_str).collect();
        v.sort_unstable();
        v
    }
    a.len() == b.len() && sorted(a) == sorted(b)
}

/// One term, compared as a graph; see [`ProfileVocab`]'s equality.
fn same_term(a: &Term, b: &Term) -> bool {
    a.iri == b.iri
        && a.kinds == b.kinds
        && same_text(&a.label, &b.label)
        && same_text(&a.definition, &b.definition)
        && same_text(&a.note, &b.note)
        && same_set(&a.included_in, &b.included_in)
        && same_set(&a.broader, &b.broader)
        && same_set(&a.implies, &b.implies)
        && same_set(&a.exact_match, &b.exact_match)
        && a.deprecated == b.deprecated
        && same_set(&a.hints.operators, &b.hints.operators)
        && same_set(&a.hints.values, &b.hints.values)
        && a.hints.datatype == b.hints.datatype
        && same_set(&a.hints.units, &b.hints.units)
}

impl ProfileVocab {
    /// Reads a profile written as JSON-LD (or, with the `turtle` feature, as
    /// Turtle), with the default limits. The syntax is told from the first
    /// characters: `{` or `[` is JSON-LD.
    pub fn parse(text: &str) -> Result<ProfileVocab, ProfileError> {
        ProfileVocab::parse_with(text, &ProfileLimits::default())
    }

    /// As [`ProfileVocab::parse`], with explicit limits.
    ///
    /// A relative IRI (`"@id": "#train"`, `<#train>`) is made absolute by
    /// the document's own `@base`. Without one, it cannot be, and the node
    /// or value is skipped with a warning, in either syntax; a profile
    /// fetched from a URL is read with [`ProfileVocab::parse_with_base`].
    pub fn parse_with(text: &str, limits: &ProfileLimits) -> Result<ProfileVocab, ProfileError> {
        ProfileVocab::read(text, None, limits)
    }

    /// As [`ProfileVocab::parse_with`], for a text whose own address is
    /// `base` (the URL it was fetched from): relative IRIs are resolved
    /// against it, unless the document sets its own `@base`. A `base` that
    /// is not an absolute IRI is not used, with a warning.
    pub fn parse_with_base(
        text: &str,
        base: &str,
        limits: &ProfileLimits,
    ) -> Result<ProfileVocab, ProfileError> {
        ProfileVocab::read(text, Some(base), limits)
    }

    fn read(
        text: &str,
        base: Option<&str>,
        limits: &ProfileLimits,
    ) -> Result<ProfileVocab, ProfileError> {
        let text = within_size(text, limits)?;
        match sniff(text)? {
            Format::Json => {
                let value: Value =
                    serde_json::from_str(text).map_err(|e| ProfileError::Json(e.to_string()))?;
                build::vocab(jsonld::read(&value, base, limits)?, limits)
            }
            #[cfg(feature = "turtle")]
            Format::Turtle => turtle::vocab(text, base, limits),
        }
    }

    /// Reads a profile written in Turtle, without guessing the syntax (for
    /// a host that knows it from a `text/turtle` content type).
    #[cfg(feature = "turtle")]
    pub fn parse_turtle(text: &str, limits: &ProfileLimits) -> Result<ProfileVocab, ProfileError> {
        let text = within_size(text, limits)?;
        turtle::vocab(text, None, limits)
    }

    /// As [`ProfileVocab::parse_turtle`], with relative IRIs resolved
    /// against `base`; see [`ProfileVocab::parse_with_base`].
    #[cfg(feature = "turtle")]
    pub fn parse_turtle_with_base(
        text: &str,
        base: &str,
        limits: &ProfileLimits,
    ) -> Result<ProfileVocab, ProfileError> {
        let text = within_size(text, limits)?;
        turtle::vocab(text, Some(base), limits)
    }

    /// As [`ProfileVocab::parse`], for a value that is already parsed.
    pub fn from_value(v: &Value, limits: &ProfileLimits) -> Result<ProfileVocab, ProfileError> {
        let parsed = jsonld::read(v, None, limits)?;
        build::vocab(parsed, limits)
    }

    /// The profile's own IRI: the node typed `owl:Ontology`, `prof:Profile`
    /// or `odrl:Profile`, else the IRI most terms name in `rdfs:isDefinedBy`.
    /// A blank node is never an IRI here; an anonymous header still gives
    /// the title and description.
    pub fn iri(&self) -> Option<&str> {
        self.iri.as_deref()
    }

    /// The profile's name: `rdfs:label`, else `skos:prefLabel`, else
    /// `dct:title` on the profile node.
    pub fn title(&self) -> &LangText {
        &self.title
    }

    /// `skos:definition`, else `rdfs:comment`, else `dct:description` on the
    /// profile node.
    pub fn description(&self) -> &LangText {
        &self.description
    }

    /// `owl:versionInfo`, else `dct:hasVersion`.
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// `owl:imports` and `prof:isProfileOf` on the profile node: recorded,
    /// never fetched.
    pub fn imports(&self) -> &[String] {
        &self.imports
    }

    /// The term with this expanded IRI. Compact IRIs are not looked up here.
    pub fn term(&self, iri: &str) -> Option<&Term> {
        self.index.get(iri).map(|&i| &self.terms[i])
    }

    /// Every term, in document order.
    pub fn terms(&self) -> impl Iterator<Item = &Term> {
        self.terms.iter()
    }

    /// The terms of one kind, in document order.
    pub fn terms_of(&self, kind: TermKind) -> impl Iterator<Item = &Term> {
        self.terms.iter().filter(move |t| t.kinds.contains(&kind))
    }

    /// What was not read, or read with a guess: lower-case fragments, each
    /// once.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Counts per kind, for a status line.
    pub fn summary(&self) -> ProfileSummary {
        let mut counts: Vec<(TermKind, usize)> = Vec::new();
        for k in self.terms.iter().flat_map(|t| &t.kinds) {
            match counts.iter_mut().find(|(c, _)| c == k) {
                Some((_, n)) => *n += 1,
                None => counts.push((*k, 1)),
            }
        }
        counts.sort();
        ProfileSummary {
            iri: self.iri.clone(),
            version: self.version.clone(),
            terms: self.terms.len(),
            counts,
            warnings: self.warnings.len(),
        }
    }
}

/// What a loaded profile holds, in numbers.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileSummary {
    pub iri: Option<String>,
    pub version: Option<String>,
    pub terms: usize,
    /// (kind, number of terms of that kind) for every kind present, in
    /// [`TermKind`] order. A term of two kinds counts under both.
    pub counts: Vec<(TermKind, usize)>,
    pub warnings: usize,
}

impl ProfileSummary {
    /// The number of terms of `kind`.
    pub fn count(&self, kind: TermKind) -> usize {
        self.counts
            .iter()
            .find(|(k, _)| *k == kind)
            .map_or(0, |(_, n)| *n)
    }
}

/// One term a profile declares.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct Term {
    pub iri: String,
    pub kinds: Vec<TermKind>,
    /// `rdfs:label`, else `skos:prefLabel`, else `dct:title`.
    pub label: LangText,
    /// `skos:definition`, else `rdfs:comment`, else `dct:description`.
    pub definition: LangText,
    /// `skos:note`, else `skos:scopeNote` (other than "Non-Normative"), else
    /// `skos:example`.
    pub note: LangText,
    /// `odrl:includedIn`.
    pub included_in: Vec<String>,
    /// `skos:broader`, `rdfs:subClassOf` and `rdfs:subPropertyOf`.
    pub broader: Vec<String>,
    /// `odrl:implies`: shown, never read as inclusion.
    pub implies: Vec<String>,
    /// `skos:exactMatch`.
    pub exact_match: Vec<String>,
    /// `owl:deprecated true`.
    pub deprecated: bool,
    /// The ds-prose extension's suggestions; empty without it.
    pub hints: OperandHints,
}

impl Term {
    pub(crate) fn new(iri: String) -> Term {
        Term {
            iri,
            kinds: Vec::new(),
            label: LangText::default(),
            definition: LangText::default(),
            note: LangText::default(),
            included_in: Vec::new(),
            broader: Vec::new(),
            implies: Vec::new(),
            exact_match: Vec::new(),
            deprecated: false,
            hints: OperandHints::default(),
        }
    }

    /// The label in the best language for `langs`, else the IRI's local
    /// name, humanised.
    pub fn display_label(&self, langs: &[String]) -> String {
        match self.label.pick(langs) {
            Some(l) => l.to_string(),
            None => crate::words::humanise(crate::words::local_name(&self.iri)),
        }
    }
}

/// Suggestions a profile makes for one left operand through the ds-prose
/// extension (`prose:`). A ds-prose convention, not ODRL; never exclusive.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OperandHints {
    /// `prose:operator`: operators to offer first.
    pub operators: Vec<String>,
    /// `prose:rightOperand`: suggested values, as the profile lists them.
    pub values: Vec<String>,
    /// `prose:datatype`: the first one given, an xsd IRI.
    pub datatype: Option<String>,
    /// `prose:unit`: suggested units.
    pub units: Vec<String>,
    /// `values` with their descendants within the profile (right operands
    /// whose `skos:broader` or `rdfs:subClassOf` leads to them), depth
    /// first, each with its depth below the suggested value (0). Each value
    /// appears once, at its first place.
    pub value_tree: Vec<(String, usize)>,
}

/// What a term is, as far as ODRL is concerned. One term can be several.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TermKind {
    Action,
    LeftOperand,
    Operator,
    RightOperand,
    ConflictTerm,
    PolicyClass,
    RuleClass,
    AssetClass,
    PartyClass,
    PartyFunction,
    AssetRelation,
    LogicalOperand,
}

/// Bounds on what one profile may cost to read.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct ProfileLimits {
    /// Text longer than this is refused before it is parsed.
    pub max_bytes: usize,
    /// More terms than this is refused; a half-read vocabulary is never
    /// returned.
    pub max_terms: usize,
    /// Nesting deeper than this is not read.
    pub max_depth: usize,
    /// Values of one property on one node beyond this are not read.
    pub max_values_per_key: usize,
    pub max_label_chars: usize,
    pub max_text_chars: usize,
    /// The IRIs and literals read from the text, counted in bytes after
    /// prefixes and term definitions are expanded; past this the profile
    /// is refused. A short prefix bound to a long namespace would otherwise
    /// make a small text cost gigabytes. A value past `max_values_per_key`
    /// is not kept, but its subject and property still count.
    pub max_expanded_bytes: usize,
}

impl Default for ProfileLimits {
    fn default() -> ProfileLimits {
        ProfileLimits {
            max_bytes: 4 * 1024 * 1024,
            max_terms: 20_000,
            max_depth: 64,
            max_values_per_key: 256,
            max_label_chars: 200,
            max_text_chars: 4_000,
            max_expanded_bytes: 32 * 1024 * 1024,
        }
    }
}

/// Why a text could not be read as a profile.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileError {
    TooLarge {
        bytes: usize,
        limit: usize,
    },
    Json(String),
    LooksLikeHtml,
    ContextOnly,
    JsonSchema,
    LooksLikePolicy,
    NoTerms,
    UnsupportedFormat,
    LimitExceeded(&'static str),
    /// The text was read as Turtle (the `turtle` feature) and is not valid
    /// Turtle.
    Turtle(String),
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileError::TooLarge { bytes, limit } => write!(
                f,
                "the profile is {bytes} bytes, more than the limit of {limit}; load a smaller profile, or raise ProfileLimits::max_bytes for one you trust"
            ),
            ProfileError::Json(e) => write!(
                f,
                "not valid JSON: {e}. A profile is read as JSON-LD (W3C publishes ODRL itself as ODRL22.json)"
            ),
            ProfileError::LooksLikeHtml => write!(
                f,
                "this looks like an HTML page, not a profile: the server probably sent its landing page. Download the profile as JSON-LD (a .jsonld or .json file) instead"
            ),
            ProfileError::ContextOnly => write!(
                f,
                "this document holds only a @context: it maps short names to IRIs but declares no vocabulary terms. Load the profile's vocabulary, the document that declares its actions and operands, instead"
            ),
            ProfileError::JsonSchema => write!(
                f,
                "this is a JSON Schema: it checks how a policy is written, it does not declare ODRL terms. Load the profile's JSON-LD vocabulary instead"
            ),
            ProfileError::LooksLikePolicy => write!(
                f,
                "this is an ODRL policy, not a profile: read it as the policy, and load here the vocabulary its profile names"
            ),
            ProfileError::NoTerms => write!(
                f,
                "no ODRL terms found: a profile declares terms typed odrl:Action, odrl:LeftOperand, odrl:Operator, odrl:RightOperand or odrl:ConflictTerm"
            ),
            ProfileError::UnsupportedFormat => write!(f, "{UNSUPPORTED}"),
            ProfileError::Turtle(e) => write!(
                f,
                "not valid Turtle: {e}. A text that does not start with {{ or [ is read as Turtle; a JSON-LD profile starts with one of them"
            ),
            ProfileError::LimitExceeded(what) => write!(
                f,
                "the profile goes past the reader's limit {what}, so none of it was read; raise ProfileLimits::{what} for a profile you trust"
            ),
        }
    }
}

#[cfg(not(feature = "turtle"))]
const UNSUPPORTED: &str = "this is not JSON-LD, and Turtle is not read in this build: convert the profile to JSON-LD (W3C publishes ODRL itself as ODRL22.json)";
#[cfg(feature = "turtle")]
const UNSUPPORTED: &str = "this is neither JSON-LD nor Turtle (RDF/XML is not read): convert the profile to JSON-LD or Turtle (W3C publishes ODRL itself as ODRL22.json and ODRL22.ttl)";

/// `text` without a byte-order mark, if it is within `max_bytes`. Checked
/// before any parsing.
fn within_size<'a>(text: &'a str, limits: &ProfileLimits) -> Result<&'a str, ProfileError> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    if text.len() > limits.max_bytes {
        return Err(ProfileError::TooLarge {
            bytes: text.len(),
            limit: limits.max_bytes,
        });
    }
    Ok(text)
}

/// The syntaxes a profile text can be read in.
enum Format {
    Json,
    #[cfg(feature = "turtle")]
    Turtle,
}

/// Which syntax `text` is in, from its first characters. Markup is refused
/// with a specific error; RDF/XML and other syntaxes are not read.
fn sniff(text: &str) -> Result<Format, ProfileError> {
    let t = text.trim_start();
    match t.as_bytes().first() {
        Some(b'{') | None => Ok(Format::Json),
        Some(b'[') if json_array(t) => Ok(Format::Json),
        Some(b'<') if t.starts_with("<?xml") || t.starts_with("<rdf:RDF") => {
            Err(ProfileError::UnsupportedFormat)
        }
        Some(b'<') if looks_like_markup(t) => Err(ProfileError::LooksLikeHtml),
        #[cfg(feature = "turtle")]
        _ => Ok(Format::Turtle),
        #[cfg(not(feature = "turtle"))]
        _ => Err(ProfileError::UnsupportedFormat),
    }
}

/// `<!doctype ..>`, `<!-- ..`, a tag with attributes (`<p class=..>`), or
/// an HTML element such as `<html>`; not an IRI in angle brackets
/// (`<https://..>`, or a relative `<train>`).
fn looks_like_markup(t: &str) -> bool {
    const ELEMENTS: &[&str] = &[
        "html", "head", "body", "meta", "title", "link", "script", "style", "div", "p", "span",
        "h1", "h2", "h3", "pre", "main", "header", "nav", "section", "article", "table", "br",
    ];
    let rest = &t[1..];
    if rest.starts_with('!') {
        return true;
    }
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    let after = rest[name.len()..].chars().next();
    if !name.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return false;
    }
    match after {
        Some(' ' | '\t' | '\n' | '\r' | '/') => true,
        Some('>') => ELEMENTS.contains(&name.to_ascii_lowercase().as_str()),
        _ => false,
    }
}

/// Whether a text that starts with `[` is a JSON array rather than a Turtle
/// blank node (`[ a odrl:Action ] .` or `[] ex:p ..`): JSON's first item
/// starts with `{`, `[`, `"`, a digit, `-`, `true`, `false` or `null`, and
/// an empty array ends the text.
fn json_array(t: &str) -> bool {
    let inner = t[1..].trim_start();
    match inner.as_bytes().first() {
        None => true,
        Some(b']') => inner[1..].trim().is_empty(),
        Some(b'{' | b'[' | b'"' | b'-' | b'0'..=b'9') => true,
        Some(_) => ["true", "false", "null"].iter().any(|w| {
            inner
                .strip_prefix(w)
                .is_some_and(|r| r.starts_with(|c: char| c.is_whitespace() || c == ',' || c == ']'))
        }),
    }
}

impl std::error::Error for ProfileError {}
