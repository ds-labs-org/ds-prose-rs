//! A policy typed only as ODRL's base class `Policy`, or not typed at all,
//! reads as a Set (#46). ODRL IM 2.1.1: a Policy with no subclass is a Set.
//! A more specific subclass written next to `Policy` still wins.
mod corpus;
use corpus::*;
use prose_core::validate::*;
use prose_core::{Policy, PolicyKind, render, render_with};
use serde_json::{Value, json};

const ODRL: &str = "http://www.w3.org/ns/odrl.jsonld";

fn typed(t: Option<Value>) -> Value {
    let mut p = json!({
        "@context": ODRL,
        "uid": "urn:p",
        "assigner": "urn:alice",
        "assignee": "urn:bob",
        "permission": [{"target": "urn:a", "action": "use"}]
    });
    if let Some(t) = t {
        p["@type"] = t;
    }
    p
}

fn read(doc: &Value) -> Policy {
    render(&doc.to_string()).unwrap().policies.remove(0)
}

/// Each way of writing the base class alone, and no type at all.
fn base_class_only() -> Vec<(&'static str, Value)> {
    vec![
        ("bare", typed(Some(json!("Policy")))),
        ("prefixed", typed(Some(json!("odrl:Policy")))),
        (
            "iri",
            typed(Some(json!("http://www.w3.org/ns/odrl/2/Policy"))),
        ),
        ("array", typed(Some(json!(["Policy"])))),
        ("prefixed array", typed(Some(json!(["odrl:Policy"])))),
        ("untyped", typed(None)),
    ]
}

#[test]
fn base_class_alone_reads_as_a_set() {
    let set = read(&typed(Some(json!("Set"))));
    for (label, doc) in base_class_only() {
        let p = read(&doc);
        assert_eq!(p.kind, PolicyKind::Set, "{label}");
        assert_eq!(p.heading, set.heading, "{label}");
        assert_eq!(p.intro, set.intro, "{label}");
        assert_eq!(
            p.intro, "A set of rules between urn:alice (assigner) and urn:bob (assignee).",
            "{label}"
        );
        assert!(!p.intro.contains("of type"), "{label}: {}", p.intro);
    }
}

#[test]
fn base_class_alone_reads_as_a_set_under_a_profile() {
    let w = wording_of(&["odrl22-excerpt.json"]);
    for (label, doc) in base_class_only() {
        let reading = render_with(&doc.to_string(), &w).unwrap();
        let p = &reading.document.policies[0];
        assert_eq!(p.heading, "Policy set urn:p", "{label}");
        assert!(
            p.intro.starts_with("A set of rules "),
            "{label}: {}",
            p.intro
        );
    }
}

#[test]
fn a_subclass_beside_the_base_class_wins() {
    for (t, kind, intro) in [
        (json!(["Policy", "Set"]), PolicyKind::Set, "A set of rules"),
        (
            json!(["odrl:Policy", "Offer"]),
            PolicyKind::Offer,
            "An offer",
        ),
        (
            json!(["Agreement", "http://www.w3.org/ns/odrl/2/Policy"]),
            PolicyKind::Agreement,
            "An agreement",
        ),
        (
            json!(["Policy", "Ticket"]),
            PolicyKind::Other("Ticket".into()),
            "A policy of type Ticket",
        ),
    ] {
        let p = read(&typed(Some(t.clone())));
        assert_eq!(p.kind, kind, "{t}");
        assert!(p.intro.starts_with(intro), "{t}: {}", p.intro);
    }
}

#[test]
fn the_validator_treats_the_base_class_as_it_treats_set() {
    let set = typed(Some(json!("Set")));
    let w = wording_of(&["odrl22-excerpt.json"]);
    for opts in [Options::advisory(), Options::strict()] {
        let want: Vec<String> = validate_json(&set.to_string(), &w, &opts)
            .findings
            .iter()
            .map(|f| f.rule.to_string())
            .collect();
        for (label, doc) in base_class_only() {
            let got: Vec<String> = validate_json(&doc.to_string(), &w, &opts)
                .findings
                .iter()
                .map(|f| f.rule.to_string())
                .collect();
            assert_eq!(got, want, "{label}");
        }
    }
}
