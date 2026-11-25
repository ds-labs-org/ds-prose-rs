//! Turtle to ODRL JSON-LD for the ds-odrl-compliance-rdf corpus.
//!
//! A case file holds a whole test: a `dsc:Request` with its policies, claims,
//! profile and expected FORCE report. Only the policies are read here: those
//! the request names with `dsc:policy`, then any ancestor they reach through
//! `dsc:inheritsFrom` that the request does not name, in file order. Each
//! becomes one compact JSON-LD node, written the way an ODRL author writes it
//! against `http://www.w3.org/ns/odrl.jsonld`: ODRL terms by their bare
//! names, the case's own `#fragment`s as `case:fragment`, other namespaces by
//! their usual prefix, everything else as a full IRI.
//!
//! The `dsc:` extension is mapped where ODRL has an equivalent and kept as an
//! extension key where it has none:
//!
//! - `dsc:inheritsFrom ( ... )` is ODRL's `inheritFrom`.
//! - `dsc:policyKind "X"` on an `odrl:Policy` is kept as `dsc:policyKind`.
//! - A `dsc:ClaimKey` left operand is a blank node, which ODRL does not
//!   allow and the renderer cannot name ("an incomplete constraint"). It is
//!   given an IRI in the case's namespace, `case:claim-<key>`, and keeps its
//!   type and key: `{"@id": "case:claim-riskScore", "@type": "dsc:ClaimKey",
//!   "dsc:key": "riskScore"}` reads as "the claim risk score".
use oxrdf::{NamedOrBlankNode, Term, Triple};
use oxttl::TurtleParser;
use serde_json::{Map, Value, json};

pub const ODRL: &str = "http://www.w3.org/ns/odrl/2/";
pub const DSC: &str = "https://ds-labs-org.github.io/ds-odrl-compliance-rdf/ns#";
const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";
const DCT: &str = "http://purl.org/dc/terms/";

/// Prefixes written into every context, besides `case:`.
const PREFIXES: &[(&str, &str)] = &[
    ("dsc", DSC),
    ("dpv", "https://w3id.org/dpv#"),
    ("foaf", "http://xmlns.com/foaf/0.1/"),
    ("vcard", "http://www.w3.org/2006/vcard/ns#"),
];

pub struct Case {
    pub slug: String,
    pub title: String,
    /// The compact JSON-LD document: `@context` and `@graph` of policies.
    pub policies: Value,
}

struct Graph {
    triples: Vec<Triple>,
    /// The case file's own base, with `#`: its resources are fragments of it.
    base: String,
}

fn subject_is(t: &Triple, s: &Term) -> bool {
    match (&t.subject, s) {
        (NamedOrBlankNode::NamedNode(a), Term::NamedNode(b)) => a == b,
        (NamedOrBlankNode::BlankNode(a), Term::BlankNode(b)) => a == b,
        _ => false,
    }
}

impl Graph {
    fn objects(&self, s: &Term, p: &str) -> Vec<Term> {
        self.triples
            .iter()
            .filter(|t| subject_is(t, s) && t.predicate.as_str() == p)
            .map(|t| t.object.clone())
            .collect()
    }

    fn object(&self, s: &Term, p: &str) -> Option<Term> {
        self.objects(s, p).into_iter().next()
    }

    fn types(&self, s: &Term) -> Vec<String> {
        self.objects(s, &format!("{RDF}type"))
            .into_iter()
            .filter_map(|t| match t {
                Term::NamedNode(n) => Some(n.into_string()),
                _ => None,
            })
            .collect()
    }

    fn list(&self, head: &Term) -> Vec<Term> {
        let mut out = Vec::new();
        let mut at = head.clone();
        while !matches!(&at, Term::NamedNode(n) if n.as_str() == format!("{RDF}nil")) {
            let Some(first) = self.object(&at, &format!("{RDF}first")) else {
                break;
            };
            out.push(first);
            match self.object(&at, &format!("{RDF}rest")) {
                Some(rest) => at = rest,
                None => break,
            }
        }
        out
    }

    fn subjects_of_type(&self, class: &str) -> Vec<Term> {
        let mut out: Vec<Term> = Vec::new();
        for t in &self.triples {
            if t.predicate.as_str() == format!("{RDF}type")
                && matches!(&t.object, Term::NamedNode(n) if n.as_str() == class)
            {
                let s: Term = t.subject.clone().into();
                if !out.contains(&s) {
                    out.push(s);
                }
            }
        }
        out
    }

    /// An IRI as an author writes it: an ODRL term bare, the case's own
    /// fragment as `case:`, a known namespace by its prefix.
    fn compact(&self, iri: &str) -> String {
        if let Some(local) = iri.strip_prefix(ODRL) {
            return local.to_string();
        }
        if let Some(local) = iri.strip_prefix(&self.base) {
            return format!("case:{local}");
        }
        for (p, ns) in PREFIXES {
            if let Some(local) = iri.strip_prefix(ns) {
                return format!("{p}:{local}");
            }
        }
        iri.to_string()
    }

