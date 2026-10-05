//! [`EditDoc`] back to JSON-LD, merging into the JSON each node was read
//! from so that whatever was not edited is written back as it was: key
//! order, key spelling, unknown keys and all.
use serde_json::{Map, Value};

use super::jsonld_read::{LOGICAL_KEYS, Reader};
use super::model::*;
use crate::read::{items, policy_nodes};

type Obj = Map<String, Value>;

const CONTEXT: &str = "http://www.w3.org/ns/odrl.jsonld";

/// Set a property under one of several accepted spellings.
///
/// `Some(v)`: store under the first of `keys` already present (keeping its
/// spelling and its position), else under `keys[0]`, appended; every other
/// key in `keys` is removed. `None`: remove all of `keys`. Uses
/// `shift_remove` throughout so the order of the other keys is kept.
pub fn set_property(obj: &mut Map<String, Value>, keys: &[&str], value: Option<Value>) {
    let Some(first) = keys.first() else { return };
    match value {
        None => {
            for k in keys {
                obj.shift_remove(*k);
            }
        }
        Some(v) => {
            let at = keys
                .iter()
                .find(|k| obj.contains_key(**k))
                .copied()
                .unwrap_or(first);
            obj.insert(at.to_string(), v);
            for k in keys {
                if *k != at {
                    obj.shift_remove(*k);
                }
            }
        }
    }
}

/// Write the document as JSON-LD. For a document read by
/// [`read_model_value`](super::read_model_value), `write_jsonld(&doc)` serialises to the same text
/// as the input until something is edited, and afterwards differs only in
/// the edited properties.
pub fn write_jsonld(doc: &EditDoc) -> Value {
    let written: Vec<Value> = doc.policies.iter().map(write_policy).collect();
    match (doc.shape, written.len()) {
        (DocShape::Single, 1) => {
            let only = written.into_iter().next().unwrap_or(Value::Null);
            if doc.policies[0].origin.is_none() {
                if let Value::Object(o) = only {
                    let mut out = Map::new();
                    out.insert("@context".into(), Value::String(CONTEXT.into()));
                    for (k, v) in o {
                        out.insert(k, v);
                    }
                    return Value::Object(out);
                }
                return only;
            }
            only
        }
        (DocShape::Array, _) => match &doc.origin {
            Some(o) if same(&policy_nodes(o.value()), &written) => o.value().clone(),
            _ => Value::Array(written),
        },
        _ => {
            let mut base = match doc.origin.as_ref().map(|o| o.value()) {
                Some(Value::Object(o)) if doc.shape == DocShape::Graph => o.clone(),
                _ => {
                    let mut m = Map::new();
                    m.insert("@context".into(), Value::String(CONTEXT.into()));
                    m
                }
            };
            let unchanged = base
                .get("@graph")
                .is_some_and(|g| same(&policy_nodes(g), &written));
            if !unchanged {
                set_property(&mut base, &["@graph"], Some(Value::Array(written)));
            }
            Value::Object(base)
        }
    }
}

fn same(orig: &[&Value], written: &[Value]) -> bool {
    orig.len() == written.len() && orig.iter().zip(written).all(|(a, b)| *a == b)
}

fn origin_value(o: &Option<Origin>) -> Value {
    o.as_ref().map_or(Value::Null, |o| o.value().clone())
}

/// The origin as (the value, its object), when it is an object.
fn origin_object(o: &Option<Origin>) -> Option<(&Value, &Obj)> {
    match o.as_ref().map(Origin::value) {
        Some(v @ Value::Object(m)) => Some((v, m)),
        _ => None,
    }
}

fn put_opt(obj: &mut Obj, keys: &[&str], now: &Option<String>, before: &Option<String>) {
    if now != before {
        set_property(obj, keys, now.clone().map(Value::String));
    }
}

fn put_str(obj: &mut Obj, keys: &[&str], now: &str, before: &str) {
    if now != before {
        set_property(
            obj,
            keys,
            (!now.is_empty()).then(|| Value::String(now.to_string())),
        );
    }
}

fn put_strings(obj: &mut Obj, key: &str, now: &[String], before: &[String]) {
    if now == before {
        return;
    }
    let bare = now.len() == 1 && obj.get(key).is_some_and(|v| !v.is_array());
    let v = if now.is_empty() {
        None
    } else if bare {
        Some(Value::String(now[0].clone()))
    } else {
        Some(Value::Array(
            now.iter().map(|s| Value::String(s.clone())).collect(),
        ))
    };
    set_property(obj, &[key], v);
}

/// Write a list of child nodes under `key`. Left alone when the children
/// come out as they went in.
/// Rebuild `orig` with its leaf items (as `items` sees them) replaced, in
/// order, by `new`.
fn substitute(orig: &Value, new: &mut std::vec::IntoIter<Value>) -> Value {
    match orig {
        Value::Array(a) => Value::Array(a.iter().map(|x| substitute(x, new)).collect()),
        Value::Object(o) => match ["@list", "@set"].into_iter().find(|k| o.contains_key(*k)) {
            Some(k) => {
                let mut o = o.clone();
                let inner = substitute(&o[k], new);
                o.insert(k.to_string(), inner);
                Value::Object(o)
            }
            None => new.next().unwrap_or(Value::Null),
        },
        _ => new.next().unwrap_or(Value::Null),
    }
}

