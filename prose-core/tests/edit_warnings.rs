//! The "no permission, prohibition or obligation" note follows the document
//! as it is edited, as a fresh read of the written JSON-LD would give it.
mod common;
use common::*;
use prose_core::edit::*;

const NO_RULES: &str = "a policy has no permission, prohibition or obligation";

fn has_note(doc: &EditDoc) -> bool {
    doc.warnings.iter().any(|w| w == NO_RULES)
}

fn reread_has_note(doc: &EditDoc) -> bool {
    has_note(&read_model_value(&write_jsonld(doc)).unwrap())
}

fn read(json: &str) -> EditDoc {
    read_model(json).unwrap()
}

fn add_permission(doc: &mut EditDoc, policy: usize) {
    let list = lp(&format!("policy[{policy}]@permission"));
    let n = list_len(doc, &list);
    doc.apply(
        &add_ev(doc, list, n, NewItem::Default),
        &EditRules::default(),
    )
    .unwrap();
}

#[test]
fn the_note_goes_when_a_rule_is_added_and_returns_when_it_is_removed() {
    let mut doc = read(r#"{"@context":"http://www.w3.org/ns/odrl.jsonld","@type":"Set"}"#);
    assert!(has_note(&doc));
    add_permission(&mut doc, 0);
    assert!(!has_note(&doc));
    assert_eq!(has_note(&doc), reread_has_note(&doc));
    let list = lp("policy[0]@permission");
    let expect = item_id(&doc, &list, 0);
    doc.apply(
        &EditEvent::Remove {
            list,
            index: 0,
            expect,
        },
        &EditRules::default(),
    )
    .unwrap();
    assert!(has_note(&doc));
    assert_eq!(has_note(&doc), reread_has_note(&doc));
}

#[test]
fn a_child_keeps_the_note_until_its_parent_gets_a_rule() {
    let mut doc = read(
        r#"{"@graph":[
            {"@type":"Set","uid":"urn:p:a"},
            {"@type":"Set","uid":"urn:p:b","inheritFrom":"urn:p:a"}
        ]}"#,
    );
    assert!(has_note(&doc));
    add_permission(&mut doc, 1);
    assert_eq!(has_note(&doc), reread_has_note(&doc));
    let mut doc = read(
        r#"{"@graph":[
            {"@type":"Set","uid":"urn:p:a"},
            {"@type":"Set","uid":"urn:p:b","inheritFrom":"urn:p:a"}
        ]}"#,
    );
    // b has no rules of its own and inherits none; a has none either.
    assert!(has_note(&doc));
    add_permission(&mut doc, 0);
    // a has a rule now, b inherits it: no policy is left without rules.
    assert!(!has_note(&doc));
    assert_eq!(has_note(&doc), reread_has_note(&doc));
}

#[test]
fn other_reader_notes_survive_edits() {
    let mut doc = read(
        r#"{"@graph":[
            {"@type":"Set","uid":"urn:p:a"},
            {"@id":"urn:not-a-policy","foo":"bar"}
        ]}"#,
    );
    let other: Vec<String> = doc
        .warnings
        .iter()
        .filter(|w| *w != NO_RULES)
        .cloned()
        .collect();
    assert!(!other.is_empty(), "{:?}", doc.warnings);
    add_permission(&mut doc, 0);
    assert!(!has_note(&doc));
    for w in &other {
        assert!(doc.warnings.contains(w), "{w} lost: {:?}", doc.warnings);
    }
}
