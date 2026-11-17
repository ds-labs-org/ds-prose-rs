//! The SHACL subset, read from the private triple list. See the module docs
//! of [`super`] for what is read.
use super::{ConstraintShape, Pattern, ShapesError, ShapesGraph};
use crate::profile::ProfileLimits;
use crate::profile::ns;
use crate::profile::triples::{Notes, Object, Parsed, Triple};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";

fn sh(local: &str) -> String {
    format!("{}{local}", ns::SH)
}

/// `iri` as a warning names it: `sh:in`, `odrl:action`, else `<iri>`.
fn short(iri: &str) -> String {
    if let Some(l) = iri.strip_prefix(ns::SH) {
        format!("sh:{l}")
    } else if let Some(l) = iri.strip_prefix(ns::ODRL) {
        format!("odrl:{l}")
    } else {
        format!("<{iri}>")
    }
}

/// The triples by subject, subjects in first-seen order.
struct Graph<'t> {
    order: Vec<&'t str>,
    by_subject: HashMap<&'t str, Vec<&'t Triple>>,
    /// The nodes some `sh:property` names: property shapes in a node shape.
    property_shapes: HashSet<&'t str>,
    /// Lists already walked, by head: a list shared by many shapes is read
    /// once.
    walked: RefCell<HashMap<&'t str, Rc<Items>>>,
}

impl<'t> Graph<'t> {
    fn new(triples: &'t [Triple]) -> Graph<'t> {
        let property = sh("property");
        let mut g = Graph {
            order: Vec::new(),
            by_subject: HashMap::new(),
            property_shapes: HashSet::new(),
            walked: RefCell::new(HashMap::new()),
        };
        for t in triples {
            if t.p == property
                && let Object::Iri(o) = &t.o
            {
                g.property_shapes.insert(o);
            }
            let list = g.by_subject.entry(&t.s).or_insert_with(|| {
                g.order.push(&t.s);
                Vec::new()
            });
            list.push(t);
        }
        g
    }

    fn objects(&self, s: &str, p: &str) -> Vec<&'t Object> {
        self.by_subject
            .get(s)
            .into_iter()
            .flatten()
            .filter(|t| t.p == p)
            .map(|t| &t.o)
            .collect()
    }

    /// The IRI (or blank node) objects of `s p ?o`, in order, each once.
    fn iris(&self, s: &str, p: &str) -> Vec<&'t str> {
        let mut out: Vec<&str> = Vec::new();
        let mut seen: HashSet<&str> = HashSet::new();
        for o in self.objects(s, p) {
            if let Object::Iri(i) = o
                && seen.insert(i.as_str())
            {
                out.push(i);
            }
        }
        out
    }

    fn literal(&self, s: &str, p: &str) -> Option<&'t str> {
        self.objects(s, p).into_iter().find_map(|o| match o {
            Object::Literal { value, .. } => Some(value.as_str()),
            Object::Iri(_) => None,
        })
    }

    /// Whether `s` is switched off with `sh:deactivated true`.
    fn deactivated(&self, s: &str) -> bool {
        self.literal(s, &sh("deactivated"))
            .is_some_and(|v| v.trim() == "true" || v.trim() == "1")
    }

    /// The `sh:` predicates `s` has, other than `known`, in first-seen order.
    fn unread(&self, s: &str, known: &[&str]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for t in self.by_subject.get(s).into_iter().flatten() {
            if let Some(local) = t.p.strip_prefix(ns::SH)
                && !known.contains(&local)
                && !out.iter().any(|o| o == local)
            {
                out.push(local.to_string());
            }
        }
        out
    }

    /// `s` and what it leads to through SHACL properties and list links.
    /// Adds to `seen`, so that a node reached from many starts is walked
    /// once.
    fn reachable_into(&self, start: &'t str, seen: &mut HashSet<&'t str>) {
        let mut stack = vec![start];
        while let Some(n) = stack.pop() {
            if !seen.insert(n) {
                continue;
            }
            for t in self.by_subject.get(n).into_iter().flatten() {
                if (t.p.starts_with(ns::SH) || t.p == RDF_FIRST || t.p == RDF_REST)
                    && let Object::Iri(o) = &t.o
                {
                    stack.push(o);
                }
            }
        }
    }
}

