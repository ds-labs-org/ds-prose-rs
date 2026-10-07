//! [`EditDoc`] back to JSON-LD, merging into the JSON each node was read
//! from so that whatever was not edited is written back as it was: key
//! order, key spelling, unknown keys and all.
use serde_json::{Map, Value};

use super::jsonld_read::{LOGICAL_KEYS, Reader, literal};
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
            Some(o) => rebuild_array(o.value(), written, &doc.policies),
            None => Value::Array(written),
        },
        _ => {
            let mut written = written;
            let mut base = match doc.origin.as_ref().map(|o| o.value()) {
                Some(Value::Object(o)) if doc.shape == DocShape::Graph => o.clone(),
                _ => {
                    // A single policy promoted to a graph: its own `@context`
                    // moves up to the wrapper instead of being repeated.
                    let read = doc.policies.iter().position(|p| p.origin.is_some());
                    let own = match (doc.shape, read.and_then(|i| written.get_mut(i))) {
                        (DocShape::Single, Some(Value::Object(first))) => {
                            first.shift_remove("@context")
                        }
                        _ => None,
                    };
                    let mut m = Map::new();
                    m.insert(
                        "@context".into(),
                        own.unwrap_or_else(|| Value::String(CONTEXT.into())),
                    );
                    m
                }
            };
            let unchanged = base
                .get("@graph")
                .is_some_and(|g| same(&policy_nodes(g), &written));
            if !unchanged {
                // Nested `@graph` wrappers (and their `@context`) stay around
                // the policies they were read around.
                let rebuilt = match base.get("@graph") {
                    Some(g @ Value::Array(_)) if doc.shape == DocShape::Graph => {
                        rebuild_array(g, written, &doc.policies)
                    }
                    _ => Value::Array(written),
                };
                set_property(&mut base, &["@graph"], Some(rebuilt));
            }
            Value::Object(base)
        }
    }
}

/// The shape of an array document: `@graph` wrappers (with their `@context`)
/// around runs of policy nodes, nested to any depth. Policy nodes are
/// numbered in document order, so what a node holds is a range of numbers.
struct Tree<'a> {
    orig: &'a Value,
    lo: usize,
    hi: usize,
    kind: Kind<'a>,
}

enum Kind<'a> {
    Leaf,
    Array(Vec<Tree<'a>>),
    Wrapper {
        obj: &'a Obj,
        graph_is_array: bool,
        kids: Vec<Tree<'a>>,
    },
}

impl<'a> Tree<'a> {
    fn parse(v: &'a Value, leaves: &mut Vec<&'a Value>) -> Tree<'a> {
        let lo = leaves.len();
        let kind = match v {
            Value::Array(a) => Kind::Array(a.iter().map(|x| Tree::parse(x, leaves)).collect()),
            Value::Object(o) if o.contains_key("@graph") => {
                let (graph_is_array, kids) = match &o["@graph"] {
                    Value::Array(a) => (true, a.iter().map(|x| Tree::parse(x, leaves)).collect()),
                    one => (false, vec![Tree::parse(one, leaves)]),
                };
                Kind::Wrapper {
                    obj: o,
                    graph_is_array,
                    kids,
                }
            }
            _ => {
                leaves.push(v);
                Kind::Leaf
            }
        };
        Tree {
            orig: v,
            lo,
            hi: leaves.len(),
            kind,
        }
    }
}

/// A written policy and the original leaf it belongs beside: the one it was
/// read from, or, for a new one, its neighbour's.
struct Placed {
    at: usize,
    value: Value,
}

/// Rebuild an array document with its policies replaced by `written`. Each
/// policy goes back under the `@graph` wrapper (and `@context`) it was read
/// from; a new one goes beside the policy before it (or, first in the
/// document, the one after it); a wrapper left with none of its policies is
/// dropped.
fn rebuild_array(orig: &Value, written: Vec<Value>, policies: &[PolicyNode]) -> Value {
    let mut leaves = Vec::new();
    let tree = Tree::parse(orig, &mut leaves);
    let mut taken = vec![false; leaves.len()];
    // Searching on from the last match keeps the usual, in-order case linear.
    let mut cursor = 0;
    let home: Vec<Option<usize>> = policies
        .iter()
        .map(|p| {
            let o = p.origin.as_ref()?.value();
            let n = leaves.len();
            let k = (cursor..n)
                .chain(0..cursor.min(n))
                .find(|k| !taken[*k] && leaves[*k] == o)?;
            taken[k] = true;
            cursor = k + 1;
            Some(k)
        })
        .collect();
    let last = leaves.len().saturating_sub(1);
    let placed: Vec<(usize, Placed)> = written
        .into_iter()
        .enumerate()
        .map(|(i, value)| {
            let at = home[i]
                .or_else(|| home[..i].iter().rev().find_map(|h| *h))
                .or_else(|| home[i..].iter().find_map(|h| *h))
                .unwrap_or(last);
            (i, Placed { at, value })
        })
        .collect();
    let order: Vec<(usize, Value)> = placed
        .iter()
        .map(|(_, p)| (p.at, p.value.clone()))
        .collect();
    let built = match emit(&tree, placed) {
        Some((_, Value::Array(a))) => Value::Array(a),
        Some((_, other)) => Value::Array(vec![other]),
        None => Value::Array(vec![]),
    };
    // A policy moved from one wrapper to another cannot keep both its own
    // wrapper and its place: the document must read back in the model's order.
    let mut after = Vec::new();
    Tree::parse(&built, &mut after);
    if after.len() == order.len() && after.iter().zip(&order).all(|(a, (_, b))| *a == b) {
        return built;
    }
    let items: Vec<(Value, Vec<&Tree>)> = order
        .into_iter()
        .map(|(at, v)| {
            let mut chain = Vec::new();
            containers(&tree, at, &mut chain);
            (v, chain)
        })
        .collect();
    match runs(&items, 0, &mut Vec::new()).pop() {
        Some(Value::Array(a)) => Value::Array(a),
        Some(other) => Value::Array(vec![other]),
        None => Value::Array(vec![]),
    }
}

