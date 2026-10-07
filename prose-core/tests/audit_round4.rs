//! Regression tests for the findings of the fourth prose-core audit round.
mod common;
use common::*;
use prose_core::edit::*;
use serde_json::{Value, json};

fn rn(v: Value) -> EditDoc {
    read_model_value(&v).unwrap()
}

fn count_policy(right: Value) -> Value {
    json!({"@type":"Set","permission":[{"action":"use","constraint":[
        {"leftOperand":"count","operator":"isAnyOf","rightOperand":right}]}]})
}

fn right(doc: &EditDoc) -> Value {
    write_jsonld(doc)["permission"][0]["constraint"][0]["rightOperand"].clone()
}

const LIST: &str = "policy[0].permission[0].constraint[0]@rightOperand";
const SLOT: &str = "policy[0].permission[0].constraint[0]#rightOperand[0]";

fn remove_value(doc: &mut EditDoc, index: usize) {
    let list = lp(LIST);
    let expect = item_id(doc, &list, index);
    doc.apply(
        &EditEvent::Remove {
            list,
            index,
            expect,
        },
        &EditRules::default(),
    )
    .unwrap();
}

fn assert_rereads(doc: &EditDoc) {
    let written = write_jsonld(doc);
    assert_eq!(
        read_model_value(&written).unwrap().normalized(),
        doc.normalized(),
        "{written}"
    );
}

// E2 (a)
#[test]
fn removing_a_typed_value_does_not_retype_the_survivor() {
    let mut doc = rn(count_policy(
        json!([{"@value":10,"@type":"xsd:integer"}, "5"]),
    ));
    remove_value(&mut doc, 0);
    assert_eq!(right(&doc), json!("5"));
    assert_rereads(&doc);
}

// E2 (b)
#[test]
fn removing_a_value_keeps_the_wrapper_of_the_one_after_it() {
    for wrapped in [
        json!({"@value":10,"@type":"xsd:integer"}),
        json!({"@value":"ten","@language":"en"}),
    ] {
        let mut doc = rn(count_policy(json!(["abc", wrapped.clone()])));
        remove_value(&mut doc, 0);
        assert_eq!(right(&doc), wrapped);
        assert_rereads(&doc);
    }
}

// E5
#[test]
fn text_the_model_keeps_as_a_string_is_written_as_a_string() {
    for (orig, text) in [
        (
            json!({"@value":10,"@type":"xsd:decimal"}),
            "0.1234567890123456",
        ),
        (
            json!({"@value":10,"@type":"xsd:decimal"}),
            "0.30000000000000004",
        ),
        (json!({"@value":10,"@type":"xsd:boolean"}), "true"),
        (json!({"@value":true,"@type":"xsd:integer"}), "11"),
    ] {
        let mut doc = rn(count_policy(orig.clone()));
        set_text(&mut doc, SLOT, text).unwrap();
        assert_eq!(right(&doc), json!(text), "{orig} -> {text}");
        assert_rereads(&doc);
    }
}

// E6
#[test]
fn editing_a_language_tagged_literal_keeps_its_language() {
    let mut doc = rn(count_policy(json!({"@value":"ten","@language":"en"})));
    set_text(&mut doc, SLOT, "eleven").unwrap();
    assert_eq!(right(&doc), json!({"@value":"eleven","@language":"en"}));
    assert_rereads(&doc);
}

#[test]
fn editing_a_native_typed_literal_keeps_its_other_keys() {
    let mut doc = rn(count_policy(
        json!({"@value":10,"@type":"xsd:integer","@index":"i"}),
    ));
    set_text(&mut doc, SLOT, "11").unwrap();
    assert_eq!(
        right(&doc),
        json!({"@value":11,"@type":"xsd:integer","@index":"i"})
    );
}