/// The items of every `sh:in` of a shape.
#[derive(Default)]
struct Items {
    iris: Vec<String>,
    literals: Vec<String>,
    /// More values than the reader keeps: a closed list cut short would
    /// close wrongly.
    cut: bool,
}

/// Collects distinct values in order.
#[derive(Default)]
struct Collect {
    items: Items,
    iris: HashSet<String>,
    literals: HashSet<String>,
}

impl Collect {
    fn push(&mut self, o: &Object) {
        match o {
            Object::Iri(i) if !i.starts_with("_:") => {
                if self.iris.insert(i.clone()) {
                    self.items.iris.push(i.clone());
                }
            }
            Object::Iri(_) => {}
            Object::Literal { value, .. } => {
                if self.literals.insert(value.clone()) {
                    self.items.literals.push(value.clone());
                }
            }
        }
    }
}

/// The values of the list headed `head`, walked once per graph.
fn walk<'t>(g: &Graph<'t>, head: &'t str, limits: &ProfileLimits) -> Rc<Items> {
    if let Some(hit) = g.walked.borrow().get(head) {
        return hit.clone();
    }
    let mut c = Collect::default();
    let mut node = head;
    let mut seen: HashSet<&str> = HashSet::new();
    let mut steps = 0;
    while node != RDF_NIL && seen.insert(node) {
        steps += 1;
        if steps > limits.max_terms {
            c.items.cut = true;
            break;
        }
        for first in g.objects(node, RDF_FIRST) {
            c.push(first);
        }
        match g.iris(node, RDF_REST).first() {
            Some(next) => node = next,
            None => break,
        }
    }
    let done = Rc::new(c.items);
    g.walked.borrow_mut().insert(head, done.clone());
    done
}

fn items(g: &Graph<'_>, shape: &str, limits: &ProfileLimits) -> Option<Items> {
    let objects = g.objects(shape, &sh("in"));
    if objects.is_empty() {
        return None;
    }
    let mut c = Collect::default();
    // A JSON-LD `@list` is read as several values of the property, a Turtle
    // list as a chain of nodes: the chain is walked, a bare IRI is an item.
    // Many values on one node were cut by the reader's per-key cap.
    if objects.len() > limits.max_values_per_key {
        c.items.cut = true;
    }
    for o in objects {
        let Object::Iri(head) = o else {
            c.push(o);
            continue;
        };
        if head == RDF_NIL {
            continue;
        }
        if g.by_subject
            .get(head.as_str())
            .is_some_and(|ts| ts.iter().any(|t| t.p == RDF_FIRST))
        {
            let walked = walk(g, head, limits);
            c.items.cut |= walked.cut;
            for i in &walked.iris {
                if c.iris.insert(i.clone()) {
                    c.items.iris.push(i.clone());
                }
            }
            for l in &walked.literals {
                if c.literals.insert(l.clone()) {
                    c.items.literals.push(l.clone());
                }
            }
        } else {
            c.push(o);
        }
    }
    Some(c.items)
}

/// Adds the values of `add` that `into` lacks, keeping at most `cap` in
/// all; says whether any were left out for the cap.
fn union(into: &mut Vec<String>, add: Vec<String>, cap: usize) -> bool {
    let mut have: HashSet<String> = into.iter().cloned().collect();
    let mut cut = false;
    for a in add {
        if have.contains(&a) {
            continue;
        }
        if into.len() >= cap {
            cut = true;
            break;
        }
        have.insert(a.clone());
        into.push(a);
    }
    cut
}

struct Reader<'t> {
    g: Graph<'t>,
    limits: &'t ProfileLimits,
    notes: Notes,
    out: ShapesGraph,
}

/// Predicates of every shape that carry no constraint.
const META: &[&str] = &[
    "name",
    "description",
    "message",
    "order",
    "group",
    "severity",
    "deactivated",
];