fn put_children(obj: &mut Obj, key: &str, mut written: Vec<Value>, bare_ok: bool) {
    let orig = obj.get(key).cloned();
    let orig_items: Vec<&Value> = orig.as_ref().map(items).unwrap_or_default();
    if same(&orig_items, &written) {
        return;
    }
    if written.is_empty() {
        obj.shift_remove(key);
        return;
    }
    // Same number of children: edit them where they stand, so a single
    // object stays a single object and a `@list` wrapper stays a wrapper.
    if let Some(orig) = &orig
        && orig_items.len() == written.len()
    {
        let new = substitute(orig, &mut written.into_iter());
        if new != *orig {
            set_property(obj, &[key], Some(new));
        }
        return;
    }
    let bare = bare_ok && written.len() == 1 && orig.as_ref().is_some_and(|v| !v.is_array());
    let v = if bare {
        written.remove(0)
    } else {
        Value::Array(written)
    };
    set_property(obj, &[key], Some(v));
}

const UID: [&str; 3] = ["uid", "@id", "id"];

// --- nodes ----------------------------------------------------------------------

fn write_policy(p: &PolicyNode) -> Value {
    if p.locked.is_some() {
        return origin_value(&p.origin);
    }
    let (mut obj, before) = match origin_object(&p.origin) {
        Some((v, o)) => (o.clone(), Reader::default().policy(v)),
        None => (Map::new(), PolicyNode::default()),
    };
    put_str(&mut obj, &["@type", "type"], &p.kind, &before.kind);
    put_opt(&mut obj, &UID, &p.uid, &before.uid);
    put_children(&mut obj, "assigner", entities(&p.assigner), true);
    put_children(&mut obj, "assignee", entities(&p.assignee), true);
    put_children(&mut obj, "target", entities(&p.target), true);
    put_children(&mut obj, "action", actions(&p.action), true);
    put_strings(&mut obj, "profile", &p.profile, &before.profile);
    put_opt(&mut obj, &["conflict"], &p.conflict, &before.conflict);
    put_strings(
        &mut obj,
        "inheritFrom",
        &p.inherit_from,
        &before.inherit_from,
    );
    put_children(&mut obj, "permission", rules(&p.permission), false);
    put_children(&mut obj, "prohibition", rules(&p.prohibition), false);
    put_children(&mut obj, "obligation", rules(&p.obligation), false);
    Value::Object(obj)
}

fn rules(v: &[RuleNode]) -> Vec<Value> {
    v.iter().map(write_rule).collect()
}

fn write_rule(r: &RuleNode) -> Value {
    if r.locked.is_some() {
        return origin_value(&r.origin);
    }
    if let Some(reference) = &r.reference {
        return Value::String(reference.clone());
    }
    let (mut obj, before) = match origin_object(&r.origin) {
        Some((v, o)) => (o.clone(), Reader::default().rule(v)),
        None => (Map::new(), RuleNode::default()),
    };
    put_opt(&mut obj, &UID, &r.uid, &before.uid);
    put_children(&mut obj, "action", actions(&r.action), true);
    put_children(&mut obj, "target", entities(&r.target), true);
    put_children(&mut obj, "assigner", entities(&r.assigner), true);
    put_children(&mut obj, "assignee", entities(&r.assignee), true);
    put_children(&mut obj, "constraint", constraints(&r.constraint), false);
    put_children(&mut obj, "duty", rules(&r.duty), false);
    put_children(&mut obj, "remedy", rules(&r.remedy), false);
    put_children(&mut obj, "consequence", rules(&r.consequence), false);
    Value::Object(obj)
}

fn entities(v: &[Entity]) -> Vec<Value> {
    v.iter().map(write_entity).collect()
}

fn write_entity(e: &Entity) -> Value {
    if e.locked.is_some() {
        return origin_value(&e.origin);
    }
    let (mut obj, before) = match origin_object(&e.origin) {
        Some((v, o)) => (o.clone(), Reader::default().entity(v, "party")),
        None => {
            if let (Some(iri), None, true) = (&e.iri, &e.part_of, e.refinement.is_empty()) {
                return Value::String(iri.clone());
            }
            (Map::new(), Entity::default())
        }
    };
    put_opt(&mut obj, &UID, &e.iri, &before.iri);
    put_opt(&mut obj, &["partOf", "source"], &e.part_of, &before.part_of);
    put_children(&mut obj, "refinement", constraints(&e.refinement), false);
    Value::Object(obj)
}

fn actions(v: &[ActionNode]) -> Vec<Value> {
    v.iter().map(write_action).collect()
}

