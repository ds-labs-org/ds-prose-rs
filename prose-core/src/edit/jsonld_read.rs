//! JSON-LD in, [`EditDoc`] out. Reads the same shapes `render` does, but
//! keeps every node addressable and remembers the JSON it came from.
use serde_json::{Map, Value};

use super::model::*;
use crate::ProseError;
use crate::profile::Prefixes;
use crate::read::{
    MAX_DEPTH, ODRL_NS, POLICY_KEYS, QUIET_PREFIXES, RULE_KEYS, bare_reference, get, ident, items,
    lineages, looks_like_policy, metadata_keys, metadata_note, policy_nodes, types,
};
use crate::words::local_name;

type Obj = Map<String, Value>;

pub(crate) const LOGICAL_KEYS: [&str; 4] = ["and", "or", "xone", "andSequence"];

/// Parse `json` and read it into the editable model.
pub fn read_model(json: &str) -> Result<EditDoc, ProseError> {
    let value: Value = serde_json::from_str(json).map_err(|e| ProseError::Json(e.to_string()))?;
    read_model_value(&value)
}

/// As [`read_model`], for a value that is already parsed. It reads bare
/// property keys only: `render_value` also reads ODRL properties written as
/// `odrl:permission` or as full IRIs, so a document written that way renders
/// there but reads here as no rules (or fails with `NoPolicy` when nothing
/// else marks it a policy), because the writer keeps key spelling.
pub fn read_model_value(value: &Value) -> Result<EditDoc, ProseError> {
    let (shape, origin) = match value {
        Value::Array(_) => (DocShape::Array, Some(Origin::new(value.clone()))),
        Value::Object(o) if o.contains_key("@graph") => {
            (DocShape::Graph, Some(Origin::new(value.clone())))
        }
        _ => (DocShape::Single, None),
    };
    let mut r = Reader::default();
    let mut policies = Vec::new();
    let mut real = 0;
    for v in policy_nodes(value) {
        match v {
            Value::Object(o) if looks_like_policy(o) => {
                real += 1;
                policies.push(r.policy(v));
            }
            _ => {
                r.warn("skipped a node that is not an ODRL policy");
                policies.push(PolicyNode {
                    origin: Some(Origin::new(v.clone())),
                    locked: Some("not an ODRL policy".into()),
                    ..PolicyNode::default()
                });
            }
        }
    }
    if real == 0 {
        return Err(ProseError::NoPolicy);
    }
    inheritance_warnings(&mut r, &policies, value);
    let mut doc = EditDoc::new(policies);
    doc.shape = shape;
    doc.origin = origin;
    doc.warnings = r.warnings;
    // A metadata key the reader read is not unread, however it is spelled.
    doc.unread = unread_odrl_keys(value)
        .into_iter()
        .filter(|k| !r.metadata_read.contains(k))
        .collect();
    Ok(doc)
}

const NO_RULES: &str = "a policy has no permission, prohibition or obligation";

fn has_rules(p: &PolicyNode) -> bool {
    !(p.permission.is_empty() && p.prohibition.is_empty() && p.obligation.is_empty())
}

/// Warns, as `render` does, when a policy without rules of its own gets
/// none by inheriting (ODRL IM 2.9) either: it inherits only from itself or
/// from policies in the document that have no rules. Parents that are not
/// in the document may have rules, so they are no reason to warn.
fn inheritance_warnings(r: &mut Reader, policies: &[PolicyNode], value: &Value) {
    let iris = Prefixes::of_document(value);
    let uids: Vec<Option<String>> = policies.iter().map(|p| p.uid.clone()).collect();
    let parents: Vec<Vec<String>> = policies.iter().map(|p| p.inherit_from.clone()).collect();
    let (lineages, _) = lineages(&uids, &parents, |id| match id.contains(':') {
        true => iris.expand(id),
        false => id.to_string(),
    });
    for (p, lineage) in policies.iter().zip(lineages) {
        if !has_rules(p)
            && !p.inherit_from.is_empty()
            && lineage.missing.is_empty()
            && !lineage.cut
            && !lineage.ancestors.iter().any(|&a| has_rules(&policies[a]))
        {
            r.warn(NO_RULES);
        }
    }
}