pub(crate) fn interpret(
    parsed: Parsed,
    limits: &ProfileLimits,
) -> Result<ShapesGraph, ShapesError> {
    let Parsed { triples, notes, .. } = parsed;
    let is_shacl = triples.iter().any(|t| {
        t.p.starts_with(ns::SH)
            || (t.p == ns::RDF_TYPE && matches!(&t.o, Object::Iri(i) if i.starts_with(ns::SH)))
    });
    if !is_shacl {
        return Err(ShapesError::NotShapes(
            "no SHACL found: nothing in this text is in the sh: namespace".to_string(),
        ));
    }
    let mut r = Reader {
        g: Graph::new(&triples),
        limits,
        notes,
        out: ShapesGraph {
            limits: limits.clone(),
            ..ShapesGraph::default()
        },
    };
    r.read();
    r.out.warnings = std::mem::take(&mut r.notes.list);
    Ok(r.out)
}

impl<'t> Reader<'t> {
    fn read(&mut self) {
        let skip = self.negated();
        let order: Vec<&'t str> = self.g.order.clone();
        for s in order {
            if skip.contains(s) || self.g.deactivated(s) {
                continue;
            }
            let props = self.g.iris(s, &sh("property"));
            if !props.is_empty() {
                self.node(s, &props);
            } else if !self.g.objects(s, &sh("path")).is_empty()
                && !self.g.property_shapes.contains(s)
            {
                // A property shape standing alone.
                self.property(s, None);
            }
        }
    }