    /// A resource-valued property: a plain string, which the ODRL context
    /// types as `@id` for every property this translator writes one into.
    fn iri(&self, t: &Term) -> Value {
        match t {
            Term::NamedNode(n) => json!(self.compact(n.as_str())),
            other => json!(other.to_string()),
        }
    }

    fn value(&self, t: &Term) -> Value {
        match t {
            Term::NamedNode(n) => json!({ "@id": self.compact(n.as_str()) }),
            Term::Literal(l) => {
                let dt = l.datatype().as_str();
                let v = l.value();
                match dt.strip_prefix(XSD) {
                    Some("integer") => v.parse::<i64>().map(Value::from).unwrap_or(json!(v)),
                    Some("decimal" | "double") => {
                        v.parse::<f64>().map(Value::from).unwrap_or(json!(v))
                    }
                    Some("boolean") => json!(v == "true"),
                    Some("string") => json!(v),
                    _ if l.language().is_some() => {
                        json!({ "@value": v, "@language": l.language().unwrap() })
                    }
                    Some(x) => json!({ "@value": v, "@type": format!("xsd:{x}") }),
                    None => json!({ "@value": v, "@type": dt }),
                }
            }
            other => json!(other.to_string()),
        }
    }

    fn one_or_many(values: Vec<Value>) -> Option<Value> {
        match values.len() {
            0 => None,
            1 => values.into_iter().next(),
            _ => Some(Value::Array(values)),
        }
    }

    fn constraint(&self, c: &Term) -> Value {
        let mut m = Map::new();
        for op in ["and", "or", "xone", "andSequence"] {
            if let Some(head) = self.object(c, &format!("{ODRL}{op}")) {
                let items = self
                    .list(&head)
                    .iter()
                    .map(|i| self.constraint(i))
                    .collect();
                m.insert(op.into(), Value::Array(items));
                return Value::Object(m);
            }
        }
        if let Some(l) = self.object(c, &format!("{ODRL}leftOperand")) {
            let left = match &l {
                Term::NamedNode(n) => json!(self.compact(n.as_str())),
                _ => {
                    let mut node = Map::new();
                    if let Some(Term::Literal(k)) = self.object(&l, &format!("{DSC}key")) {
                        node.insert("@id".into(), json!(format!("case:claim-{}", k.value())));
                    }
                    if self
                        .types(&l)
                        .iter()
                        .any(|t| t == &format!("{DSC}ClaimKey"))
                    {
                        node.insert("@type".into(), json!("dsc:ClaimKey"));
                    }
                    if let Some(k) = self.object(&l, &format!("{DSC}key")) {
                        node.insert("dsc:key".into(), self.value(&k));
                    }
                    Value::Object(node)
                }
            };
            m.insert("leftOperand".into(), left);
        }
        if let Some(Term::NamedNode(o)) = self.object(c, &format!("{ODRL}operator")) {
            m.insert("operator".into(), json!(self.compact(o.as_str())));
        }
        let rights: Vec<Value> = self
            .objects(c, &format!("{ODRL}rightOperand"))
            .iter()
            .map(|r| self.value(r))
            .collect();
        if let Some(r) = Self::one_or_many(rights) {
            m.insert("rightOperand".into(), r);
        }
        if let Some(Term::NamedNode(u)) = self.object(c, &format!("{ODRL}unit")) {
            m.insert("unit".into(), json!(self.compact(u.as_str())));
        }
        Value::Object(m)
    }

    fn constraints(&self, s: &Term, p: &str) -> Option<Value> {
        let cs: Vec<Value> = self
            .objects(s, &format!("{ODRL}{p}"))
            .iter()
            .map(|c| self.constraint(c))
            .collect();
        (!cs.is_empty()).then_some(Value::Array(cs))
    }

    fn action(&self, a: &Term) -> Value {
        match a {
            Term::NamedNode(n) => json!(self.compact(n.as_str())),
            _ => {
                let mut m = Map::new();
                if let Some(Term::NamedNode(v)) = self.object(a, &format!("{RDF}value")) {
                    let name = self.compact(v.as_str());
                    let id = if name.contains(':') {
                        name
                    } else {
                        format!("odrl:{name}")
                    };
                    m.insert("rdf:value".into(), json!({ "@id": id }));
                }
                if let Some(r) = self.constraints(a, "refinement") {
                    m.insert("refinement".into(), r);
                }
                Value::Object(m)
            }
        }
    }