#[derive(Default)]
pub(crate) struct Reader {
    pub warnings: Vec<String>,
    /// The same messages, for de-duplication in constant time.
    seen: std::collections::HashSet<String>,
    /// The keys read as a policy's metadata.
    metadata_read: std::collections::HashSet<String>,
}

fn locked_origin(v: &Value) -> Option<Origin> {
    Some(Origin::new(v.clone()))
}

impl Reader {
    fn warn(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if self.seen.insert(msg.clone()) {
            self.warnings.push(msg);
        }
    }

    fn unknown_keys(&mut self, o: &Obj, known: &[&str], what: &str) {
        self.unknown_keys_but(o, known, &[], what);
    }

    /// As `unknown_keys`, with `also` known too.
    fn unknown_keys_but(&mut self, o: &Obj, known: &[&str], also: &[&String], what: &str) {
        for k in o.keys() {
            if known.contains(&k.as_str())
                || also.contains(&k)
                || QUIET_PREFIXES.iter().any(|p| k.starts_with(p))
            {
                continue;
            }
            self.warn(format!("ignored unknown property \"{k}\" on {what}"));
        }
    }

    pub(crate) fn policy(&mut self, v: &Value) -> PolicyNode {
        let Value::Object(o) = v else {
            return PolicyNode {
                origin: locked_origin(v),
                locked: Some("not an ODRL policy".into()),
                ..PolicyNode::default()
            };
        };
        let keys = metadata_keys(o);
        let read: Vec<&String> = keys.iter().map(|(_, k)| *k).collect();
        self.unknown_keys_but(o, POLICY_KEYS, &read, "a policy");
        let mut metadata = Vec::new();
        for (term, key) in keys {
            self.metadata_read.insert(key.clone());
            for v in items(&o[key]) {
                // Agreement or not, a value reads or it does not.
                match metadata_note(term, false, v) {
                    Some(_) => metadata.push((term.to_string(), v.clone())),
                    None => self.warn(format!("skipped an unreadable {term}")),
                }
            }
        }
        let ts = types(o);
        let kind = ts
            .iter()
            .find(|t| local_name(t) != "Policy")
            .or(ts.first())
            .map(|s| s.to_string())
            .unwrap_or_default();
        let p = PolicyNode {
            id: NodeId(0),
            origin: locked_origin(v),
            locked: None,
            kind,
            uid: get(o, &["uid", "@id", "id"]).and_then(ident),
            assigner: self.entities(o.get("assigner"), "party"),
            assignee: self.entities(o.get("assignee"), "party"),
            target: self.entities(o.get("target"), "asset"),
            action: self.actions(o.get("action")),
            profile: strings(o.get("profile")),
            conflict: o.get("conflict").and_then(ident),
            inherit_from: strings(o.get("inheritFrom")),
            metadata,
            permission: self.rules(o.get("permission")),
            prohibition: self.rules(o.get("prohibition")),
            obligation: self.rules(o.get("obligation")),
        };
        // One that inherits is looked at once the whole document is read.
        if !has_rules(&p) && p.inherit_from.is_empty() {
            self.warn(NO_RULES);
        }
        p
    }

    fn rules(&mut self, v: Option<&Value>) -> Vec<RuleNode> {
        v.map(|v| items(v).into_iter().map(|r| self.rule(r)).collect())
            .unwrap_or_default()
    }

    pub(crate) fn rule(&mut self, v: &Value) -> RuleNode {
        match v {
            Value::String(s) => RuleNode {
                origin: locked_origin(v),
                reference: Some(s.clone()),
                ..RuleNode::default()
            },
            Value::Object(o) if bare_reference(o).is_some() => RuleNode {
                origin: locked_origin(v),
                reference: bare_reference(o),
                ..RuleNode::default()
            },
            Value::Object(o) => {
                self.unknown_keys(o, RULE_KEYS, "a rule");
                RuleNode {
                    id: NodeId(0),
                    origin: locked_origin(v),
                    locked: None,
                    reference: None,
                    uid: get(o, &["uid", "@id", "id"]).and_then(ident),
                    action: self.actions(o.get("action")),
                    target: self.entities(o.get("target"), "asset"),
                    assigner: self.entities(o.get("assigner"), "party"),
                    assignee: self.entities(o.get("assignee"), "party"),
                    constraint: self.constraints(o.get("constraint")),
                    duty: self.rules(o.get("duty")),
                    remedy: self.rules(o.get("remedy")),
                    consequence: self.rules(o.get("consequence")),
                }
            }
            _ => RuleNode {
                origin: locked_origin(v),
                locked: Some("not a rule".into()),
                ..RuleNode::default()
            },
        }
    }

