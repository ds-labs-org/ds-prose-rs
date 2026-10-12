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
        let mut seen: HashSet<&str> = HashSet::new();
        for o in self.objects(s, p) {
            if let Object::Iri(i) = o
                && seen.insert(i)
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
    let b = Builder {
        g: &g,
        limits,
        derived: derived_kinds(&triples, limits.max_depth),
    };
    let mut v = ProfileVocab {
        prefixes: prefixes.into_iter().collect(),
        ..ProfileVocab::default()
    };
    // An anonymous header still gives the title, but never the IRI.
    let header = b.profile_header();
    v.iri = header
        .clone()
        .filter(|h| named(h))
        .or_else(|| b.most_defined_by());
    if let Some(p) = header.or_else(|| v.iri.clone()) {
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
            v.imports.extend(
                g.iris(&p, &pred)
                    .into_iter()
                    .filter(|i| named(i))
                    .map(String::from),
            );
        }
    }
    for &s in &g.order {
        if !named(s) {
            continue;
        }
        let kinds = b.kinds(s, &mut notes);
        if kinds.is_empty() {
            continue;
        }
        if v.terms.len() == limits.max_terms {
            return Err(ProfileError::LimitExceeded("max_terms"));
        }
        let term = b.term(s, kinds);
        v.index.insert(term.iri.clone(), v.terms.len());
        v.terms.push(term);
    }
    if v.terms.is_empty() {
        return Err(ProfileError::NoTerms);
    }
    value_trees(&mut v.terms, limits, &mut notes);
    b.cycles(&mut notes);
    v.warnings = notes.list;
    Ok(v)
}

/// Each left operand's suggested values with their descendants: the right
/// operands whose broader links lead to them, depth first, each once.
///
/// Bounded three ways, so neither a dense or cyclic hierarchy nor many left
/// operands suggesting it costs more than the profile's size: depth stays
/// below `max_depth`, one tree holds at most `max_values_per_key` values,
/// and the whole profile adds at most `max_terms` descendants and looks at
/// most sixteen times that many links. A suggested value itself is always
/// kept (up to the per-tree cap); a tree cut short says so in a warning.
fn value_trees(terms: &mut [Term], limits: &ProfileLimits, notes: &mut Notes) {
    // Broader value -> its narrower right operands, in document order.
    let mut narrower: HashMap<&str, Vec<&str>> = HashMap::new();
    for t in terms.iter() {
        if t.kinds.contains(&TermKind::RightOperand) {
            for b in &t.broader {
                narrower.entry(b).or_default().push(&t.iri);
            }
        }
    }
    let cap = limits.max_values_per_key;
    let mut room = limits.max_terms;
    let mut looks = limits.max_terms.saturating_mul(16);
    let mut trees: Vec<(usize, Vec<(String, usize)>)> = Vec::new();
    for (i, t) in terms.iter().enumerate() {
        if t.hints.values.is_empty() {
            continue;
        }
        let mut out: Vec<(String, usize)> = Vec::new();
        let mut seen: HashSet<&str> = HashSet::new();
        let roots: Vec<&str> = t.hints.values.iter().map(String::as_str).collect();
        // (the links still to look at, their depth), innermost last.
        let mut stack: Vec<(std::slice::Iter<&str>, usize)> = vec![(roots.iter(), 0)];
        let mut cut = false;
        while let Some((links, depth)) = stack.last_mut() {
            let depth = *depth;
            let Some(&next) = links.next() else {
                stack.pop();
                continue;
            };
            if depth > 0 {
                if looks == 0 || room == 0 {
                    // Out of budget: keep the remaining suggested values only.
                    cut = true;
                    stack.truncate(1);
                    continue;
                }
                looks -= 1;
            }
            if !seen.insert(next) {
                continue;
            }
            if out.len() == cap {
                cut = true;
                break;
            }
            if depth > 0 {
                room -= 1;
            }
            out.push((next.to_string(), depth));
            if depth + 1 < limits.max_depth
                && let Some(kids) = narrower.get(next)
            {
                stack.push((kids.iter(), depth + 1));
            }
        }
        if cut {
            notes.warn(format!(
                "the suggested values of \"{}\" are cut at {} values",
                t.iri,
                out.len()
            ));
        }
        trees.push((i, out));
    }
    for (i, tree) in trees {
        terms[i].hints.value_tree = tree;
    }
}

/// Not a blank node. Blank node labels are made up by the reader (at random
/// in Turtle), so they never become a term, a profile IRI or a link.
fn named(iri: &str) -> bool {
    !iri.starts_with("_:")
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
    /// The kinds each subject has through rdfs:subPropertyOf or
    /// rdfs:subClassOf, worked out once for the whole graph.
    derived: HashMap<&'t str, Vec<TermKind>>,
}

