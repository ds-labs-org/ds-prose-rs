mod common;
use common::*;
use prose_core::edit::*;
use serde_json::{Value, json};

#[test]
fn every_fixture_reads() {
    for name in FIXTURES {
        let doc = read_model(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!doc.policies.is_empty(), "{name}");
    }
}

#[test]
fn write_is_lossless_down_to_key_order() {
    for name in FIXTURES {
        let v = fixture_value(name);
        let doc = read_model_value(&v).unwrap();
        assert_eq!(text_of(&write_jsonld(&doc)), text_of(&v), "{name}");
    }
}

#[test]
fn shapes_are_recognised() {
    let shape = |n: &str| read_model(&fixture(n)).unwrap().shape;
    assert_eq!(shape("graph-two"), DocShape::Graph);
    assert_eq!(shape("array-top"), DocShape::Array);
    assert_eq!(shape("offer"), DocShape::Single);
    assert_eq!(read_model(&fixture("graph-two")).unwrap().policies.len(), 2);
}

#[test]
fn ids_are_numbered_depth_first_from_one() {
    let doc = read_model(&fixture("offer")).unwrap();
    assert_eq!(doc.policies[0].id, NodeId(1));
    let perm = &doc.policies[0].permission[0];
    // policy, assigner, then the rule, its action, target, constraint, duty...
    assert_eq!(doc.policies[0].assigner[0].id, NodeId(2));
    assert_eq!(perm.id, NodeId(3));
    assert_eq!(perm.action[0].id, NodeId(4));
    assert_eq!(perm.target[0].id, NodeId(5));
    assert_eq!(perm.constraint[0].id(), NodeId(6));
    assert_eq!(perm.duty[0].id, NodeId(7));
}

/// (fixture, slot, new text, JSON pointer of the one property that changes)
const LEAF_EDITS: &[(&str, &str, &str, &str)] = &[
    (
        "offer",
        "policy[0].permission[0].action[0]#name",
        "play",
        "/permission/0/action",
    ),
    (
        "agreement",
        "policy[0].permission[0].constraint[0].child[0]#rightOperand[0]",
        "science",
        "/permission/0/constraint/0/or/0/rightOperand/0",
    ),
    ("set", "policy[0]#uid", "urn:policy:changed", "/uid"),
    (
        "graph-two",
        "policy[1]#uid",
        "urn:policy:changed",
        "/@graph/1/uid",
    ),
    ("array-top", "policy[1]#uid", "urn:policy:changed", "/1/uid"),
    (
        "rule-reference",
        "policy[0].permission[0]#reference",
        "urn:rule:other",
        "/permission/0",
    ),
    (
        "unknown-keys",
        "policy[0].permission[0].constraint[0]#leftOperand",
        "elapsedTime",
        "/permission/0/constraint/0/leftOperand",
    ),
    (
        "entity-collection",
        "policy[0].permission[0].target[0]#partOf",
        "urn:collection:films",
        "/permission/0/target/partOf",
    ),
    (
        "operand-reference",
        "policy[0].permission[0].constraint[1]#rightOperandReference",
        "urn:ref:other",
        "/permission/0/constraint/1/rightOperandReference",
    ),
    (
        "operand-reference",
        "policy[0].permission[0].constraint[0]#rightOperand[0]",
        "11.00",
        "/permission/0/constraint/0/rightOperand/@value",
    ),
    (
        "list-wrapper",
        "policy[0].permission[0].constraint[0]#rightOperand[0]",
        "science",
        "/permission/0/constraint/@list/0/rightOperand",
    ),
    (
        "single-object",
        "policy[0].permission[0].action[0]#name",
        "play",
        "/permission/action",
    ),
    (
        "policy-action",
        "policy[0].action[0]#name",
        "odrl:play",
        "/action/rdf:value/@id",
    ),
    (
        "inheritance",
        "policy[0]#profile[1]",
        "urn:profile:three",
        "/profile/1",
    ),
    (
        "inheritance",
        "policy[0]#inheritFrom[0]",
        "urn:policy:other",
        "/inheritFrom",
    ),
];

#[test]
fn a_leaf_edit_changes_only_that_property() {
    for (name, slot, value, pointer) in LEAF_EDITS {
        let v = fixture_value(name);
        let mut doc = read_model_value(&v).unwrap();
        set_text(&mut doc, slot, value).unwrap_or_else(|e| panic!("{name} {slot}: {e}"));
        let mut expected = v.clone();
        *expected
            .pointer_mut(pointer)
            .unwrap_or_else(|| panic!("{name}: no {pointer}")) = Value::String(value.to_string());
        let written = write_jsonld(&doc);
        assert_eq!(text_of(&written), text_of(&expected), "{name} {slot}");
        // And what was written reads back to the same model.
        let again = read_model_value(&written).unwrap();
        assert_eq!(again.normalized(), doc.normalized(), "{name} {slot}");
    }
}

#[test]
fn choices_edit_one_property() {
    let v = fixture_value("inheritance");
    let mut doc = read_model_value(&v).unwrap();
    let slot: SlotPath = "policy[0]#conflict".parse().unwrap();
    doc.apply(
        &EditEvent::SetChoice {
            slot: slot.clone(),
            expect: doc.policies[0].id,
            value: "prohibit".into(),
        },
        &EditRules::default(),
    )
    .unwrap();
    let mut expected = v.clone();
    expected["conflict"] = json!("prohibit");
    assert_eq!(text_of(&write_jsonld(&doc)), text_of(&expected));
    // Clearing removes the key and keeps the rest in place.
    doc.apply(
        &EditEvent::SetChoice {
            slot,
            expect: doc.policies[0].id,
            value: String::new(),
        },
        &EditRules::default(),
    )
    .unwrap();
    let mut expected = v.clone();
    expected.as_object_mut().unwrap().shift_remove("conflict");
    assert_eq!(text_of(&write_jsonld(&doc)), text_of(&expected));
}