    /// The shapes under `sh:not` and `sh:xone`, which say what is not
    /// allowed or that one alternative holds: reading their lists as allowed
    /// would invert or widen them.
    fn negated(&mut self) -> HashSet<&'t str> {
        let mut skip: HashSet<&'t str> = HashSet::new();
        let order: Vec<&'t str> = self.g.order.clone();
        for s in order {
            for name in ["not", "xone"] {
                for n in self.g.iris(s, &sh(name)) {
                    self.notes.warn(format!(
                        "ignored sh:{name}: the shapes under it are not read"
                    ));
                    self.g.reachable_into(n, &mut skip);
                }
            }
        }
        skip
    }

    /// A node shape with property shapes `props`.
    fn node(&mut self, s: &str, props: &[&str]) {
        let mut lefts: Vec<String> = Vec::new();
        for p in props {
            if self.path_of(p).as_deref() == Some(&format!("{}leftOperand", ns::ODRL)) {
                let has: Vec<String> = self
                    .g
                    .iris(p, &sh("hasValue"))
                    .into_iter()
                    .map(String::from)
                    .collect();
                union(&mut lefts, has, usize::MAX);
            }
        }
        let left = match lefts.as_slice() {
            [one] => Some(one.clone()),
            [] => None,
            _ => {
                self.notes.warn(format!(
                    "ignored a constraint shape that selects more than one left operand ({}): a shape is read for one left operand",
                    lefts.iter().map(|l| short(l)).collect::<Vec<_>>().join(", ")
                ));
                None
            }
        };
        for p in props {
            self.property(p, left.as_deref());
        }
        for local in self.g.unread(
            s,
            &[
                "property",
                "or",
                "and",
                "not",
                "xone",
                "targetClass",
                "targetNode",
                "targetSubjectsOf",
                "targetObjectsOf",
            ]
            .iter()
            .chain(META)
            .copied()
            .collect::<Vec<_>>(),
        ) {
            self.notes.warn(format!("ignored sh:{local} on a shape"));
        }
    }

    /// Values were left out: more than the reader keeps for one list.
    fn cut_warning(&mut self, here: &str, for_left: &str) {
        self.notes.warn(format!(
            "kept only the first {} values of {here}{for_left}: it holds more values than the reader keeps",
            self.limits.max_terms
        ));
    }

    fn path_of(&self, p: &str) -> Option<String> {
        self.g.iris(p, &sh("path")).first().map(|s| s.to_string())
    }

    fn constraint(&mut self, left: &str) -> &mut ConstraintShape {
        let i = match self.out.constraints.iter().position(|(l, _)| l == left) {
            Some(i) => i,
            None => {
                self.out
                    .constraints
                    .push((left.to_string(), ConstraintShape::default()));
                self.out.constraints.len() - 1
            }
        };
        &mut self.out.constraints[i].1
    }

    /// One property shape `p`, inside the constraint shape of `left`.
    fn property(&mut self, p: &str, left: Option<&str>) {
        if self.g.deactivated(p) {
            return;
        }
        let Some(path) = self.path_of(p) else {
            self.notes
                .warn("ignored a property shape with no sh:path that is an IRI");
            return;
        };
        let Some(local) = path.strip_prefix(ns::ODRL) else {
            self.unread_path(p, &path);
            return;
        };
        let here = format!("the {} shape", short(&path));
        let for_left = left
            .map(|l| format!(" for {}", short(l)))
            .unwrap_or_default();
        let known: &[&str] = match local {
            "rightOperand" => &[
                "path",
                "in",
                "class",
                "node",
                "datatype",
                "pattern",
                "flags",
                "minInclusive",
                "maxInclusive",
            ],
            "operator" | "unit" | "action" => &["path", "in"],
            "leftOperand" => &["path", "in", "hasValue"],
            _ => {
                self.unread_path(p, &path);
                return;
            }
        };
        let known: Vec<&str> = known.iter().chain(META).copied().collect();
        for u in self.g.unread(p, &known) {
            self.notes
                .warn(format!("ignored sh:{u} on {here}{for_left}"));
        }
        match local {
            "rightOperand" | "operator" | "unit" => {
                let Some(left) = left else {
                    self.notes.warn(format!(
                        "ignored {here}: no left operand selects it (a constraint shape needs odrl:leftOperand with sh:hasValue)"
                    ));
                    return;
                };
                match local {
                    "rightOperand" => self.right_operand(p, left, &here),
                    _ => {
                        if let Some(list) = self.closed_list(p, &format!("{here}{for_left}")) {
                            let cap = self.limits.max_terms;
                            let c = self.constraint(left);
                            let cut = union(
                                if local == "operator" {
                                    &mut c.operators
                                } else {
                                    &mut c.units
                                },
                                list,
                                cap,
                            );
                            if cut {
                                self.cut_warning(&here, &for_left);
                            }
                        }
                    }
                }
            }
            "action" => {
                if let Some(list) = self.closed_list(p, &here) {
                    let cap = self.limits.max_terms;
                    if union(self.out.actions.get_or_insert_with(Vec::new), list, cap) {
                        self.cut_warning(&here, "");
                    }
                }
            }
            _ => {
                // odrl:leftOperand: the selector, or a closed list.
                if !self.g.objects(p, &sh("hasValue")).is_empty() {
                    return;
                }
                if let Some(list) = self.closed_list(p, &here) {
                    let cap = self.limits.max_terms;
                    if union(
                        self.out.left_operands.get_or_insert_with(Vec::new),
                        list,
                        cap,
                    ) {
                        self.cut_warning(&here, "");
                    }
                } else if self.g.objects(p, &sh("in")).is_empty() {
                    self.notes.warn(format!(
                        "ignored {here}: it has neither sh:hasValue nor sh:in"
                    ));
                }
            }
        }
    }

    /// A property shape on a path that is not read, unless it only holds
    /// nested shapes (which are read on their own).
    fn unread_path(&mut self, p: &str, path: &str) {
        let nested = ["qualifiedValueShape", "node", "or", "and"];
        if nested.iter().any(|n| !self.g.objects(p, &sh(n)).is_empty()) {
            return;
        }
        self.notes.warn(format!(
            "ignored the property shape on {}: only odrl:rightOperand, odrl:operator, odrl:unit, odrl:action and odrl:leftOperand are read",
            short(path)
        ));
    }

    /// The IRIs of the `sh:in` of `p`, or `None` (with a warning when there
    /// was a list that cannot be used).
    fn closed_list(&mut self, p: &str, here: &str) -> Option<Vec<String>> {
        let items = self.list(p, here, false)?;
        Some(items.iris)
    }

    /// As [`Reader::closed_list`], with the literal members kept when
    /// `literals` (the right operand's list); otherwise a literal ignores
    /// the whole list.
    fn list(&mut self, p: &str, here: &str, literals: bool) -> Option<Items> {
        let mut items = items(&self.g, p, self.limits)?;
        if !literals && !items.literals.is_empty() {
            // ODRL terms are IRIs, and a shape may write one as a string: an
            // absolute IRI is that IRI. Anything else (a curie, a bare name)
            // needs a prefix the reader does not guess, so it ignores the list.
            if !items.literals.iter().all(|l| is_absolute_iri(l)) {
                self.notes.warn(format!(
                    "ignored the sh:in of {here}: it lists a string that is not an absolute IRI, and only IRIs are read"
                ));
                return None;
            }
            for l in std::mem::take(&mut items.literals) {
                if !items.iris.contains(&l) {
                    items.iris.push(l);
                }
            }
        }
        if items.cut {
            self.notes.warn(format!(
                "ignored the sh:in of {here}: it holds more values than the reader keeps ({}), and a cut list would close wrongly",
                self.limits.max_values_per_key
            ));
            return None;
        }
        if items.iris.is_empty() && items.literals.is_empty() {
            self.notes
                .warn(format!("ignored the empty sh:in of {here}"));
            return None;
        }
        Some(items)
    }

    fn right_operand(&mut self, p: &str, left: &str, here: &str) {
        let list = self.list(p, here, true);
        let mut classes: Vec<String> = Vec::new();
        for name in ["class", "node"] {
            let found: Vec<String> = self
                .g
                .iris(p, &sh(name))
                .into_iter()
                .map(String::from)
                .collect();
            union(&mut classes, found, usize::MAX);
        }
        if list.is_some() && !classes.is_empty() {
            self.notes.warn(format!(
                "ignored sh:class beside sh:in on {here} for {}: the list says what is allowed",
                short(left)
            ));
            classes.clear();
        }
        let datatype = self
            .g
            .iris(p, &sh("datatype"))
            .first()
            .map(|s| s.to_string());
        let pattern = self.g.literal(p, &sh("pattern")).map(str::to_string);
        let flags = self.g.literal(p, &sh("flags")).unwrap_or("").to_string();
        let pattern = pattern.and_then(|src| match Pattern::new(&src, &flags) {
            Ok(_) => Some((src, flags)),
            Err(why) => {
                self.notes.warn(format!(
                    "ignored the sh:pattern \"{src}\" on {here} for {}: {why}",
                    short(left)
                ));
                None
            }
        });
        let min = self.g.literal(p, &sh("minInclusive")).map(str::to_string);
        let max = self.g.literal(p, &sh("maxInclusive")).map(str::to_string);
        let mut conflicts: Vec<&str> = Vec::new();
        let cap = self.limits.max_terms;
        let mut cut = false;
        if list.is_none()
            && classes.is_empty()
            && datatype.is_none()
            && pattern.is_none()
            && min.is_none()
            && max.is_none()
        {
            // Nothing was read (an ignored or empty list): no constraint.
            return;
        }
        let c = self.constraint(left);
        if let Some(list) = list {
            cut |= union(&mut c.values, list.iris, cap);
            cut |= union(&mut c.literals, list.literals, cap);
        }
        union(&mut c.classes, classes, usize::MAX);
        // Alternatives that disagree on a single value cannot both be read:
        // the first stands.
        for (name, have, new) in [
            ("sh:datatype", &mut c.datatype, datatype),
            ("sh:minInclusive", &mut c.min_inclusive, min),
            ("sh:maxInclusive", &mut c.max_inclusive, max),
        ] {
            match (&*have, new) {
                (None, new) => *have = new,
                (Some(a), Some(b)) if *a != b => conflicts.push(name),
                _ => {}
            }
        }
        if let Some(new) = pattern {
            match &c.pattern {
                None => c.pattern = Some(new),
                Some(a) if *a != new => conflicts.push("sh:pattern"),
                _ => {}
            }
        }
        if cut {
            let for_left = format!(" for {}", short(left));
            self.cut_warning(here, &for_left);
        }
        for name in conflicts {
            self.notes.warn(format!(
                "ignored a second {name} for {}: the first is read",
                short(left)
            ));
        }
    }
}

/// Whether a string is written as an absolute IRI: `scheme://rest`, or a
/// `urn:` / `did:` name. `odrl:eq` is a curie, not an IRI.
fn is_absolute_iri(s: &str) -> bool {
    let Some((scheme, rest)) = s.split_once(':') else {
        return false;
    };
    let scheme_ok = scheme
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c));
    scheme_ok
        && !s.chars().any(|c| c.is_whitespace())
        && (rest.starts_with("//") && rest.len() > 2
            || (["urn", "did"].contains(&scheme.to_ascii_lowercase().as_str()) && !rest.is_empty()))
}
