//! The term rules read each policy with the prefixes its own `@context`
//! scope binds (its own, then those of the object whose `@graph` holds it),
//! as the reader does; a sibling's context is not in scope. Two policies of
//! one `@graph` may bind the same prefix to different namespaces.
mod common;
mod corpus;
use corpus::*;
use prose_core::edit::*;
use prose_core::validate::*;
use serde_json::{Value, json};

const ODRL: &str = "http://www.w3.org/ns/odrl.jsonld";
const RESEARCH_NS: &str = "https://example.org/odrl/profile/research/";
const RESEARCH: &str = "https://example.org/odrl/profile/research";
const OTHER_NS: &str = "https://other.example/vocab/";

fn policy(uid: &str, ex: &str, profile: bool) -> Value {
    let mut p = json!({
        "@context": [ODRL, {"ex": ex}],
        "@type": "Set",
        "uid": uid,
        "permission": [{"target": "urn:a", "action": "ex:train"}]
    });
    if profile {
        p["profile"] = json!(RESEARCH);
    }
    p
}

fn run(doc: Value) -> Vec<(String, String, Severity)> {
    validate_json(
        &doc.to_string(),
        &wording_of(&["research.jsonld"]),
        &Options::strict(),
    )
    .findings
    .iter()
    .map(|f| (f.rule.to_string(), target_str(&f.target), f.severity))
    .collect()
}

#[test]
fn a_prefix_bound_differently_in_two_graph_policies_is_read_per_policy() {
    let got = run(json!({"@graph": [
        policy("urn:p1", OTHER_NS, false),
        policy("urn:p2", RESEARCH_NS, true),
    ]}));
    assert_eq!(
        got,
        [(
            "profile.unknown-term".to_string(),
            "policy[0].permission[0].action[0]#name".to_string(),
            Severity::Error
        )]
    );
}

#[test]
fn the_order_of_the_policies_does_not_matter() {
    let got = run(json!({"@graph": [
        policy("urn:p1", RESEARCH_NS, true),
        policy("urn:p2", OTHER_NS, false),
    ]}));
    assert_eq!(
        got,
        [(
            "profile.unknown-term".to_string(),
            "policy[1].permission[0].action[0]#name".to_string(),
            Severity::Error
        )]
    );
}

#[test]
fn a_graph_policy_overrides_the_wrapper_context() {
    let mut a = policy("urn:p1", OTHER_NS, false);
    a["@context"] = json!(ODRL);
    let got = run(json!({
        "@context": {"ex": OTHER_NS},
        "@graph": [a, policy("urn:p2", RESEARCH_NS, true)]
    }));
    assert_eq!(
        got.iter().map(|g| g.1.as_str()).collect::<Vec<_>>(),
        ["policy[0].permission[0].action[0]#name"]
    );
}
