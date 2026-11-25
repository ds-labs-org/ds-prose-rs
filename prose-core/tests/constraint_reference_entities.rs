//! A target's or a party's refinement may refer to a constraint the
//! document does not declare. The refinement reads inside the rule's
//! sentence, and the rule carries the IRI, so a host can link it there too.
use prose_core::render;

fn policy(body: &str) -> String {
    format!(
        r#"{{"@context": "http://www.w3.org/ns/odrl.jsonld", "@type": "Set", "uid": "urn:p", {body}}}"#
    )
}

#[test]
fn a_target_refinement_reference_is_carried_by_the_rule() {
    let doc = render(&policy(
        r#""permission": [{"action": "use",
            "target": {"@id": "urn:a", "refinement": "http://example.com/c9"}}]"#,
    ))
    .unwrap();
    let rule = &doc.policies[0].rules[0];
    assert_eq!(
        rule.sentence,
        "Anyone may use urn:a where the constraint defined at http://example.com/c9 holds."
    );
    assert_eq!(rule.references, ["http://example.com/c9"]);
}

#[test]
fn a_party_refinement_reference_is_carried_by_the_rule() {
    let doc = render(&policy(
        r#""permission": [{"action": "use", "target": "urn:a",
            "assignee": {"@id": "urn:bob", "refinement": [{"and": [{"@id": "http://example.com/c8"}]}]}}]"#,
    ))
    .unwrap();
    let rule = &doc.policies[0].rules[0];
    assert!(
        rule.sentence
            .contains("the constraint defined at http://example.com/c8 holds"),
        "{}",
        rule.sentence
    );
    assert_eq!(rule.references, ["http://example.com/c8"]);
}

/// A rule that inherits its policy's target inherits that target's
/// references too; a rule with a target of its own does not.
#[test]
fn an_inherited_target_brings_its_references() {
    let doc = render(&policy(
        r#""target": {"@id": "urn:a", "refinement": {"@id": "http://example.com/c7"}},
           "permission": [{"action": "use"}, {"action": "read", "target": "urn:b"}]"#,
    ))
    .unwrap();
    let rules = &doc.policies[0].rules;
    assert_eq!(rules[0].references, ["http://example.com/c7"]);
    assert!(rules[1].references.is_empty());
}

/// Declared constraints are read in place and carry no reference; nor
/// do a rule's own conditions, which carry theirs on the condition.
#[test]
fn only_undeclared_entity_refinements_are_carried() {
    let doc = render(
        r#"{"@context": "http://www.w3.org/ns/odrl.jsonld", "@graph": [
            {"@type": "Set", "uid": "urn:p", "permission": [{"action": "use",
              "target": {"@id": "urn:a", "refinement": "urn:c:declared"},
              "constraint": "http://example.com/c6"}]},
            {"@type": "Constraint", "@id": "urn:c:declared", "leftOperand": "count",
             "operator": "lteq", "rightOperand": 1}
        ]}"#,
    )
    .unwrap();
    let rule = &doc.policies[0].rules[0];
    assert!(rule.references.is_empty(), "{:?}", rule.references);
    assert_eq!(
        rule.conditions[0].reference.as_deref(),
        Some("http://example.com/c6")
    );
}
