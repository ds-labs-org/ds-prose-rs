//! The term rules read each policy with the prefixes its own `@context`
//! scope binds (its own, then those of the object whose `@graph` holds it),
//! as the reader does; a sibling's context is not in scope. Two policies of
//! one `@graph` may bind the same prefix to different namespaces.
mod common;
mod corpus;
use common::*;
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

fn strict_of(doc: &EditDoc) -> Vec<(String, String, Severity)> {
    validate(doc, &wording_of(&["research.jsonld"]), &Options::strict())
        .findings
        .iter()
        .map(|f| (f.rule.to_string(), target_str(&f.target), f.severity))
        .collect()
}

#[test]
fn a_moved_graph_policy_keeps_its_own_scope() {
    let mut doc = read_model_value(&json!({"@graph": [
        policy("urn:p1", OTHER_NS, false),
        policy("urn:p2", RESEARCH_NS, true),
    ]}))
    .unwrap();
    let expect = doc.policies[1].id;
    doc.apply(
        &EditEvent::Move {
            list: ListPath::policies(),
            from: 1,
            to: 0,
            expect,
        },
        &EditRules::default(),
    )
    .unwrap();
    assert_eq!(
        strict_of(&doc),
        [(
            "profile.unknown-term".to_string(),
            "policy[1].permission[0].action[0]#name".to_string(),
            Severity::Error
        )]
    );
}

#[test]
fn a_policy_added_beside_a_single_one_reads_the_context_it_is_written_under() {
    // Written out, the single policy's context moves up to the @graph
    // wrapper, so it binds `ex` for the new policy too.
    let mut doc = read_model_value(&policy("urn:p1", RESEARCH_NS, true)).unwrap();
    let rules = EditRules::default();
    doc.apply(
        &add_ev(&doc, ListPath::policies(), 1, NewItem::Default),
        &rules,
    )
    .unwrap();
    doc.apply(
        &add_ev(&doc, lp("policy[1]@permission"), 0, NewItem::Default),
        &rules,
    )
    .unwrap();
    doc.policies[1].profile = vec![RESEARCH.to_string()];
    set_text(
        &mut doc,
        "policy[1].permission[0].action[0]#name",
        "ex:train",
    )
    .unwrap();
    let written = write_jsonld(&doc);
    assert_eq!(
        written["@context"][1]["ex"],
        json!(RESEARCH_NS),
        "{written}"
    );
    let terms: Vec<_> = strict_of(&doc)
        .into_iter()
        .filter(|f| f.0.starts_with("profile."))
        .collect();
    assert_eq!(terms, []);
}

#[test]
fn inheritance_links_read_each_policy_with_its_own_scope() {
    // With one merged map p1 -> ex:q and p2 -> ex:p close a cycle; read
    // with their own bindings they name policies elsewhere.
    let node = |uid: &str, parent: &str, ex: &str| {
        json!({
            "@context": [ODRL, {"ex": ex}],
            "@type": "Set",
            "uid": uid,
            "inheritFrom": parent,
            "permission": [{"target": "urn:a", "action": "use"}]
        })
    };
    let cycle = |doc: Value| {
        let o = Options::strict().structural(true);
        validate_json(&doc.to_string(), &wording_of(&["research.jsonld"]), &o)
            .findings
            .iter()
            .filter(|f| f.rule.to_string() == "odrl.inheritance-cycle")
            .count()
    };
    assert_eq!(
        cycle(
            json!({"@graph": [node("ex:p", "ex:q", OTHER_NS), node("ex:q", "ex:p", RESEARCH_NS)]})
        ),
        0
    );
    assert_eq!(
        cycle(json!({"@graph": [node("ex:p", "ex:q", OTHER_NS), node("ex:q", "ex:p", OTHER_NS)]})),
        2
    );
    // The first policy's link is read with its own binding, not the last
    // policy's: urn:p1 -> ex:q names <other:q>, which is not urn:p2's uid.
    assert_eq!(
        cycle(
            json!({"@graph": [node("urn:p1", "ex:q", OTHER_NS), node("ex:q", "urn:p1", RESEARCH_NS)]})
        ),
        0
    );
}

#[test]
fn the_same_policy_under_two_wrappers_reads_each_wrapper_context() {
    let mut p = policy("urn:p", RESEARCH_NS, true);
    p.as_object_mut().unwrap().remove("@context");
    let got = run(json!([
        {"@context": [ODRL, {"ex": OTHER_NS}], "@graph": [p.clone()]},
        {"@context": [ODRL, {"ex": RESEARCH_NS}], "@graph": [p]}
    ]));
    assert_eq!(
        got.iter().map(|g| g.1.as_str()).collect::<Vec<_>>(),
        ["policy[0].permission[0].action[0]#name"]
    );
}

#[test]
fn a_policy_added_to_a_graph_is_read_with_every_context_merged() {
    let mut doc = read_model_value(&json!({"@graph": [
        policy("urn:p1", RESEARCH_NS, true),
        policy("urn:p2", OTHER_NS, false),
    ]}))
    .unwrap();
    let rules = EditRules::default();
    doc.apply(
        &add_ev(&doc, ListPath::policies(), 2, NewItem::Default),
        &rules,
    )
    .unwrap();
    doc.apply(
        &add_ev(&doc, lp("policy[2]@permission"), 0, NewItem::Default),
        &rules,
    )
    .unwrap();
    doc.policies[2].profile = vec![RESEARCH.to_string()];
    set_text(
        &mut doc,
        "policy[2].permission[0].action[0]#name",
        "ex:train",
    )
    .unwrap();
    let terms: Vec<_> = strict_of(&doc)
        .into_iter()
        .filter(|f| f.0.starts_with("profile."))
        .map(|f| f.1)
        .collect();
    assert_eq!(terms, ["policy[1].permission[0].action[0]#name"]);
}

#[test]
fn a_node_already_matched_is_not_matched_again() {
    let mut p = policy("urn:p", RESEARCH_NS, true);
    p.as_object_mut().unwrap().remove("@context");
    let mut doc = read_model_value(&json!([
        {"@context": [ODRL, {"ex": OTHER_NS}], "@graph": [p.clone()]},
        {"@context": [ODRL, {"ex": RESEARCH_NS}], "@graph": [p]},
        policy("urn:q", RESEARCH_NS, true)
    ]))
    .unwrap();
    let expect = doc.policies[2].id;
    doc.apply(
        &EditEvent::Move {
            list: ListPath::policies(),
            from: 2,
            to: 1,
            expect,
        },
        &EditRules::default(),
    )
    .unwrap();
    assert_eq!(
        strict_of(&doc)
            .iter()
            .map(|g| g.1.as_str())
            .collect::<Vec<_>>(),
        ["policy[0].permission[0].action[0]#name"]
    );
}
