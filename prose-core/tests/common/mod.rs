#![allow(dead_code)]
use prose_core::edit::*;
use serde_json::Value;

pub const FIXTURES: &[&str] = &[
    "agreement",
    "offer",
    "set",
    "graph-two",
    "array-top",
    "rule-reference",
    "unknown-keys",
    "entity-collection",
    "operand-reference",
    "list-wrapper",
    "single-object",
    "policy-action",
    "inheritance",
];

pub fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

pub fn fixture_value(name: &str) -> Value {
    serde_json::from_str(&fixture(name)).unwrap()
}

pub fn text_of(v: &Value) -> String {
    serde_json::to_string(v).unwrap()
}

pub fn id_of(doc: &EditDoc, path: &str) -> NodeId {
    let p: NodePath = path.parse().unwrap();
    doc.node(&p)
        .unwrap_or_else(|| panic!("no node {path}"))
        .id()
}

pub fn set_text(doc: &mut EditDoc, slot: &str, value: &str) -> Result<(), EditError> {
    let slot: SlotPath = slot.parse().unwrap();
    let expect = doc.node(&slot.node).map(|n| n.id()).unwrap_or_default();
    doc.apply(
        &EditEvent::SetText {
            slot,
            expect,
            value: value.to_string(),
        },
        &EditRules::default(),
    )
}

pub fn lp(s: &str) -> ListPath {
    s.parse().unwrap()
}

pub fn np(s: &str) -> NodePath {
    s.parse().unwrap()
}

pub const RICH: &str = r#"{
  "@type": "Set", "uid": "urn:policy:r",
  "assigner": "urn:a", "assignee": "urn:b",
  "target": {"uid": "urn:t", "refinement": [{"leftOperand": "fileFormat", "operator": "eq", "rightOperand": "mp4"}]},
  "action": {"rdf:value": {"@id": "use"}, "refinement": [{"leftOperand": "count", "operator": "lteq", "rightOperand": 3}]},
  "profile": ["urn:p1"], "inheritFrom": ["urn:parent"],
  "permission": [{
    "action": "use", "target": "urn:x",
    "constraint": [{"or": [{"leftOperand": "purpose", "operator": "isAnyOf", "rightOperand": ["a", "b"]}]}],
    "duty": [{"action": "attribute", "consequence": [{"action": "compensate"}]}]
  }],
  "prohibition": [{"action": "copy", "remedy": [{"action": "delete"}]}],
  "obligation": [{"action": "inform"}]
}"#;

pub fn rich() -> EditDoc {
    read_model(RICH).unwrap()
}

/// The id to pass as `expect` for the item at `index` of `list`, or for
/// the owner of a list of plain values.
pub fn item_id(doc: &EditDoc, list: &ListPath, index: usize) -> NodeId {
    match list.item_path(index) {
        Some(p) => doc.node(&p).expect("item").id(),
        None => match &list.owner {
            Some(o) => doc.node(o).expect("owner").id(),
            None => NodeId(0),
        },
    }
}

pub fn list_len(doc: &EditDoc, list: &ListPath) -> usize {
    let mut n = 0;
    match list.item_path(0) {
        Some(_) => {
            while list.item_path(n).and_then(|p| doc.node(&p)).is_some() {
                n += 1;
            }
        }
        None => {
            let owner = list.owner.as_ref().and_then(|o| doc.node(o)).unwrap();
            n = match (owner, list.kind) {
                (NodeRef::Policy(p), ListKind::Profile) => p.profile.len(),
                (NodeRef::Policy(p), ListKind::InheritFrom) => p.inherit_from.len(),
                (NodeRef::Constraint(ConstraintNode::Atomic(a)), ListKind::RightOperand) => {
                    match &a.right {
                        RightOperand::Values(v) => v.len(),
                        _ => 0,
                    }
                }
                _ => 0,
            };
        }
    }
    n
}