    fn entities(&mut self, v: Option<&Value>, noun: &str) -> Vec<Entity> {
        v.map(|v| items(v).into_iter().map(|e| self.entity(e, noun)).collect())
            .unwrap_or_default()
    }

    pub(crate) fn entity(&mut self, v: &Value, noun: &str) -> Entity {
        match v {
            Value::String(s) => Entity {
                origin: locked_origin(v),
                iri: Some(s.clone()),
                ..Entity::default()
            },
            Value::Object(o) => {
                let iri = get(o, &["uid", "@id", "id"])
                    .and_then(Value::as_str)
                    .map(String::from);
                let part_of = o.get("source").or_else(|| o.get("partOf")).and_then(ident);
                if iri.is_none() && part_of.is_none() {
                    self.warn(format!("a {noun} has no identifier"));
                    return Entity {
                        origin: locked_origin(v),
                        locked: Some("has no identifier".into()),
                        ..Entity::default()
                    };
                }
                Entity {
                    id: NodeId(0),
                    origin: locked_origin(v),
                    locked: None,
                    iri,
                    part_of,
                    refinement: self.constraints(o.get("refinement")),
                }
            }
            _ => {
                self.warn(format!("skipped an unreadable {noun}"));
                Entity {
                    origin: locked_origin(v),
                    locked: Some(format!("not a {noun}")),
                    ..Entity::default()
                }
            }
        }
    }

    fn actions(&mut self, v: Option<&Value>) -> Vec<ActionNode> {
        v.map(|v| items(v).into_iter().map(|a| self.action(a)).collect())
            .unwrap_or_default()
    }

    pub(crate) fn action(&mut self, v: &Value) -> ActionNode {
        match v {
            Value::String(s) => ActionNode {
                origin: locked_origin(v),
                name: s.clone(),
                ..ActionNode::default()
            },
            Value::Object(o) => match get(o, &["rdf:value", "value", "@id"]).and_then(ident) {
                Some(name) => ActionNode {
                    id: NodeId(0),
                    origin: locked_origin(v),
                    locked: None,
                    name,
                    refinement: self.constraints(o.get("refinement")),
                },
                None => {
                    self.warn("skipped an unreadable action");
                    ActionNode {
                        origin: locked_origin(v),
                        locked: Some("has no name".into()),
                        ..ActionNode::default()
                    }
                }
            },
            _ => {
                self.warn("skipped an unreadable action");
                ActionNode {
                    origin: locked_origin(v),
                    locked: Some("not an action".into()),
                    ..ActionNode::default()
                }
            }
        }
    }

    fn constraints(&mut self, v: Option<&Value>) -> Vec<ConstraintNode> {
        v.map(|v| items(v).into_iter().map(|c| self.constraint(c)).collect())
            .unwrap_or_default()
    }