#[test]
fn structural_edits_round_trip_through_the_model() {
    for name in FIXTURES {
        let v = fixture_value(name);
        let mut doc = read_model_value(&v).unwrap();
        let rules = EditRules::default();
        // Add a rule, a constraint and a profile to the first policy.
        let p0 = np("policy[0]");
        for (list, item) in [
            (
                ListPath::of(&p0, ListKind::Rules(RuleList::Obligation)),
                NewItem::Default,
            ),
            (ListPath::of(&p0, ListKind::Profile), NewItem::Default),
            (
                ListPath::of(&p0, ListKind::Entities(EntityRole::Target)),
                NewItem::Default,
            ),
        ] {
            let index = {
                let n = doc.policies[0].obligation.len();
                match list.kind {
                    ListKind::Rules(_) => n,
                    ListKind::Profile => doc.policies[0].profile.len(),
                    _ => doc.policies[0].target.len(),
                }
            };
            doc.apply(&EditEvent::Add { list, index, item }, &rules)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        let written = write_jsonld(&doc);
        let again = read_model_value(&written).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(again.normalized(), doc.normalized(), "{name}");
    }
}

#[test]
fn a_new_single_document_gets_a_context_first() {
    let mut doc = EditDoc::new(vec![PolicyNode {
        kind: "Set".into(),
        uid: Some("urn:policy:1".into()),
        ..PolicyNode::default()
    }]);
    doc.policies[0].permission.push(RuleNode {
        action: vec![ActionNode {
            name: "use".into(),
            ..ActionNode::default()
        }],
        ..RuleNode::default()
    });
    doc.renumber();
    let v = write_jsonld(&doc);
    assert_eq!(
        text_of(&v),
        r#"{"@context":"http://www.w3.org/ns/odrl.jsonld","@type":"Set","uid":"urn:policy:1","permission":[{"action":["use"]}]}"#
    );
}

#[test]
fn removing_a_policy_from_a_graph_and_an_array_works() {
    for name in ["graph-two", "array-top"] {
        let v = fixture_value(name);
        let mut doc = read_model_value(&v).unwrap();
        let expect = doc.policies[0].id;
        doc.apply(
            &EditEvent::Remove {
                list: ListPath::policies(),
                index: 0,
                expect,
            },
            &EditRules::default(),
        )
        .unwrap();
        let written = write_jsonld(&doc);
        let again = read_model_value(&written).unwrap();
        assert_eq!(again.policies.len(), 1, "{name}");
        assert_eq!(again.normalized(), doc.normalized(), "{name}");
    }
}

#[test]
fn set_property_keeps_spelling_and_position() {
    let mut o: serde_json::Map<String, Value> =
        serde_json::from_str(r#"{"a":1,"@id":"x","b":2,"uid":"y"}"#).unwrap();
    set_property(&mut o, &["uid", "@id", "id"], Some(json!("z")));
    // Written under the first listed key that is present; the others go.
    assert_eq!(
        text_of(&Value::Object(o.clone())),
        r#"{"a":1,"b":2,"uid":"z"}"#
    );
    set_property(&mut o, &["uid"], None);
    assert_eq!(text_of(&Value::Object(o.clone())), r#"{"a":1,"b":2}"#);
    set_property(&mut o, &["q", "r"], Some(json!(0)));
    assert_eq!(text_of(&Value::Object(o)), r#"{"a":1,"b":2,"q":0}"#);
}

#[test]
fn locked_and_odd_nodes_are_kept() {
    let v = json!({
        "@type": "Set",
        "permission": [
            7,
            {"action": ["use", 5], "constraint": [3, {"and": [], "or": []}, {"leftOperand": "count", "operator": "eq", "rightOperand": [null]}]}
        ],
        "assignee": [{"x": 1}]
    });
    let doc = read_model_value(&v).unwrap();
    assert!(doc.policies[0].permission[0].locked.is_some());
    assert!(doc.policies[0].assignee[0].locked.is_some());
    let c = &doc.policies[0].permission[1].constraint;
    assert!(matches!(c[0], ConstraintNode::Opaque { .. }));
    assert!(matches!(c[1], ConstraintNode::Opaque { .. }));
    assert!(matches!(c[2], ConstraintNode::Opaque { .. }));
    assert_eq!(text_of(&write_jsonld(&doc)), text_of(&v));
}

#[test]
fn non_policy_nodes_are_kept_in_place() {
    let v = json!({"@graph": [{"hello": 1}, {"@type": "Set", "permission": [{"action": "use"}]}]});
    let doc = read_model_value(&v).unwrap();
    assert_eq!(doc.policies.len(), 2);
    assert_eq!(
        doc.policies[0].locked.as_deref(),
        Some("not an ODRL policy")
    );
    assert!(
        doc.warnings
            .iter()
            .any(|w| w == "skipped a node that is not an ODRL policy")
    );
    assert_eq!(text_of(&write_jsonld(&doc)), text_of(&v));
}

#[test]
fn no_policy_is_the_same_error_as_render() {
    assert_eq!(
        read_model(r#"{"hello":1}"#),
        Err(prose_core::ProseError::NoPolicy)
    );
    assert!(matches!(
        read_model("{"),
        Err(prose_core::ProseError::Json(_))
    ));
    assert_eq!(read_model("[1,2]"), Err(prose_core::ProseError::NoPolicy));
}

#[test]
fn warnings_match_the_v01_reader() {
    let doc = read_model(&fixture("unknown-keys")).unwrap();
    let v01 = prose_core::render(&fixture("unknown-keys")).unwrap();
    assert_eq!(doc.warnings, v01.warnings);
}
