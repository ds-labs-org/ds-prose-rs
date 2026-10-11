//! Triples to a vocabulary: what each subject is, as ODRL sees it.
use super::ns::{self, odrl};
use super::triples::{Object, Parsed, Triple};
use super::{ProfileError, ProfileLimits, ProfileVocab, Term, TermKind};
use std::collections::HashMap;

/// The triples grouped by subject, subjects in first-seen order.
struct Graph<'t> {
    order: Vec<&'t str>,
    by_subject: HashMap<&'t str, Vec<&'t Triple>>,
}

impl<'t> Graph<'t> {
    fn new(triples: &'t [Triple]) -> Graph<'t> {
        let mut g = Graph {
            order: Vec::new(),
            by_subject: HashMap::new(),
        };
        for t in triples {
            let list = g.by_subject.entry(&t.s).or_insert_with(|| {
                g.order.push(&t.s);
                Vec::new()
            });
            list.push(t);
        }
        g
    }

    /// The IRI objects of `s p ?o`, in order.
    fn iris(&self, s: &str, p: &str) -> Vec<&'t str> {
        self.by_subject
            .get(s)
            .into_iter()
            .flatten()
            .filter(|t| t.p == p)
            .filter_map(|t| match &t.o {
                Object::Iri(i) => Some(i.as_str()),
                Object::Literal { .. } => None,
            })
            .collect()
    }

    fn has_type(&self, s: &str, class: &str) -> bool {
        self.iris(s, ns::RDF_TYPE).contains(&class)
    }
}

pub(crate) fn vocab(parsed: Parsed, limits: &ProfileLimits) -> Result<ProfileVocab, ProfileError> {
    let g = Graph::new(&parsed.triples);
    let mut v = ProfileVocab {
        prefixes: parsed.prefixes.clone(),
        ..ProfileVocab::default()
    };
    v.iri = profile_node(&g);
    for &s in &g.order {
        if s.starts_with("_:") {
            continue;
        }
        let kinds = kinds(&g, s);
        if kinds.is_empty() {
            continue;
        }
        let mut term = Term::new(s.to_string());
        term.kinds = kinds;
        v.index.insert(term.iri.clone(), v.terms.len());
        v.terms.push(term);
    }
    let _ = limits;
    if v.terms.is_empty() {
        return Err(ProfileError::NoTerms);
    }
    v.warnings = parsed.notes.list;
    Ok(v)
}

/// The kinds `s` has, in [`TermKind`] order.
fn kinds(g: &Graph<'_>, s: &str) -> Vec<TermKind> {
    let mut out: Vec<TermKind> = [
        ("Action", TermKind::Action),
        ("LeftOperand", TermKind::LeftOperand),
        ("Operator", TermKind::Operator),
        ("RightOperand", TermKind::RightOperand),
        ("ConflictTerm", TermKind::ConflictTerm),
    ]
    .into_iter()
    .filter(|(class, _)| g.has_type(s, &odrl(class)))
    .map(|(_, k)| k)
    .collect();
    out.sort();
    out.dedup();
    out
}

/// The profile's own node: typed `owl:Ontology`, `prof:Profile` or
/// `odrl:Profile`.
fn profile_node(g: &Graph<'_>) -> Option<String> {
    let classes = [
        format!("{}Ontology", ns::OWL),
        format!("{}Profile", ns::PROF),
        odrl("Profile"),
    ];
    g.order
        .iter()
        .find(|s| classes.iter().any(|c| g.has_type(s, c)))
        .map(|s| s.to_string())
}