    pub(crate) fn constraint(&mut self, v: &Value) -> ConstraintNode {
        let opaque = |reason: &str| ConstraintNode::Opaque {
            id: NodeId(0),
            origin: Origin::new(v.clone()),
            reason: reason.to_string(),
        };
        match v {
            Value::String(s) => ConstraintNode::Reference {
                id: NodeId(0),
                origin: locked_origin(v),
                iri: s.clone(),
            },
            Value::Object(o) => {
                let present: Vec<&str> = LOGICAL_KEYS
                    .into_iter()
                    .filter(|k| o.contains_key(*k))
                    .collect();
                match present.as_slice() {
                    [] => {}
                    [key] => {
                        let op = LogicalOp::parse(key).unwrap_or_default();
                        return ConstraintNode::Logical(LogicalConstraint {
                            id: NodeId(0),
                            origin: locked_origin(v),
                            op,
                            children: self.constraints(o.get(*key)),
                        });
                    }
                    _ => {
                        self.warn("a constraint has several logical operators");
                        return opaque("several logical operators");
                    }
                }
                let left = get(o, &["leftOperand"]).and_then(ident);
                let operator = get(o, &["operator"]).and_then(ident);
                if left.is_none() || operator.is_none() {
                    self.warn("a constraint is missing its leftOperand or operator");
                }
                let right = if let Some(r) = o.get("rightOperand") {
                    let mut vals = Vec::new();
                    for x in items(r) {
                        match literal(x) {
                            Some(l) => vals.push(l),
                            None => {
                                self.warn("skipped an unreadable right operand");
                                return opaque("unreadable right operand");
                            }
                        }
                    }
                    RightOperand::Values(vals)
                } else if let Some(r) = o.get("rightOperandReference").and_then(ident) {
                    RightOperand::Reference(r)
                } else {
                    self.warn("a constraint is missing its rightOperand");
                    RightOperand::Missing
                };
                ConstraintNode::Atomic(AtomicConstraint {
                    id: NodeId(0),
                    origin: locked_origin(v),
                    left: left.unwrap_or_default(),
                    operator: operator.unwrap_or_default(),
                    right,
                    unit: o.get("unit").and_then(ident),
                })
            }
            _ => {
                self.warn("skipped an unreadable constraint");
                opaque("not a constraint")
            }
        }
    }
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.map(|v| items(v).into_iter().filter_map(ident).collect())
        .unwrap_or_default()
}

pub(crate) fn literal(v: &Value) -> Option<Literal> {
    match v {
        Value::String(s) => Some(Literal::Str(s.clone())),
        Value::Number(n) => Some(Literal::Num(n.clone())),
        Value::Bool(b) => Some(Literal::Bool(*b)),
        Value::Object(o) => {
            if let Some(inner) = o.get("@value") {
                return Some(match (inner, o.get("@type").and_then(Value::as_str)) {
                    (Value::String(s), Some(t)) => Literal::Typed {
                        value: s.clone(),
                        datatype: t.to_string(),
                    },
                    (Value::String(s), None) => Literal::Str(s.clone()),
                    (Value::Number(n), _) => Literal::Num(n.clone()),
                    (Value::Bool(b), _) => Literal::Bool(*b),
                    (other, _) => Literal::Str(other.to_string()),
                });
            }
            ident(v).map(Literal::Iri)
        }
        _ => None,
    }
}

