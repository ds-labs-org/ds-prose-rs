//! The pre-0.7 Tractus-X action shape names the action under `odrl:type`
//! instead of `@id` or `rdf:value`. It used to render "perform an unspecified
//! action" with no warning; an action object that names nothing must never
//! vanish silently either.
use prose_core::{Document, render_value};
use serde_json::{Value, json};

fn render(v: Value) -> Document {
    render_value(&v).expect("a policy")
}

fn sentence(doc: &Document) -> String {
    doc.policies
        .iter()
        .flat_map(|p| p.rules.iter().map(|r| r.sentence.clone()))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn policy(action: Value) -> Value {
    json!({"@type": "Set", "permission": [{"action": action, "target": "urn:a"}]})
}

#[test]
fn an_odrl_type_action_is_read() {
    let doc = render(policy(json!({"odrl:type": "odrl:use"})));
    assert!(
        !sentence(&doc).contains("unspecified action"),
        "{}",
        sentence(&doc)
    );
    assert!(sentence(&doc).contains("use"), "{}", sentence(&doc));
}

#[test]
fn a_bare_type_action_is_read() {
    let doc = render(policy(json!({"type": "use"})));
    assert!(sentence(&doc).contains("use"), "{}", sentence(&doc));
}

#[test]
fn a_type_given_as_an_id_object_is_read() {
    let doc = render(policy(json!({"odrl:type": {"@id": "odrl:distribute"}})));
    assert!(sentence(&doc).contains("distribute"), "{}", sentence(&doc));
}

#[test]
fn the_type_action_keeps_its_refinement() {
    let doc = render(policy(json!({
        "odrl:type": "odrl:use",
        "odrl:refinement": [{"leftOperand": "purpose", "operator": "eq", "rightOperand": "research"}]
    })));
    assert!(sentence(&doc).contains("use"), "{}", sentence(&doc));
    assert!(!doc.policies[0].rules[0].refinements.is_empty());
}

#[test]
fn an_id_still_wins_over_a_type() {
    let doc = render(policy(json!({"@id": "odrl:read", "odrl:type": "odrl:use"})));
    assert!(sentence(&doc).contains("read"), "{}", sentence(&doc));
}

#[test]
fn an_action_object_that_names_nothing_is_reported() {
    let doc = render(policy(json!({"odrl:refinement": []})));
    assert!(
        doc.warnings.iter().any(|w| w.contains("action")),
        "{:?}",
        doc.warnings
    );
}