/// The containers (arrays and `@graph` wrappers) holding leaf `at`, outermost first.
fn containers<'t, 'a>(node: &'t Tree<'a>, at: usize, out: &mut Vec<&'t Tree<'a>>) {
    let kids = match &node.kind {
        Kind::Leaf => return,
        Kind::Array(k) | Kind::Wrapper { kids: k, .. } => k,
    };
    out.push(node);
    if let Some(kid) = kids.iter().find(|k| (k.lo..k.hi).contains(&at)) {
        containers(kid, at, out);
    }
}

/// Policies in the order given, grouped into the containers they sit in:
/// each unbroken run under one container shares one copy of it, so a
/// container reappears when another one's policy comes between. A container
/// that never held a policy goes into the first copy of its parent, at the
/// position it had among the parent's children.
fn runs<'t>(
    items: &[(Value, Vec<&'t Tree<'t>>)],
    depth: usize,
    seen: &mut Vec<*const Tree<'t>>,
) -> Vec<Value> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let Some(&node) = items[i].1.get(depth) else {
            out.push(items[i].0.clone());
            i += 1;
            continue;
        };
        let mut j = i + 1;
        while j < items.len()
            && items[j]
                .1
                .get(depth)
                .is_some_and(|n| std::ptr::eq(*n, node))
        {
            j += 1;
        }
        let mut inner = runs(&items[i..j], depth + 1, seen);
        if !seen.contains(&std::ptr::from_ref(node)) {
            seen.push(std::ptr::from_ref(node));
            if let Kind::Array(kids) | Kind::Wrapper { kids, .. } = &node.kind {
                for (at, kid) in kids.iter().enumerate() {
                    if kid.lo == kid.hi && !matches!(kid.kind, Kind::Leaf) {
                        inner.insert(at.min(inner.len()), kid.orig.clone());
                    }
                }
            }
        }
        out.push(match &node.kind {
            Kind::Wrapper {
                obj,
                graph_is_array,
                ..
            } => {
                let mut o = (*obj).clone();
                let g = if !graph_is_array && inner.len() == 1 {
                    inner.remove(0)
                } else {
                    Value::Array(inner)
                };
                o.insert("@graph".into(), g);
                Value::Object(o)
            }
            _ => Value::Array(inner),
        });
        i = j;
    }
    out
}

