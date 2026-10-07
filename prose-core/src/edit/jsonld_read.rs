//! JSON-LD in, [`EditDoc`] out. Reads the same shapes `render` does, but
//! keeps every node addressable and remembers the JSON it came from.
use serde_json::{Map, Value};

use super::model::*;
use crate::ProseError;
use crate::read::{
    POLICY_KEYS, QUIET_PREFIXES, RULE_KEYS, bare_reference, get, ident, items, looks_like_policy,
    policy_nodes, types,
};
use crate::words::local_name;

type Obj = Map<String, Value>;

pub(crate) const LOGICAL_KEYS: [&str; 4] = ["and", "or", "xone", "andSequence"];

/// Parse `json` and read it into the editable model.
pub fn read_model(json: &str) -> Result<EditDoc, ProseError> {
    let value: Value = serde_json::from_str(json).map_err(|e| ProseError::Json(e.to_string()))?;
    read_model_value(&value)
}

/// As [`read_model`], for a value that is already parsed. Accepts what
/// `render_value` accepts and fails with `NoPolicy` in the same cases.
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
    let mut doc = EditDoc::new(policies);
    doc.shape = shape;
    doc.origin = origin;
    doc.warnings = r.warnings;
    Ok(doc)
}

#[derive(Default)]
pub(crate) struct Reader {
    pub warnings: Vec<String>,
    /// The same messages, for de-duplication in constant time.
    seen: std::collections::HashSet<String>,
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
        for k in o.keys() {
            if known.contains(&k.as_str()) || QUIET_PREFIXES.iter().any(|p| k.starts_with(p)) {
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
        self.unknown_keys(o, POLICY_KEYS, "a policy");
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
            permission: self.rules(o.get("permission")),
            prohibition: self.rules(o.get("prohibition")),
            obligation: self.rules(o.get("obligation")),
        };
        if p.permission.is_empty() && p.prohibition.is_empty() && p.obligation.is_empty() {
            self.warn("a policy has no permission, prohibition or obligation");
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