// E5, broadly: every original wrapper, every kind of edited text.
#[test]
fn any_edit_of_a_wrapped_value_rereads_as_the_model() {
    let types = [
        "xsd:integer",
        "xsd:int",
        "xsd:long",
        "xsd:short",
        "xsd:byte",
        "xsd:decimal",
        "xsd:double",
        "xsd:float",
        "xsd:nonNegativeInteger",
        "xsd:positiveInteger",
        "xsd:unsignedInt",
        "xsd:boolean",
        "xsd:string",
        "xsd:date",
    ];
    let originals = |t: &str| {
        vec![
            json!({"@value":10,"@type":t}),
            json!({"@value":true,"@type":t}),
            json!({"@value":"10","@type":t}),
            json!({"@value":"ten","@language":"en"}),
        ]
    };
    let texts = [
        "11",
        "0.5",
        "0.1234567890123456",
        "0.30000000000000004",
        "true",
        "false",
        "abc",
        "",
        "-0",
        "1e3",
        "007",
        "12345678901234567890123",
    ];
    for t in types {
        for orig in originals(t) {
            for text in texts {
                let mut doc = rn(count_policy(orig.clone()));
                if set_text(&mut doc, SLOT, text).is_err() {
                    continue;
                }
                assert_rereads_ctx(&doc, &format!("{orig} -> {text:?}"));
            }
        }
    }
}

fn assert_rereads_ctx(doc: &EditDoc, what: &str) {
    let written = write_jsonld(doc);
    assert_eq!(
        read_model_value(&written).unwrap().normalized(),
        doc.normalized(),
        "{what}: {written}"
    );
}

fn pol(id: &str) -> Value {
    json!({"@type":"Set","uid":id,"permission":[{"action":"use"}]})
}

fn move_policy(doc: &mut EditDoc, from: usize, to: usize) {
    let list = ListPath::policies();
    let expect = item_id(doc, &list, from);
    doc.apply(
        &EditEvent::Move {
            list,
            from,
            to,
            expect,
        },
        &EditRules::default(),
    )
    .unwrap();
}

fn with_empty_wrapper() -> Value {
    json!([
        {"@context":{"ex":"A"},"@graph":[pol("urn:1"),pol("urn:3")]},
        {"@context":"urn:E","@id":"urn:g:empty","@graph":[]},
        {"@context":{"ex":"B"},"@graph":[pol("urn:2")]}
    ])
}

fn ids(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|w| w["@id"].as_str().unwrap_or("-").to_string())
        .collect()
}

// E3
#[test]
fn moving_a_policy_across_wrappers_keeps_an_empty_wrapper() {
    let mut doc = rn(with_empty_wrapper());
    move_policy(&mut doc, 2, 1);
    let written = write_jsonld(&doc);
    assert!(
        ids(&written).contains(&"urn:g:empty".to_string()),
        "{written}"
    );
    let empty = written
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["@id"] == "urn:g:empty")
        .unwrap();
    assert_eq!(empty, &with_empty_wrapper()[1], "with its keys");
    let again = read_model_value(&written).unwrap();
    assert_eq!(again.normalized(), doc.normalized());
}

// E3, related
#[test]
fn an_edit_leaves_an_empty_wrapper_where_it_was() {
    let mut doc = rn(with_empty_wrapper());
    set_text(&mut doc, "policy[0]#uid", "urn:1x").unwrap();
    let written = write_jsonld(&doc);
    assert_eq!(ids(&written), ["-", "urn:g:empty", "-"], "{written}");
}

// E4
#[test]
fn nested_wrappers_inside_a_graph_object_keep_their_contexts() {
    let v = json!({"@context":"urn:top","@graph":[
        {"@context":{"ex":"A"},"@graph":[pol("urn:1"),pol("urn:2")]},
        {"@context":{"ex":"B"},"@graph":[pol("urn:3")]}
    ]});
    let mut doc = rn(v.clone());
    assert_eq!(write_jsonld(&doc), v);
    set_text(&mut doc, "policy[0]#uid", "urn:1x").unwrap();
    let w = write_jsonld(&doc);
    let mut expect = v.clone();
    expect["@graph"][0]["@graph"][0]["uid"] = json!("urn:1x");
    assert_eq!(w, expect);
    // and a move across wrappers still reads back in the model's order
    let mut doc = rn(v);
    move_policy(&mut doc, 2, 0);
    let w = write_jsonld(&doc);
    let again = read_model_value(&w).unwrap();
    assert_eq!(again.normalized(), doc.normalized(), "{w}");
    assert_eq!(w["@graph"][0]["@context"], json!({"ex":"B"}), "{w}");
}