/// `node` with the policies placed under it, and the document position of
/// its first one; `None` when none are left.
fn emit(node: &Tree, pols: Vec<(usize, Placed)>) -> Option<(usize, Value)> {
    let (kids, wrapper) = match &node.kind {
        Kind::Leaf => return None,
        Kind::Array(k) => (k, None),
        Kind::Wrapper {
            obj,
            graph_is_array,
            kids,
        } => (kids, Some((*obj, *graph_is_array))),
    };
    // In document order: each policy placed on a direct leaf, each
    // sub-container that still holds one. A container that never held a
    // policy is kept, after as many of them as came before it.
    let mut out: Vec<(usize, Value)> = Vec::new();
    let mut empties: Vec<(usize, Value)> = Vec::new();
    for kid in kids {
        let mut mine: Vec<(usize, Placed)> = Vec::new();
        for (i, p) in &pols {
            if (kid.lo..kid.hi).contains(&p.at) {
                mine.push((
                    *i,
                    Placed {
                        at: p.at,
                        value: p.value.clone(),
                    },
                ));
            }
        }
        match kid.kind {
            Kind::Leaf => out.extend(mine.into_iter().map(|(i, p)| (i, p.value))),
            _ if mine.is_empty() => {
                if kid.lo == kid.hi {
                    empties.push((out.len(), kid.orig.clone()));
                }
            }
            _ => out.extend(emit(kid, mine)),
        }
    }
    out.sort_by_key(|(i, _)| *i);
    let first = out.first().map(|(i, _)| *i)?;
    let mut vals: Vec<Value> = out.into_iter().map(|(_, v)| v).collect();
    for (n, (after, e)) in empties.into_iter().enumerate() {
        vals.insert((after + n).min(vals.len()), e);
    }
    let value = match wrapper {
        None => Value::Array(vals),
        Some((o, graph_is_array)) => {
            let mut o = o.clone();
            let g = if !graph_is_array && vals.len() == 1 {
                vals.remove(0)
            } else {
                Value::Array(vals)
            };
            o.insert("@graph".into(), g);
            Value::Object(o)
        }
    };
    Some((first, value))
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
        // Written as it was read, unless the reference was edited.
        if let Some(o) = &r.origin
            && Reader::default().rule(o.value()).reference.as_ref() == Some(reference)
        {
            return o.value().clone();
        }
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
    // `{"@id": ..}` alone reads back as a reference; ODRL's own `uid` for the
    // same identifier does not, so a rule emptied down to its id keeps it.
    if obj.len() == 1
        && let Some(id) = obj.shift_remove("@id")
    {
        obj.insert("uid".into(), id);
    }
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

/// An edited value that replaces a value object (`{"@value": 10, "@type":
/// "xsd:integer"}`, `{"@value": "ten", "@language": "en"}`) keeps the
/// wrapper's other keys: the model reads such a value as a plain number,
/// boolean or string, so the writer carries what the model dropped. The
/// `@value` follows the model's variant, so the document re-reads as the
/// model: a string (text a number or boolean could not hold) is never written
/// as a native value.
fn retyped(orig: &Value, l: &Literal) -> Value {
    let Value::Object(o) = orig else {
        return literal_value(l);
    };
    if !o.contains_key("@value") {
        return literal_value(l);
    }
    let mut m = o.clone();
    match l {
        Literal::Num(n) => {
            m.insert("@value".into(), Value::Number(n.clone()));
        }
        Literal::Bool(b) => {
            m.insert("@value".into(), Value::Bool(*b));
        }
        Literal::Typed { value, datatype } => {
            m.insert("@value".into(), Value::String(value.clone()));
            m.insert("@type".into(), Value::String(datatype.clone()));
        }
        Literal::Str(s) => {
            // A plain string carries no datatype (it would read back as
            // `Typed`), but keeps `@language` and the like.
            let typed = m.shift_remove("@type").is_some();
            m.insert("@value".into(), Value::String(s.clone()));
            if typed && m.len() == 1 {
                return Value::String(s.clone());
            }
        }
        Literal::Iri(_) => return literal_value(l),
    }
    Value::Object(m)
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
                let orig = obj.get("rightOperand").cloned();
                let orig_items: Vec<&Value> = orig.as_ref().map(items).unwrap_or_default();
                // A value that still reads as it did is written as it was.
                let mut vals = right_values(&orig_items, v);
                let value = match &orig {
                    // Same number of values: edit them where they stand, so a
                    // bare value stays bare and a `@list` stays a list.
                    Some(o) if orig_items.len() == vals.len() && !vals.is_empty() => {
                        substitute(o, &mut vals.into_iter())
                    }
                    Some(Value::Object(w)) if w.len() == 1 && w.contains_key("@list") => {
                        let mut w = w.clone();
                        w.insert("@list".into(), Value::Array(vals));
                        Value::Object(w)
                    }
                    _ if vals.len() == 1 => vals.remove(0),
                    _ => Value::Array(vals),
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

/// The JSON for the model's right-operand values. A value that still reads as
/// one of the original items is that item, whichever position it moved to
/// (removing one value must not hand its wrapper to the next). An edited
/// value takes over the wrapper of the original at its own position, unless
/// another value already kept that original.
fn right_values(orig_items: &[&Value], now: &[Literal]) -> Vec<Value> {
    let mut claimed = vec![false; orig_items.len()];
    let mut kept: Vec<Option<usize>> = Vec::with_capacity(now.len());
    for l in now {
        let k = (0..orig_items.len())
            .find(|k| !claimed[*k] && literal(orig_items[*k]).as_ref() == Some(l));
        if let Some(k) = k {
            claimed[k] = true;
        }
        kept.push(k);
    }
    now.iter()
        .enumerate()
        .map(|(i, l)| match kept[i] {
            Some(k) => orig_items[k].clone(),
            None => match orig_items.get(i) {
                Some(o) if !claimed[i] => retyped(o, l),
                _ => literal_value(l),
            },
        })
        .collect()
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
    if children.is_empty() {
        // A group keeps its key when it empties; `{}` would read back as an
        // empty atomic constraint.
        let had = obj.get(new_key).map_or(0, |v| items(v).len());
        if had > 0 {
            set_property(&mut obj, &[new_key], Some(Value::Array(vec![])));
        }
    } else {
        put_children(&mut obj, new_key, children, false);
    }
    Value::Object(obj)
}
