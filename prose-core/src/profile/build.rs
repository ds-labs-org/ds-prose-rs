//! Triples to a vocabulary: what each subject is, as ODRL sees it.
use super::ns::{self, odrl};
use super::text::{LangText, plain};
use super::triples::{Notes, Object, Parsed, Triple};
use super::{ProfileError, ProfileLimits, ProfileVocab, Term, TermKind};
use std::collections::{HashMap, HashSet};

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

    fn objects(&self, s: &str, p: &str) -> impl Iterator<Item = &'t Object> {
        self.by_subject
            .get(s)
            .into_iter()
            .flatten()
            .filter(move |t| t.p == p)
            .map(|t| &t.o)
    }

    /// The IRI objects of `s p ?o`, in order, each once.
    fn iris(&self, s: &str, p: &str) -> Vec<&'t str> {
        let mut out: Vec<&str> = Vec::new();
        for o in self.objects(s, p) {
            if let Object::Iri(i) = o
                && !out.contains(&i.as_str())
            {
                out.push(i);
            }
        }
        out
    }

    /// The literal objects of `s p ?o` as (language, text).
    fn literals(&self, s: &str, p: &str) -> Vec<(Option<&'t str>, &'t str)> {
        self.objects(s, p)
            .filter_map(|o| match o {
                Object::Literal { value, lang, .. } => Some((lang.as_deref(), value.as_str())),
                Object::Iri(_) => None,
            })
            .collect()
    }

    fn has_type(&self, s: &str, class: &str) -> bool {
        self.iris(s, ns::RDF_TYPE).contains(&class)
    }
}

pub(crate) fn vocab(parsed: Parsed, limits: &ProfileLimits) -> Result<ProfileVocab, ProfileError> {
    let Parsed {
        triples,
        prefixes,
        mut notes,
    } = parsed;
    let g = Graph::new(&triples);
    let b = Builder { g: &g, limits };
    let mut v = ProfileVocab {
        prefixes,
        ..ProfileVocab::default()
    };
    if let Some(p) = b.profile_node() {
        v.title = b.text(&p, &b.label_chain(), limits.max_label_chars);
        v.description = b.text(&p, &b.definition_chain(), limits.max_text_chars);
        v.version = [
            format!("{}versionInfo", ns::OWL),
            format!("{}hasVersion", ns::DCT),
        ]
        .iter()
        .find_map(|pred| g.literals(&p, pred).first().map(|(_, t)| t.to_string()))
        .map(|t| plain(&t, limits.max_label_chars));
        for pred in [
            format!("{}imports", ns::OWL),
            format!("{}isProfileOf", ns::PROF),
        ] {
            v.imports
                .extend(g.iris(&p, &pred).into_iter().map(String::from));
        }
        v.iri = Some(p);
    }
    for &s in &g.order {
        if s.starts_with("_:") {
            continue;
        }
        let kinds = b.kinds(s, &mut notes);
        if kinds.is_empty() {
            continue;
        }
        let term = b.term(s, kinds);
        v.index.insert(term.iri.clone(), v.terms.len());
        v.terms.push(term);
    }
    if v.terms.is_empty() {
        return Err(ProfileError::NoTerms);
    }
    v.warnings = notes.list;
    Ok(v)
}

fn rdfs(local: &str) -> String {
    format!("{}{local}", ns::RDFS)
}

fn skos(local: &str) -> String {
    format!("{}{local}", ns::SKOS)
}

struct Builder<'g, 't> {
    g: &'g Graph<'t>,
    limits: &'g ProfileLimits,
}

impl Builder<'_, '_> {
    fn label_chain(&self) -> [String; 3] {
        [
            rdfs("label"),
            skos("prefLabel"),
            format!("{}title", ns::DCT),
        ]
    }

    fn definition_chain(&self) -> [String; 3] {
        [
            skos("definition"),
            rdfs("comment"),
            format!("{}description", ns::DCT),
        ]
    }

    /// The literals of the first predicate in `chain` that has any, made
    /// plain text.
    fn text(&self, s: &str, chain: &[String], max: usize) -> LangText {
        self.text_where(s, chain, max, |_| true)
    }

    fn text_where(
        &self,
        s: &str,
        chain: &[String],
        max: usize,
        keep: impl Fn(&str) -> bool,
    ) -> LangText {
        let mut out = LangText::default();
        for p in chain {
            for (lang, text) in self.g.literals(s, p) {
                let text = plain(text, max);
                if !text.is_empty() && keep(&text) {
                    out.push(lang, text);
                }
            }
            if !out.is_empty() {
                break;
            }
        }
        out
    }