/// The property keys in `v` that are ODRL's but are not bare, and so were
/// not read: a compact or full IRI that expands into the ODRL namespace, or
/// a term an @context defines as an ODRL IRI other than its own name.
/// Expansion follows what the document's @contexts define, as JSON-LD does:
/// a prefix defined through another prefix (`"o": "odrl:"`), a term defined
/// through another term, and a term relative to the `@vocab`. It
/// over-reports rather than guesses: every definition of a name counts,
/// whatever its scope, `odrl:` is always the ODRL namespace (a rebound
/// `odrl` prefix is not tracked), and an expansion that runs past
/// [`MAX_EXPANSION`] steps counts as ODRL's. Skips `@context`, `@value`
/// and the value of a term typed `@json` (JSON data, not properties);
/// iterative and depth-bounded, so a hostile value cannot overflow the
/// stack.
fn unread_odrl_keys(v: &Value) -> Vec<String> {
    let ctx = Defs::of(v);
    let mut out: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![(v, 0usize)];
    while let Some((x, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            continue;
        }
        match x {
            Value::Array(a) => stack.extend(a.iter().rev().map(|y| (y, depth + 1))),
            Value::Object(o) => {
                for (k, y) in o.iter().rev() {
                    if k == "@context" || k == "@value" {
                        continue;
                    }
                    if !seen.contains(k) && ctx.odrl_key(k) {
                        seen.insert(k.clone());
                        out.push(k.clone());
                    }
                    if !ctx.json_typed(k) {
                        stack.push((y, depth + 1));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// How many definitions one key's expansion may follow before it stops and
/// counts the key as ODRL's.
const MAX_EXPANSION: usize = 64;

/// Every term definition and `@vocab` the @contexts of a document hold,
/// whatever their scope.
#[derive(Default)]
struct Defs {
    terms: std::collections::HashMap<String, Vec<String>>,
    vocab: Vec<String>,
    json: std::collections::HashSet<String>,
    plain: std::collections::HashSet<String>,
}

impl Defs {
    fn of(v: &Value) -> Defs {
        let mut d = Defs::default();
        let mut stack = vec![(v, 0usize, false)];
        while let Some((x, depth, in_ctx)) = stack.pop() {
            if depth > MAX_DEPTH {
                continue;
            }
            match x {
                Value::Array(a) => stack.extend(a.iter().map(|y| (y, depth + 1, in_ctx))),
                Value::Object(o) => {
                    for (k, y) in o {
                        if k == "@context" {
                            stack.push((y, depth + 1, true));
                        } else if in_ctx && k == "@vocab" {
                            // An empty @vocab (the base) never reaches ODRL's
                            // namespace from a term.
                            if let Value::String(s) = y
                                && !s.is_empty()
                                && !d.vocab.contains(s)
                            {
                                d.vocab.push(s.clone());
                            }
                        } else if in_ctx && !k.starts_with('@') {
                            let (iri, json) = match y {
                                Value::String(s) => (Some(s.as_str()), false),
                                Value::Object(t) => (
                                    t.get("@id").and_then(Value::as_str),
                                    t.get("@type").and_then(Value::as_str) == Some("@json"),
                                ),
                                _ => (None, false),
                            };
                            if let Some(iri) = iri {
                                // A context repeated on every node is one
                                // definition, not one per node.
                                let iris = d.terms.entry(k.clone()).or_default();
                                if !iris.iter().any(|i| i == iri) {
                                    iris.push(iri.to_string());
                                }
                            }
                            if json {
                                d.json.insert(k.clone());
                            } else {
                                d.plain.insert(k.clone());
                            }
                            // A term definition may carry a scoped context.
                            stack.push((y, depth + 1, true));
                        } else {
                            stack.push((y, depth + 1, in_ctx));
                        }
                    }
                }
                _ => {}
            }
        }
        d
    }

    /// Whether every definition of the term `k` types it `@json`.
    fn json_typed(&self, k: &str) -> bool {
        self.json.contains(k) && !self.plain.contains(k)
    }

    /// Whether the property key `k` is ODRL's and not read as a bare key.
    fn odrl_key(&self, k: &str) -> bool {
        let mut budget = MAX_EXPANSION;
        if k.contains(':') {
            return self.odrl_iri(k, None, &mut budget);
        }
        self.terms
            .get(k)
            .is_some_and(|iris| iris.iter().any(|i| self.odrl_iri(i, Some(k), &mut budget)))
    }

    /// Whether `iri`, as written in a term definition (or as a key), may
    /// expand into the ODRL namespace with a local name other than `own`.
    fn odrl_iri(&self, iri: &str, own: Option<&str>, budget: &mut usize) -> bool {
        if *budget == 0 {
            return true;
        }
        *budget -= 1;
        if let Some(local) = iri.strip_prefix(ODRL_NS) {
            return !local.is_empty() && Some(local) != own;
        }
        if iri.starts_with('@') {
            return false;
        }
        match iri.split_once(':') {
            // An absolute IRI or a blank node, not a compact one.
            Some((_, rest)) if rest.starts_with("//") => false,
            Some(("_", _)) => false,
            Some((p, rest)) => {
                (p == "odrl" && self.odrl_iri(&format!("{ODRL_NS}{rest}"), own, budget))
                    || self.terms.get(p).is_some_and(|iris| {
                        iris.iter()
                            .any(|i| self.odrl_iri(&format!("{i}{rest}"), own, budget))
                    })
            }
            // A term, or relative to the @vocab.
            None => {
                self.terms.get(iri).is_some_and(|iris| {
                    iris.iter()
                        .filter(|i| i.as_str() != iri)
                        .any(|i| self.odrl_iri(i, own, budget))
                }) || self
                    .vocab
                    .iter()
                    .any(|v| self.odrl_iri(&format!("{v}{iri}"), own, budget))
            }
        }
    }
}