/// Store an action name in an object, keeping the shape already there.
fn write_name(obj: &mut Obj, name: &str) {
    let key = ["rdf:value", "value", "@id"]
        .into_iter()
        .find(|k| obj.contains_key(*k));
    let named = |name: &str| {
        let mut m = Map::new();
        m.insert("@id".into(), Value::String(name.to_string()));
        Value::Object(m)
    };
    match key {
        Some("@id") => {
            obj.insert("@id".into(), Value::String(name.to_string()));
        }
        Some(k) => {
            let v = match obj.get(k) {
                Some(Value::String(_)) => Value::String(name.to_string()),
                Some(Value::Object(inner)) => {
                    let mut inner = inner.clone();
                    let at = UID
                        .into_iter()
                        .find(|k| inner.contains_key(*k))
                        .unwrap_or("@id");
                    inner.insert(at.into(), Value::String(name.to_string()));
                    Value::Object(inner)
                }
                _ => named(name),
            };
            obj.insert(k.to_string(), v);
        }
        None => {
            obj.insert("rdf:value".into(), named(name));
        }
    }
}

fn write_action(a: &ActionNode) -> Value {
    if a.locked.is_some() {
        return origin_value(&a.origin);
    }
    let (mut obj, before) = match origin_object(&a.origin) {
        Some((v, o)) => (o.clone(), Reader::default().action(v)),
        None => {
            if a.refinement.is_empty() {
                return Value::String(a.name.clone());
            }
            (Map::new(), ActionNode::default())
        }
    };
    if a.name != before.name || origin_object(&a.origin).is_none() {
        write_name(&mut obj, &a.name);
    }
    put_children(&mut obj, "refinement", constraints(&a.refinement), false);
    Value::Object(obj)
}

fn constraints(v: &[ConstraintNode]) -> Vec<Value> {
    v.iter().map(write_constraint).collect()
}

fn literal_value(l: &Literal) -> Value {
    match l {
        Literal::Str(s) => Value::String(s.clone()),
        Literal::Num(n) => Value::Number(n.clone()),
        Literal::Bool(b) => Value::Bool(*b),
        Literal::Typed { value, datatype } => {
            let mut m = Map::new();
            m.insert("@value".into(), Value::String(value.clone()));
            m.insert("@type".into(), Value::String(datatype.clone()));
            Value::Object(m)
        }
        Literal::Iri(s) => {
            let mut m = Map::new();
            m.insert("@id".into(), Value::String(s.clone()));
            Value::Object(m)
        }
    }
}

fn write_constraint(c: &ConstraintNode) -> Value {
    match c {
        ConstraintNode::Opaque { origin, .. } => origin.value().clone(),
        ConstraintNode::Reference { iri, .. } => Value::String(iri.clone()),
        ConstraintNode::Atomic(a) => write_atomic(a),
        ConstraintNode::Logical(l) => write_logical(l),
    }
}

fn write_atomic(a: &AtomicConstraint) -> Value {
    let (mut obj, before) = match origin_object(&a.origin) {
        Some((v, o)) => {
            let b = match Reader::default().constraint(v) {
                ConstraintNode::Atomic(b) => b,
                _ => nothing(),
            };
            (o.clone(), b)
        }
        None => (Map::new(), nothing()),
    };
    put_str(&mut obj, &["leftOperand"], &a.left, &before.left);
    put_str(&mut obj, &["operator"], &a.operator, &before.operator);
    if a.right != before.right {
        match &a.right {
            RightOperand::Values(v) => {
                let mut vals: Vec<Value> = v.iter().map(literal_value).collect();
                let value = if vals.len() == 1 {
                    vals.remove(0)
                } else {
                    Value::Array(vals)
                };
                set_property(&mut obj, &["rightOperand"], Some(value));
                obj.shift_remove("rightOperandReference");
            }
            RightOperand::Reference(r) => {
                set_property(
                    &mut obj,
                    &["rightOperandReference"],
                    Some(Value::String(r.clone())),
                );
                obj.shift_remove("rightOperand");
            }
            RightOperand::Missing => {
                obj.shift_remove("rightOperand");
                obj.shift_remove("rightOperandReference");
            }
        }
    }
    put_opt(&mut obj, &["unit"], &a.unit, &before.unit);
    Value::Object(obj)
}

fn nothing() -> AtomicConstraint {
    AtomicConstraint {
        right: RightOperand::Missing,
        ..AtomicConstraint::default()
    }
}

fn write_logical(l: &LogicalConstraint) -> Value {
    let new_key = l.op.as_str();
    let children = constraints(&l.children);
    let mut obj = match origin_object(&l.origin) {
        Some((_, o)) => o.clone(),
        None => {
            let mut m = Map::new();
            m.insert(new_key.into(), Value::Array(children));
            return Value::Object(m);
        }
    };
    let Some(old_key) = LOGICAL_KEYS.into_iter().find(|k| obj.contains_key(*k)) else {
        obj.insert(new_key.into(), Value::Array(children));
        return Value::Object(obj);
    };
    if old_key != new_key {
        // Rename the key where it stands, so the other keys keep their order.
        let mut renamed = Map::new();
        for (k, v) in obj {
            if k == old_key {
                renamed.insert(new_key.to_string(), v);
            } else {
                renamed.insert(k, v);
            }
        }
        obj = renamed;
    }
    put_children(&mut obj, new_key, children, false);
    Value::Object(obj)
}