    fn rule(&self, r: &Term) -> Value {
        let mut m = Map::new();
        if let Term::NamedNode(n) = r {
            m.insert("uid".into(), json!(self.compact(n.as_str())));
        }
        for p in ["assigner", "assignee"] {
            if let Some(v) = self.object(r, &format!("{ODRL}{p}")) {
                m.insert(p.into(), self.iri(&v));
            }
        }
        let targets: Vec<Value> = self
            .objects(r, &format!("{ODRL}target"))
            .iter()
            .map(|t| self.iri(t))
            .collect();
        if let Some(t) = Self::one_or_many(targets) {
            m.insert("target".into(), t);
        }
        if let Some(a) = self.object(r, &format!("{ODRL}action")) {
            m.insert("action".into(), self.action(&a));
        }
        if let Some(c) = self.constraints(r, "constraint") {
            m.insert("constraint".into(), c);
        }
        for p in ["duty", "remedy", "consequence"] {
            let rules: Vec<Value> = self
                .objects(r, &format!("{ODRL}{p}"))
                .iter()
                .map(|d| self.rule(d))
                .collect();
            if !rules.is_empty() {
                m.insert(p.into(), Value::Array(rules));
            }
        }
        Value::Object(m)
    }

    fn policy(&self, p: &Term) -> Value {
        let mut m = Map::new();
        let kind = self
            .types(p)
            .iter()
            .find_map(|t| t.strip_prefix(ODRL).map(str::to_string))
            .unwrap_or_else(|| "Policy".into());
        m.insert("@type".into(), json!(kind));
        if let Term::NamedNode(n) = p {
            m.insert("uid".into(), json!(self.compact(n.as_str())));
        }
        if let Some(k) = self.object(p, &format!("{DSC}policyKind")) {
            m.insert("dsc:policyKind".into(), self.value(&k));
        }
        if let Some(head) = self.object(p, &format!("{DSC}inheritsFrom")) {
            let parents: Vec<Value> = self.list(&head).iter().map(|x| self.iri(x)).collect();
            if let Some(v) = Self::one_or_many(parents) {
                m.insert("inheritFrom".into(), v);
            }
        }
        if let Some(Term::NamedNode(c)) = self.object(p, &format!("{ODRL}conflict")) {
            m.insert("conflict".into(), json!(self.compact(c.as_str())));
        }
        for party in ["assigner", "assignee"] {
            if let Some(v) = self.object(p, &format!("{ODRL}{party}")) {
                m.insert(party.into(), self.iri(&v));
            }
        }
        for kind in ["permission", "prohibition", "obligation"] {
            let rules: Vec<Value> = self
                .objects(p, &format!("{ODRL}{kind}"))
                .iter()
                .map(|r| self.rule(r))
                .collect();
            if !rules.is_empty() {
                m.insert(kind.into(), Value::Array(rules));
            }
        }
        Value::Object(m)
    }

    /// The request's policies, then ancestors it does not name.
    fn policies(&self) -> Vec<Term> {
        let mut out: Vec<Term> = Vec::new();
        for req in self.subjects_of_type(&format!("{DSC}Request")) {
            for p in self.objects(&req, &format!("{DSC}policy")) {
                if !out.contains(&p) {
                    out.push(p);
                }
            }
        }
        let mut i = 0;
        while i < out.len() {
            if let Some(head) = self.object(&out[i].clone(), &format!("{DSC}inheritsFrom")) {
                for parent in self.list(&head) {
                    if !out.contains(&parent) {
                        out.push(parent);
                    }
                }
            }
            i += 1;
        }
        out
    }
}

/// Reads one case file. `slug` is its file name without `.ttl`.
pub fn translate(slug: &str, ttl: &str) -> Result<Case, String> {
    let base = format!("https://ds-labs-org.github.io/ds-odrl-compliance-rdf/cases/{slug}");
    let parser = TurtleParser::new()
        .with_base_iri(&base)
        .map_err(|e| e.to_string())?;
    let triples = parser
        .for_slice(ttl.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{slug}: {e}"))?;
    let g = Graph {
        triples,
        base: format!("{base}#"),
    };
    let title = g
        .subjects_of_type(&format!("{DSC}TestCase"))
        .first()
        .and_then(|t| g.object(t, &format!("{DCT}title")))
        .map(|t| match t {
            Term::Literal(l) => l.value().to_string(),
            other => other.to_string(),
        })
        .ok_or_else(|| format!("{slug}: no dsc:TestCase with a dct:title"))?;
    let policies: Vec<Value> = g.policies().iter().map(|p| g.policy(p)).collect();
    if policies.is_empty() {
        return Err(format!("{slug}: the request names no policy"));
    }
    let mut ctx = Map::new();
    ctx.insert("case".into(), json!(g.base));
    for (p, ns) in PREFIXES {
        ctx.insert((*p).into(), json!(ns));
    }
    Ok(Case {
        slug: slug.to_string(),
        title,
        policies: json!({
            "@context": ["http://www.w3.org/ns/odrl.jsonld", Value::Object(ctx)],
            "@graph": policies,
        }),
    })
}
