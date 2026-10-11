//! Turtle to triples (the `turtle` feature), through oxttl. The triples go
//! into the same sink as JSON-LD's, so the per-key cap and every later rule
//! apply unchanged. oxttl keeps its parse stack on the heap, so nesting is
//! bounded by `max_bytes`, not by the call stack.
use super::triples::{Object, Parsed, Sink};
use super::{ProfileError, ProfileLimits, ns};
use oxrdf::{NamedOrBlankNode, Term};
use oxttl::TurtleParser;

pub(crate) fn read(text: &str, limits: &ProfileLimits) -> Result<Parsed, ProfileError> {
    let mut sink = Sink::new(limits);
    let mut parser = TurtleParser::new().for_slice(text.as_bytes());
    for triple in parser.by_ref() {
        let t = triple.map_err(|e| ProfileError::Turtle(e.to_string()))?;
        let s = match &t.subject {
            NamedOrBlankNode::NamedNode(n) => n.as_str().to_string(),
            NamedOrBlankNode::BlankNode(b) => format!("_:{}", b.as_str()),
        };
        #[allow(unreachable_patterns)] // oxrdf's rdf-12 feature adds quoted triples.
        let o = match t.object {
            Term::NamedNode(n) => Object::Iri(n.into_string()),
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
        .map(|(p, ns)| (p.to_string(), ns.to_string()))
        .collect();
    for (p, ns) in prefixes {
        sink.bind(&p, &ns);
    }
    Ok(sink.parsed)
}
