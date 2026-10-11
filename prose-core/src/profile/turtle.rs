//! Turtle to triples (the `turtle` feature), through oxttl. The triples go
//! into the same sink as JSON-LD's, so the per-key cap and every later rule
//! apply unchanged. oxttl keeps its parse stack on the heap, so nesting is
//! bounded by `max_bytes`, not by the call stack.
use super::triples::{Object, Parsed, Sink};
use super::{ProfileError, ProfileLimits, ProfileVocab, build, ns};
use oxrdf::{NamedOrBlankNode, Term};
use oxttl::TurtleParser;

/// The base a text is read against when it has none: oxttl refuses a
/// relative IRI without a base, so one under a reserved `.invalid` host
/// (RFC 2606) stands in, and every IRI under it is skipped as relative, as
/// the JSON-LD reader skips a relative `@id`.
const NO_BASE: &str = "https://ds-prose.invalid/";

/// `iri` as the document wrote it, if it was relative and there was no
/// base to resolve it against.
fn relative(iri: &str) -> Option<&str> {
    iri.strip_prefix(NO_BASE)
}

const NO_BASE_WHY: &str = "there is no @base and no base IRI was given";

/// The vocabulary of a Turtle text. A text whose terms were all skipped as
/// relative is refused as Turtle, naming the first such IRI, rather than as
/// one with no terms: the base is what is missing.
pub(crate) fn vocab(
    text: &str,
    base: Option<&str>,
    limits: &ProfileLimits,
) -> Result<ProfileVocab, ProfileError> {
    let (parsed, first_relative) = read(text, base, limits)?;
    match (build::vocab(parsed, limits), first_relative) {
        (Err(ProfileError::NoTerms), Some(rel)) => Err(ProfileError::Turtle(format!(
            "the relative IRI \"{rel}\" cannot be made absolute: {NO_BASE_WHY}"
        ))),
        (v, _) => v,
    }
}

/// `base`: the document's own address, if the host knows it; a `@base` in
/// the text still wins. Also gives the first relative IRI skipped.
fn read(
    text: &str,
    base: Option<&str>,
    limits: &ProfileLimits,
) -> Result<(Parsed, Option<String>), ProfileError> {
    let mut sink = Sink::new(limits);
    let given = base.and_then(|b| match TurtleParser::new().with_base_iri(b) {
        Ok(p) => Some(p),
        Err(_) => {
            sink.warn(format!(
                "the base IRI \"{b}\" is not absolute; relative IRIs are skipped"
            ));
            None
        }
    });
    let parser = match given {
        Some(p) => p,
        None => TurtleParser::new()
            .with_base_iri(NO_BASE)
            .map_err(|e| ProfileError::Turtle(e.to_string()))?,
    };
    let mut first_relative: Option<String> = None;
    let mut parser = parser.for_slice(text.as_bytes());
    for triple in parser.by_ref() {
        let t = triple.map_err(|e| ProfileError::Turtle(e.to_string()))?;
        let s = match &t.subject {
            NamedOrBlankNode::NamedNode(n) => n.as_str().to_string(),
            NamedOrBlankNode::BlankNode(b) => format!("_:{}", b.as_str()),
        };
        let rel = [
            Some(s.as_str()),
            Some(t.predicate.as_str()),
            match &t.object {
                Term::NamedNode(n) => Some(n.as_str()),
                _ => None,
            },
        ]
        .into_iter()
        .flatten()
        .find_map(relative);
        if first_relative.is_none() {
            first_relative = rel.map(String::from);
        }
        if let Some(rel) = relative(&s) {
            sink.warn(format!(
                "skipped the node with the relative IRI \"{rel}\": {NO_BASE_WHY}"
            ));
            continue;
        }
        if let Some(rel) = relative(t.predicate.as_str()) {
            sink.warn(format!("skipped the relative IRI \"{rel}\": {NO_BASE_WHY}"));
            continue;
        }
        #[allow(unreachable_patterns)] // oxrdf's rdf-12 feature adds quoted triples.
        let o = match t.object {
            Term::NamedNode(n) => match relative(n.as_str()) {
                Some(rel) => {
                    sink.warn(format!("skipped the relative IRI \"{rel}\": {NO_BASE_WHY}"));
                    continue;
                }
                None => Object::Iri(n.into_string()),
            },
            Term::BlankNode(b) => Object::Iri(format!("_:{}", b.as_str())),
            Term::Literal(l) => {
                let lang = l.language().map(str::to_ascii_lowercase);
                let datatype = l.datatype().as_str();
                let datatype =
                    (lang.is_none() && datatype != ns::XSD_STRING).then(|| datatype.to_string());
                Object::Literal {
                    value: l.value().to_string(),
                    lang,
                    datatype,
                }
            }
            _ => {
                sink.warn("skipped a quoted triple");
                continue;
            }
        };
        sink.push(&s, t.predicate.as_str(), o);
        if sink.over() {
            return Err(Sink::too_expanded());
        }
    }
    let prefixes: Vec<(String, String)> = parser
        .prefixes()
        .filter(|(_, ns)| relative(ns).is_none())
        .map(|(p, ns)| (p.to_string(), ns.to_string()))
        .collect();
    for (p, ns) in prefixes {
        sink.bind(&p, &ns);
    }
    Ok((sink.parsed, first_relative))
}