/// The kinds subjects have through sub-properties of `odrl:function`,
/// `relation` and `operand`, and sub-classes of the ODRL policy, rule, asset
/// and party classes, transitively within `max_depth` steps. Each search
/// walks down from the ODRL terms once, so a wide hierarchy costs its size,
/// not its size squared.
fn derived_kinds(triples: &[Triple], max_depth: usize) -> HashMap<&str, Vec<TermKind>> {
    let groups: [(&str, &[&str], TermKind); 7] = [
        ("subPropertyOf", &["function"], TermKind::PartyFunction),
        ("subPropertyOf", &["relation"], TermKind::AssetRelation),
        ("subPropertyOf", &["operand"], TermKind::LogicalOperand),
        (
            "subClassOf",
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
            "subClassOf",
            &["Rule", "Permission", "Prohibition", "Duty"],
            TermKind::RuleClass,
        ),
        (
            "subClassOf",
            &["Asset", "AssetCollection"],
            TermKind::AssetClass,
        ),
        (
            "subClassOf",
            &["Party", "PartyCollection"],
            TermKind::PartyClass,
        ),
    ];
    if max_depth == 0 {
        return HashMap::new();
    }
    // Object -> subjects, per predicate.
    let mut down: HashMap<&str, HashMap<&str, Vec<&str>>> = HashMap::new();
    for t in triples {
        if let Some(local) = t.p.strip_prefix(ns::RDFS)
            && matches!(local, "subPropertyOf" | "subClassOf")
            && let Object::Iri(o) = &t.o
        {
            down.entry(local)
                .or_default()
                .entry(o)
                .or_default()
                .push(&t.s);
        }
    }
    let mut out: HashMap<&str, Vec<TermKind>> = HashMap::new();
    for (pred, names, kind) in groups {
        let Some(edges) = down.get(pred) else {
            continue;
        };
        let roots: Vec<String> = names.iter().map(|n| odrl(n)).collect();
        let mut seen: HashSet<&str> = HashSet::new();
        let mut frontier: Vec<&str> = Vec::new();
        for r in &roots {
            for &c in edges.get(r.as_str()).into_iter().flatten() {
                if seen.insert(c) {
                    frontier.push(c);
                }
            }
        }
        for _ in 1..max_depth {
            let mut next = Vec::new();
            for x in frontier {
                for &c in edges.get(x).into_iter().flatten() {
                    if seen.insert(c) {
                        next.push(c);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            frontier = next;
        }
        for s in seen {
            out.entry(s).or_default().push(kind);
        }
    }
    out
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
        let iris = |p: String| -> Vec<String> {
            g.iris(s, &p)
                .into_iter()
                .filter(|i| named(i))
                .map(String::from)
                .collect()
        };
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
        // The ds-prose extension, on left operands only.
        if t.kinds.contains(&TermKind::LeftOperand) {
            let prose = |local: &str| iris(format!("{}{local}", ns::PROSE));
            t.hints.operators = prose("operator");
            t.hints.values = prose("rightOperand");
            t.hints.datatype = prose("datatype").into_iter().next();
            t.hints.units = prose("unit");
            t.hints.closed = g
                .literals(s, &format!("{}closed", ns::PROSE))
                .iter()
                .any(|(_, v)| matches!(*v, "true" | "1"));
        }
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

        out.extend(self.derived.get(s).into_iter().flatten());

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

    /// One warning per cycle along includedIn and the broader links. An
    /// iterative depth-first search, so a long chain cannot overflow the
    /// call stack; every walk elsewhere is bounded by a visited set.
    fn cycles(&self, notes: &mut Notes) {
        let preds = [
            odrl("includedIn"),
            skos("broader"),
            rdfs("subClassOf"),
            rdfs("subPropertyOf"),
        ];
        let edges =
            |s: &str| -> Vec<&str> { preds.iter().flat_map(|p| self.g.iris(s, p)).collect() };
        // Present and `false`: on the current path. `true`: done.
        let mut done: HashMap<&str, bool> = HashMap::new();
        for &root in &self.g.order {
            if done.contains_key(root) {
                continue;
            }
            done.insert(root, false);
            let mut stack = vec![(root, edges(root), 0)];
            while let Some((node, out, i)) = stack.last_mut() {
                let Some(&next) = out.get(*i) else {
                    done.insert(node, true);
                    stack.pop();
                    continue;
                };
                *i += 1;
                match done.get(next) {
                    Some(false) => notes.warn(format!(
                        "\"{next}\" is its own ancestor through odrl:includedIn or a broader link (a cycle); the cycle is not followed"
                    )),
                    Some(true) => {}
                    None => {
                        done.insert(next, false);
                        stack.push((next, edges(next), 0));
                    }
                }
            }
        }
    }

    /// The profile's own node: the first typed `owl:Ontology`,
    /// `prof:Profile` or `odrl:Profile`, possibly a blank node.
    fn profile_header(&self) -> Option<String> {
        let g = self.g;
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

    /// The IRI named most often in `rdfs:isDefinedBy` (the first such on a
    /// tie).
    fn most_defined_by(&self) -> Option<String> {
        let g = self.g;
        let defined_by = rdfs("isDefinedBy");
        // Counted in a map, with first-seen order kept for the tie.
        let mut order: Vec<&str> = Vec::new();
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for s in &g.order {
            for o in g.iris(s, &defined_by).into_iter().filter(|o| named(o)) {
                let n = counts.entry(o).or_insert_with(|| {
                    order.push(o);
                    0
                });
                *n += 1;
            }
        }
        let max = counts.values().copied().max()?;
        order
            .into_iter()
            .find(|o| counts[o] == max)
            .map(String::from)
    }
}
