//! Regression tests for the findings of the second prose-core audit round.
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

fn two_wrappers() -> Value {
    json!([
        {"@context":{"ex":"http://a.example/"},"@graph":[
            {"@type":"Set","uid":"urn:p1","permission":[{"action":"use"}]}]},
        {"@context":{"ex":"http://b.example/"},"@graph":[
            {"@type":"Set","uid":"urn:p2","permission":[{"action":"ex:act"}]}]}
    ])
}

fn add_policy(doc: &mut EditDoc, index: usize) {
    doc.apply(
        &add_ev(doc, ListPath::policies(), index, NewItem::Default),
        &rules(),
    )
    .unwrap();
}

// R1
#[test]
fn removing_a_graph_wrapped_policy_keeps_the_others_under_their_own_context() {
    let mut doc = rn(two_wrappers());
    let list = ListPath::policies();
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
    assert_eq!(
        write_jsonld(&doc),
        json!([{"@context":{"ex":"http://b.example/"},"@graph":[
            {"@type":"Set","uid":"urn:p2","permission":[{"action":"ex:act"}]}]}])
    );
}

// R1
#[test]
fn adding_a_policy_before_graph_wrapped_ones_keeps_the_others_under_their_own_context() {
    let mut doc = rn(two_wrappers());
    add_policy(&mut doc, 0);
    let w = write_jsonld(&doc);
    let arr = w.as_array().unwrap();
    assert_eq!(arr.len(), 2, "{w}");
    assert_eq!(arr[0]["@context"], json!({"ex":"http://a.example/"}));
    assert_eq!(arr[1]["@context"], json!({"ex":"http://b.example/"}));
    let a: Vec<&Value> = arr[0]["@graph"].as_array().unwrap().iter().collect();
    assert_eq!(a.len(), 2, "{w}");
    assert_eq!(a[1]["uid"], json!("urn:p1"));
    assert_eq!(
        arr[1]["@graph"],
        json!([
        {"@type":"Set","uid":"urn:p2","permission":[{"action":"ex:act"}]}])
    );
}

// R1
#[test]
fn adding_a_policy_between_wrappers_lands_with_its_neighbour() {
    let mut doc = rn(two_wrappers());
    add_policy(&mut doc, 1);
    let w = write_jsonld(&doc);
    let arr = w.as_array().unwrap();
    assert_eq!(arr.len(), 2, "{w}");
    assert_eq!(arr[1]["@graph"].as_array().unwrap().len(), 1, "{w}");
    assert_eq!(arr[1]["@graph"][0]["uid"], json!("urn:p2"));
    assert_eq!(arr[0]["@graph"].as_array().unwrap().len(), 2, "{w}");
}

// R2
#[test]
fn a_rule_edited_down_to_its_uid_stays_an_editable_rule() {
    for key in ["uid", "@id"] {
        let mut doc = rn(json!({"@type":"Set","permission":[{key:"urn:r1","target":"urn:t"}]}));
        let list = lp("policy[0].permission[0]@target");
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
        assert!(rules().can_add(&doc, &lp("policy[0].permission[0]@action")));
        let written = write_jsonld(&doc);
        let again = read_model_value(&written).unwrap();
        assert!(
            again.policies[0].permission[0].reference.is_none(),
            "{key}: {written} re-read as a reference"
        );
        assert!(rules().can_add(&again, &lp("policy[0].permission[0]@action")));
        assert_eq!(again.normalized(), doc.normalized(), "{key}");
    }
}

// R3
#[test]
fn typed_native_literals_stay_numeric_in_the_model() {
    let doc = rn(
        json!({"@type":"Set","permission":[{"action":"use","constraint":[
        {"leftOperand":"count","operator":"lteq","rightOperand":{"@value":10,"@type":"xsd:integer"}},
        {"leftOperand":"flag","operator":"eq","rightOperand":{"@value":true,"@type":"xsd:boolean"}}]}]}),
    );
    let cs = &doc.policies[0].permission[0].constraint;
    let right = |c: &ConstraintNode| match c {
        ConstraintNode::Atomic(a) => a.right.clone(),
        _ => panic!("not atomic"),
    };
    assert_eq!(
        right(&cs[0]),
        RightOperand::Values(vec![Literal::Num(10.into())])
    );
    assert_eq!(
        right(&cs[1]),
        RightOperand::Values(vec![Literal::Bool(true)])
    );
}

// R6
#[test]
fn adding_a_policy_before_the_original_hoists_the_context_once() {
    let mut doc = rn(
        json!({"@context":"http://www.w3.org/ns/odrl.jsonld","@type":"Set","uid":"urn:p","permission":[{"action":"use"}]}),
    );
    add_policy(&mut doc, 0);
    let w = write_jsonld(&doc);
    assert_eq!(w["@context"], json!("http://www.w3.org/ns/odrl.jsonld"));
    for p in w["@graph"].as_array().unwrap() {
        assert!(p.get("@context").is_none(), "{w}");
    }
    assert_eq!(w["@graph"][1]["uid"], json!("urn:p"));
}

// R5
#[test]
fn switching_between_single_value_operators_on_a_multi_valued_constraint_is_allowed() {
    let mut doc = rn(
        json!({"@type":"Set","permission":[{"action":"use","constraint":[
        {"leftOperand":"count","operator":"eq","rightOperand":[1,2]}]}]}),
    );
    let p = "policy[0].permission[0].constraint[0]";
    let expect = id_of(&doc, p);
    doc.apply(
        &EditEvent::SetChoice {
            slot: format!("{p}#operator").parse().unwrap(),
            expect,
            value: "neq".into(),
        },
        &rules(),
    )
    .unwrap();
    // the public check the editor uses agrees with the reducer
    assert!(rules().operator_switch_allowed("eq", "neq", 2));
    assert!(rules().operator_switch_allowed("isAnyOf", "isAllOf", 3));
    assert!(rules().operator_switch_allowed("isAnyOf", "eq", 1));
    assert!(!rules().operator_switch_allowed("isAnyOf", "eq", 3));
    assert!(!rules().operator_switch_allowed("odrl:isAnyOf", "odrl:eq", 3));
}