    fn term(&self, s: &str, kinds: Vec<TermKind>) -> Term {
        let g = self.g;
        let iris =
            |p: String| -> Vec<String> { g.iris(s, &p).into_iter().map(String::from).collect() };
        let mut t = Term::new(s.to_string());
        t.kinds = kinds;
        t.label = self.text(s, &self.label_chain(), self.limits.max_label_chars);
        t.definition = self.text(s, &self.definition_chain(), self.limits.max_text_chars);
        let note = skos("note");
        let scope = skos("scopeNote");
        let example = skos("example");
        t.note = self.text(s, std::slice::from_ref(&note), self.limits.max_text_chars);
        if t.note.is_empty() {
            t.note = self.text_where(
                s,
                std::slice::from_ref(&scope),
                self.limits.max_text_chars,
                |x| !x.eq_ignore_ascii_case("Non-Normative"),
            );
        }
        if t.note.is_empty() {
            t.note = self.text(
                s,
                std::slice::from_ref(&example),
                self.limits.max_text_chars,
            );
        }
        t.included_in = iris(odrl("includedIn"));
        for p in [skos("broader"), rdfs("subClassOf"), rdfs("subPropertyOf")] {
            for i in iris(p) {
                if !t.broader.contains(&i) {
                    t.broader.push(i);
                }
            }
        }
        t.implies = iris(odrl("implies"));
        t.exact_match = iris(skos("exactMatch"));
        t.deprecated = g
            .literals(s, &format!("{}deprecated", ns::OWL))
            .iter()
            .any(|(_, v)| matches!(*v, "true" | "1"));
        t
    }

    /// The kinds `s` has, in [`TermKind`] order.
    fn kinds(&self, s: &str, notes: &mut Notes) -> Vec<TermKind> {
        let g = self.g;
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

        let props = self.ancestors(s, &rdfs("subPropertyOf"));
        for (local, kind) in [
            ("function", TermKind::PartyFunction),
            ("relation", TermKind::AssetRelation),
            ("operand", TermKind::LogicalOperand),
        ] {
            if props.contains(&odrl(local)) {
                out.push(kind);
            }
        }
        let classes = self.ancestors(s, &rdfs("subClassOf"));
        let groups: [(&[&str], TermKind); 4] = [
            (
                &[
                    "Policy",
                    "Set",
                    "Offer",
                    "Agreement",
                    "Privacy",
                    "Request",
                    "Ticket",
                    "Assertion",
                ],
                TermKind::PolicyClass,
            ),
            (
                &["Rule", "Permission", "Prohibition", "Duty"],
                TermKind::RuleClass,
            ),
            (&["Asset", "AssetCollection"], TermKind::AssetClass),
            (&["Party", "PartyCollection"], TermKind::PartyClass),
        ];
        for (names, kind) in groups {
            if names.iter().any(|n| classes.contains(&odrl(n))) {
                out.push(kind);
            }
        }

        if out.is_empty()
            && g.iris(s, ns::RDF_TYPE).is_empty()
            && !g.iris(s, &odrl("includedIn")).is_empty()
        {
            notes.warn(format!(
                "\"{s}\" has odrl:includedIn but no type; it is read as an action"
            ));
            out.push(TermKind::Action);
        }
        out.sort();
        out.dedup();
        out
    }

    /// Every IRI reachable from `s` along `p` within the document, bounded
    /// by a visited set and `max_depth`.
    fn ancestors(&self, s: &str, p: &str) -> HashSet<String> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut frontier: Vec<&str> = vec![s];
        for _ in 0..self.limits.max_depth {
            let mut next = Vec::new();
            for x in frontier {
                for o in self.g.iris(x, p) {
                    if seen.insert(o.to_string()) {
                        next.push(o);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            frontier = next;
        }
        seen
    }

    /// The profile's own node: typed `owl:Ontology`, `prof:Profile` or
    /// `odrl:Profile`, else the IRI named most often in `rdfs:isDefinedBy`
    /// (the first such on a tie).
    fn profile_node(&self) -> Option<String> {
        let g = self.g;
        let classes = [
            format!("{}Ontology", ns::OWL),
            format!("{}Profile", ns::PROF),
            odrl("Profile"),
        ];
        if let Some(s) = g
            .order
            .iter()
            .find(|s| classes.iter().any(|c| g.has_type(s, c)))
        {
            return Some(s.to_string());
        }
        let defined_by = rdfs("isDefinedBy");
        let mut counts: Vec<(&str, usize)> = Vec::new();
        for s in &g.order {
            for o in g.iris(s, &defined_by) {
                match counts.iter_mut().find(|(i, _)| *i == o) {
                    Some((_, n)) => *n += 1,
                    None => counts.push((o, 1)),
                }
            }
        }
        let max = counts.iter().map(|(_, n)| *n).max()?;
        counts
            .into_iter()
            .find(|(_, n)| *n == max)
            .map(|(i, _)| i.to_string())
    }
}
