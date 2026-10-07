//! Regression tests for the findings of the third prose-core audit round.
mod common;
use common::*;
use prose_core::edit::*;
use serde_json::{Value, json};

fn rn(v: Value) -> EditDoc {
    read_model_value(&v).unwrap()
}

fn right(doc: &EditDoc) -> Value {
    write_jsonld(doc)["permission"][0]["constraint"][0]["rightOperand"].clone()
}

const RIGHT: &str = "policy[0].permission[0].constraint[0]#rightOperand[0]";

fn count_policy(right: Value) -> Value {
    json!({"@type":"Set","permission":[{"action":"use","constraint":[
        {"leftOperand":"count","operator":"lteq","rightOperand":right}]}]})
}

// D3
#[test]
fn editing_a_string_valued_typed_literal_keeps_its_string_form() {
    for (orig, text) in [
        (json!({"@value":"10","@type":"xsd:integer"}), "11"),
        (json!({"@value":"true","@type":"xsd:boolean"}), "false"),
    ] {
        let mut doc = rn(count_policy(orig.clone()));
        set_text(&mut doc, RIGHT, text).unwrap();
        let written = write_jsonld(&doc);
        assert_eq!(
            right(&doc),
            json!({"@value":text,"@type":orig["@type"]}),
            "{written}"
        );
        assert_eq!(
            read_model_value(&written).unwrap().normalized(),
            doc.normalized()
        );
    }
}

// D5
#[test]
fn editing_a_native_typed_literal_to_text_rereads_as_the_model() {
    let mut doc = rn(count_policy(json!({"@value":10,"@type":"xsd:integer"})));
    set_text(&mut doc, RIGHT, "abc").unwrap();
    let written = write_jsonld(&doc);
    assert_eq!(
        read_model_value(&written).unwrap().normalized(),
        doc.normalized(),
        "{written}"
    );
}

// D3 / D5 guard: a native value edited to a number stays native and typed.
#[test]
fn editing_a_native_typed_literal_to_a_number_stays_native() {
    let mut doc = rn(count_policy(json!({"@value":10,"@type":"xsd:integer"})));
    set_text(&mut doc, RIGHT, "11").unwrap();
    assert_eq!(right(&doc), json!({"@value":11,"@type":"xsd:integer"}));
}

// D6
#[test]
fn moving_a_policy_across_graph_wrappers_survives_the_write() {
    let p = |id: &str| json!({"@type":"Set","uid":id,"permission":[{"action":"use"}]});
    let v = json!([
        {"@context":{"ex":"A"},"@graph":[p("urn:a"),p("urn:c")]},
        {"@context":{"ex":"B"},"@graph":[p("urn:b")]}
    ]);
    let mut doc = rn(v);
    let list = ListPath::policies();
    let expect = item_id(&doc, &list, 2);
    let r = doc.apply(
        &EditEvent::Move {
            list,
            from: 2,
            to: 1,
            expect,
        },
        &EditRules::default(),
    );
    r.unwrap();
    {
        let written = write_jsonld(&doc);
        let again = read_model_value(&written).unwrap();
        let uids = |d: &EditDoc| -> Vec<Option<String>> {
            d.policies.iter().map(|p| p.uid.clone()).collect()
        };
        assert_eq!(uids(&again), uids(&doc), "{written}");
        // each policy is still under the context it was read under
        let ctx: Vec<&str> = written
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w["@context"]["ex"].as_str().unwrap())
            .collect();
        assert_eq!(ctx, ["A", "B", "A"], "{written}");
    }
}
