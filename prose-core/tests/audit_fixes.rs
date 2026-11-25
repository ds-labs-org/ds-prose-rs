//! Regression tests for the findings of the prose-core audit. Each test is
//! named after the defect it pins.
mod common;
use common::*;
use prose_core::edit::*;
use serde_json::{Value, json};

fn rules() -> EditRules {
    EditRules::default()
}

fn rn(v: Value) -> EditDoc {
    read_model_value(&v).unwrap()
}

// #1
#[test]
fn reference_rule_refuses_every_add() {
    let mut doc = read_model(r#"{"@type":"Set","permission":["urn:rule:1"]}"#).unwrap();
    for l in [
        "policy[0].permission[0]@constraint",
        "policy[0].permission[0]@duty",
        "policy[0].permission[0]@action",
        "policy[0].permission[0]@target",
        "policy[0].permission[0]@assigner",
        "policy[0].permission[0]@assignee",
    ] {
        let list = lp(l);
        assert!(!rules().can_add(&doc, &list), "can_add offered {l}");
        let r = doc.apply(&add_ev(&doc, list, 0, NewItem::Default), &rules());
        assert!(r.is_err(), "{l}: reducer accepted an add the writer drops");
    }
}

// #2
#[test]
fn removing_the_last_child_of_a_group_keeps_a_group() {
    let mut doc = rn(
        json!({"@type":"Set","permission":[{"action":"use","constraint":[
        {"and":[{"leftOperand":"count","operator":"eq","rightOperand":1}]}]}]}),
    );
    let list = lp("policy[0].permission[0].constraint[0]@child");
    let expect = item_id(&doc, &list, 0);
    doc.apply(
        &EditEvent::Remove {
            list,
            index: 0,
            expect,
        },
        &rules(),
    )
    .unwrap();
    let written = write_jsonld(&doc);
    let again = read_model_value(&written).unwrap();
    assert_eq!(again.normalized(), doc.normalized(), "{written}");
}

// #4
#[test]
fn change_rule_kind_respects_the_source_minimum() {
    let mut r = rules();
    r.permission = Limit::exactly(1);
    let mut doc = read_model(r#"{"@type":"Set","permission":[{"action":"use"}]}"#).unwrap();
    let expect = id_of(&doc, "policy[0].permission[0]");
    let res = doc.apply(
        &EditEvent::ChangeRuleKind {
            rule: np("policy[0].permission[0]"),
            expect,
            to: RuleList::Prohibition,
        },
        &r,
    );
    assert!(
        res.is_err(),
        "kind change left permission below its minimum"
    );
    assert_eq!(doc.policies[0].permission.len(), 1);
}

// #5 / #27
#[test]
fn typed_numeric_and_boolean_literals_keep_their_datatype_after_an_edit() {
    for (right, text, want) in [
        (
            json!({"@value":10,"@type":"xsd:integer"}),
            "20",
            json!({"@value":20,"@type":"xsd:integer"}),
        ),
        (
            json!({"@value":true,"@type":"xsd:boolean"}),
            "false",
            json!({"@value":false,"@type":"xsd:boolean"}),
        ),
    ] {
        let mut doc = rn(
            json!({"@type":"Set","permission":[{"action":"use","constraint":[
            {"leftOperand":"count","operator":"lteq","rightOperand":right}]}]}),
        );
        set_text(
            &mut doc,
            "policy[0].permission[0].constraint[0]#rightOperand[0]",
            text,
        )
        .unwrap();
        let w = write_jsonld(&doc);
        assert_eq!(w["permission"][0]["constraint"][0]["rightOperand"], want);
    }
}

#[test]
fn typed_literal_survives_a_sibling_edit_unchanged() {
    let mut doc = rn(
        json!({"@type":"Set","permission":[{"action":"use","constraint":[
        {"leftOperand":"count","operator":"isAnyOf","rightOperand":[{"@value":10,"@type":"xsd:integer"},"b"]}]}]}),
    );
    set_text(
        &mut doc,
        "policy[0].permission[0].constraint[0]#rightOperand[1]",
        "c",
    )
    .unwrap();
    let w = write_jsonld(&doc);
    assert_eq!(
        w["permission"][0]["constraint"][0]["rightOperand"],
        json!([{"@value":10,"@type":"xsd:integer"},"c"])
    );
}

// #6
#[test]
fn clearing_part_of_of_a_collection_only_entity_is_refused() {
    let mut doc =
        rn(json!({"@type":"Set","permission":[{"action":"use","target":{"partOf":"urn:coll"}}]}));
    let r = set_text(&mut doc, "policy[0].permission[0].target[0]#partOf", "");
    assert!(matches!(r, Err(EditError::Invalid(_))), "{r:?}");
    assert_eq!(
        doc.policies[0].permission[0].target[0].part_of.as_deref(),
        Some("urn:coll")
    );
}

// #7
#[test]
fn editing_a_right_operand_keeps_its_list_wrapper_and_array_shape() {
    let slot = "policy[0].permission[0].constraint[0]#rightOperand[0]";
    let base = |r: Value| {
        json!({"@type":"Set","permission":[{"action":"use","target":"urn:t","constraint":[
            {"leftOperand":"purpose","operator":"isAnyOf","rightOperand":r}]}]})
    };
    for (r, want) in [
        (
            json!({"@list":[{"@id":"urn:a"},{"@id":"urn:b"}]}),
            json!({"@list":[{"@id":"urn:new"},{"@id":"urn:b"}]}),
        ),
        (json!(["urn:a"]), json!(["urn:new"])),
        (json!({"@list":["a"]}), json!({"@list":["urn:new"]})),
    ] {
        let mut doc = rn(base(r));
        set_text(&mut doc, slot, "urn:new").unwrap();
        let w = write_jsonld(&doc);
        let got = &w["permission"][0]["constraint"][0]["rightOperand"];
        assert_eq!(*got, want);
    }
}

// #8
#[test]
fn wrap_meets_the_group_minimum() {
    let mut r = rules();
    r.logical_children = Limit::at_least(2);
    let mut doc = rn(
        json!({"@type":"Set","permission":[{"action":"use","constraint":[
        {"leftOperand":"purpose","operator":"eq","rightOperand":"research"}]}]}),
    );
    let p = "policy[0].permission[0].constraint[0]";
    let expect = id_of(&doc, p);
    doc.apply(
        &EditEvent::Wrap {
            constraint: np(p),
            expect,
            op: LogicalOp::And,
        },
        &r,
    )
    .unwrap();
    let Some(NodeRef::Constraint(ConstraintNode::Logical(l))) = doc.node(&np(p)) else {
        panic!("not a group")
    };
    assert!(l.children.len() >= 2, "{} children", l.children.len());
}

// #9
#[test]
fn switching_to_a_single_value_operator_with_several_values_is_refused() {
    let mut doc = rn(
        json!({"@type":"Set","uid":"urn:p","permission":[{"action":"use","target":"urn:x","constraint":[
        {"leftOperand":"count","operator":"isAnyOf","rightOperand":[1,2,3]}]}]}),
    );
    let p = "policy[0].permission[0].constraint[0]";
    let expect = id_of(&doc, p);
    let r = doc.apply(
        &EditEvent::SetChoice {
            slot: format!("{p}#operator").parse().unwrap(),
            expect,
            value: "eq".into(),
        },
        &rules(),
    );
    assert!(r.is_err(), "{r:?}");
    // with one value left the switch is fine
    let mut one = rn(
        json!({"@type":"Set","permission":[{"action":"use","constraint":[
        {"leftOperand":"count","operator":"isAnyOf","rightOperand":[1]}]}]}),
    );
    let expect = id_of(&one, p);
    one.apply(
        &EditEvent::SetChoice {
            slot: format!("{p}#operator").parse().unwrap(),
            expect,
            value: "eq".into(),
        },
        &rules(),
    )
    .unwrap();
}

// #10
#[test]
fn set_text_reference_on_an_ordinary_rule_is_refused() {
    let mut doc = read_model(r#"{"@type":"Set","permission":[{"action":"use"}]}"#).unwrap();
    let r = set_text(&mut doc, "policy[0].permission[0]#reference", "urn:other");
    assert!(r.is_err(), "{r:?}");
    assert!(doc.policies[0].permission[0].reference.is_none());
}

// #11
#[test]
fn numeric_text_that_would_round_stays_text() {
    for text in [
        "18446744073709551616",
        "123456789012345678901234567890",
        "1e3",
        "-0",
        "0.1234567890123456789",
    ] {
        let l = Literal::Num(serde_json::Number::from(5u64)).with_text(text);
        assert_eq!(l, Literal::Str(text.into()), "{text}");
    }
    for text in ["42", "-7", "1.5", "0", "18446744073709551615"] {
        assert!(
            matches!(
                Literal::Num(serde_json::Number::from(5u64)).with_text(text),
                Literal::Num(_)
            ),
            "{text}"
        );
    }
    let l = Literal::Num(serde_json::Number::from(5u64)).with_text("42");
    assert_eq!(l.raw(), "42");
}

// #12
#[test]
fn clearing_a_policy_kind_is_refused() {
    let mut doc = read_model(r#"{"@type":"Set","uid":"urn:p"}"#).unwrap();
    let r = set_text(&mut doc, "policy[0]#kind", "");
    assert!(r.is_err(), "{r:?}");
    assert_eq!(doc.policies[0].kind, "Set");
}

// #13
#[test]
fn follow_up_labels_do_not_misname_the_rule() {
    let d = prose_core::render(
        r#"{"@type":"Set","permission":[{"action":"use","remedy":[{"action":"y"}],"consequence":[{"action":"z"}]}]}"#,
    )
    .unwrap();
    for f in &d.policies[0].rules[0].follow_ups {
        assert!(
            !f.label.contains("this prohibition") && !f.label.contains("this duty"),
            "{}",
            f.label
        );
    }
}

// #14
#[test]
fn a_bare_node_reference_object_is_a_rule_reference() {
    let s = prose_core::render(r#"{"@type":"Set","permission":["urn:rule:1"]}"#).unwrap();
    let o = prose_core::render(r#"{"@type":"Set","permission":[{"@id":"urn:rule:1"}]}"#).unwrap();
    assert_eq!(
        s.policies[0].rules[0].sentence,
        o.policies[0].rules[0].sentence
    );
    let text = r#"{"@type":"Set","permission":[{"@id":"urn:rule:1"}]}"#;
    let doc = read_model(text).unwrap();
    assert_eq!(
        doc.policies[0].permission[0].reference.as_deref(),
        Some("urn:rule:1")
    );
    // untouched, it is written back as it was
    assert_eq!(
        text_of(&write_jsonld(&doc)),
        text_of(&serde_json::from_str::<Value>(text).unwrap())
    );
}

// #15
#[test]
fn many_distinct_unknown_keys_are_linear() {
    let mut o = serde_json::Map::new();
    o.insert("@type".into(), json!("Set"));
    o.insert(
        "permission".into(),
        json!([{"target":"urn:t","action":"use"}]),
    );
    for i in 0..40_000 {
        o.insert(format!("zz{i}"), json!(1));
    }
    let v = Value::Object(o);
    let t = std::time::Instant::now();
    prose_core::render_value(&v).unwrap();
    read_model_value(&v).unwrap();
    let el = t.elapsed();
    assert!(el.as_millis() < 1500, "took {el:?}");
}

// #28
#[test]
fn array_with_graph_wrapper_keeps_wrapper_and_context_after_an_edit() {
    let v = json!([{"@context":"http://www.w3.org/ns/odrl.jsonld","@graph":[
        {"@type":"Set","uid":"urn:p","permission":[{"action":"use"}]}]}]);
    let mut doc = rn(v);
    set_text(&mut doc, "policy[0]#uid", "urn:q").unwrap();
    assert_eq!(
        write_jsonld(&doc),
        json!([{"@context":"http://www.w3.org/ns/odrl.jsonld","@graph":[
            {"@type":"Set","uid":"urn:q","permission":[{"action":"use"}]}]}])
    );
}

// #29
#[test]
fn promoting_a_single_policy_hoists_its_context_once() {
    let mut doc = rn(
        json!({"@context":"http://www.w3.org/ns/odrl.jsonld","@type":"Set","uid":"urn:p","permission":[{"action":"use"}]}),
    );
    doc.apply(
        &add_ev(&doc, ListPath::policies(), 1, NewItem::Default),
        &rules(),
    )
    .unwrap();
    let w = write_jsonld(&doc);
    assert_eq!(w["@context"], json!("http://www.w3.org/ns/odrl.jsonld"));
    assert!(w["@graph"][0].get("@context").is_none(), "{w}");
}

// #31
#[test]
fn value_objects_render_with_sorted_keys_as_in_v01() {
    let d = prose_core::render(
        r#"{"@type":"Set","permission":[{"action":"use","constraint":[{"leftOperand":"x","operator":"eq","rightOperand":{"@value":{"b":1,"a":2}}}]}]}"#,
    )
    .unwrap();
    assert_eq!(
        d.policies[0].rules[0].conditions[0].text,
        r#"the x is equal to {"a":2,"b":1}"#
    );
}

// #3
#[test]
fn a_stale_add_is_refused_when_its_owner_has_moved_on() {
    let mut doc = rn(json!([
        {"@type":"Set","uid":"urn:p0","permission":[{"action":"use"}]},
        {"@type":"Set","uid":"urn:p1","permission":[{"action":"use"}]}
    ]));
    let list = lp("policy[0]@permission");
    let add = add_ev(&doc, list, 1, NewItem::Default);
    let expect = item_id(&doc, &ListPath::policies(), 0);
    doc.apply(
        &EditEvent::Remove {
            list: ListPath::policies(),
            index: 0,
            expect,
        },
        &rules(),
    )
    .unwrap();
    let r = doc.apply(&add, &rules());
    assert!(matches!(r, Err(EditError::Stale { .. })), "{r:?}");
    assert_eq!(doc.policies[0].permission.len(), 1, "p1 must be untouched");
}

// #3
#[test]
fn an_add_must_carry_exactly_the_expectation_its_list_implies() {
    let mut doc = rn(json!({"@type":"Set","uid":"urn:p0","permission":[{"action":"use"}]}));
    let before = doc.clone();
    let with_owner = lp("policy[0]@permission");
    let missing = EditEvent::Add {
        list: with_owner.clone(),
        index: 0,
        expect: None,
        item: NewItem::Default,
    };
    let surplus = EditEvent::Add {
        list: ListPath::policies(),
        index: 0,
        expect: Some(NodeId(1)),
        item: NewItem::Default,
    };
    for ev in [missing, surplus] {
        let r = doc.apply(&ev, &rules());
        assert!(matches!(r, Err(EditError::Invalid(_))), "{r:?}");
        assert_eq!(doc, before);
    }
    assert_eq!(doc.list_owner_id(&ListPath::policies()), None);
    assert_eq!(
        doc.list_owner_id(&with_owner),
        Some(id_of(&doc, "policy[0]"))
    );
}
