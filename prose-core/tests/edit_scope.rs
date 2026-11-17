//! Limits of `max: 0` as the scope of an incoming document: what is out of
//! it, and the document without it.
use prose_core::edit::{EditRules, Limit, out_of_scope, read_model, strip_out_of_scope};
use serde_json::{Value, json};

fn rules(f: impl FnOnce(&mut EditRules)) -> EditRules {
    let mut r = EditRules::default();
    f(&mut r);
    r
}

fn no_parties() -> EditRules {
    rules(|r| {
        r.policy_assigner = Limit::NONE;
        r.policy_assignee = Limit::NONE;
        r.rule_assigner = Limit::NONE;
        r.rule_assignee = Limit::NONE;
    })
}

fn paths(v: &Value, r: &EditRules) -> Vec<String> {
    out_of_scope(v, r).into_iter().map(|o| o.path).collect()
}

fn stripped(mut v: Value, r: &EditRules) -> Value {
    strip_out_of_scope(&mut v, r);
    v
}

fn doc() -> Value {
    json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Set",
        "uid": "urn:p:1",
        "assigner": "urn:a",
        "assignee": "urn:b",
        "permission": [{
            "target": "urn:t", "action": "use",
            "assigner": "urn:a", "assignee": "urn:b",
            "duty": [{ "action": "inform", "assigner": "urn:a", "assignee": "urn:b" }]
        }]
    })
}

#[test]
fn nothing_is_out_of_scope_under_the_default_limits() {
    assert!(out_of_scope(&doc(), &EditRules::default()).is_empty());
    assert_eq!(stripped(doc(), &EditRules::default()), doc());
}

#[test]
fn parties_at_the_policy_rule_and_duty_level_are_found_with_their_paths() {
    let found = paths(&doc(), &no_parties());
    assert_eq!(
        found,
        [
            "assigner",
            "assignee",
            "permission[0].assigner",
            "permission[0].assignee",
            "permission[0].duty[0].assigner",
            "permission[0].duty[0].assignee",
        ]
    );
    let hits = out_of_scope(&doc(), &no_parties());
    assert_eq!(hits[0].limit, "policyAssigner");
    assert_eq!(hits[2].limit, "ruleAssigner");
    assert_eq!(hits[0].key, "assigner");
}

#[test]
fn strip_removes_them_and_keeps_everything_else_in_order() {
    let got = stripped(doc(), &no_parties());
    let want = json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Set",
        "uid": "urn:p:1",
        "permission": [{ "target": "urn:t", "action": "use", "duty": [{ "action": "inform" }] }]
    });
    assert_eq!(got, want);
    assert_eq!(
        serde_json::to_string(&got).unwrap(),
        serde_json::to_string(&want).unwrap(),
        "key order is kept"
    );
    assert!(out_of_scope(&got, &no_parties()).is_empty());
    assert!(read_model(&got.to_string()).is_ok());
}

#[test]
fn a_limit_applies_to_its_own_level_only() {
    let r = rules(|r| r.rule_assigner = Limit::NONE);
    assert_eq!(
        paths(&doc(), &r),
        ["permission[0].assigner", "permission[0].duty[0].assigner"]
    );
    let r = rules(|r| r.policy_assignee = Limit::NONE);
    assert_eq!(paths(&doc(), &r), ["assignee"]);
}

#[test]
fn prefixed_bare_and_full_iri_spellings_are_all_found() {
    let v = json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", { "o": "http://www.w3.org/ns/odrl/2/" }],
        "@type": "Set",
        "odrl:assigner": "urn:a",
        "http://www.w3.org/ns/odrl/2/assignee": "urn:b",
        "o:assigner": "urn:c",
        "permission": [{
            "action": "use",
            "odrl:assignee": "urn:b",
            "http://www.w3.org/ns/odrl/2/assigner": "urn:a"
        }]
    });
    let r = no_parties();
    assert_eq!(
        paths(&v, &r),
        [
            "odrl:assigner",
            "http://www.w3.org/ns/odrl/2/assignee",
            "o:assigner",
            "permission[0].odrl:assignee",
            "permission[0].http://www.w3.org/ns/odrl/2/assigner",
        ]
    );
    assert_eq!(
        stripped(v, &r),
        json!({
            "@context": ["http://www.w3.org/ns/odrl.jsonld", { "o": "http://www.w3.org/ns/odrl/2/" }],
            "@type": "Set",
            "permission": [{ "action": "use" }]
        })
    );
}

#[test]
fn another_vocabularys_assigner_is_not_odrls() {
    let v = json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", { "ex": "https://example.org/" }],
        "@type": "Set",
        "ex:assigner": "urn:a",
        "permission": [{ "action": "use", "ex:assignee": "urn:b" }]
    });
    assert!(out_of_scope(&v, &no_parties()).is_empty());
}

#[test]
fn other_lists_at_max_zero_are_covered() {
    let v = json!({
        "@type": "Set",
        "uid": "urn:p:1",
        "profile": "https://example.org/p",
        "inheritFrom": "urn:p:0",
        "target": "urn:t",
        "permission": [{
            "action": [{ "rdf:value": "use", "refinement": [{ "leftOperand": "x", "operator": "eq", "rightOperand": 1 }] }],
            "constraint": [{ "leftOperand": "y", "operator": "eq", "rightOperand": 2 }],
            "duty": [{ "action": "inform" }]
        }],
        "prohibition": [{ "action": "distribute", "remedy": [{ "action": "delete" }] }],
        "obligation": [{ "action": "inform", "consequence": [{ "action": "delete" }] }]
    });
    let case = |name: &str, f: fn(&mut EditRules), want: &[&str]| {
        let r = rules(f);
        assert_eq!(paths(&v, &r), want, "{name}");
        let s = stripped(v.clone(), &r);
        assert!(out_of_scope(&s, &r).is_empty(), "{name}: nothing left");
    };
    case("policyUid", |r| r.policy_uid = Limit::NONE, &["uid"]);
    case("profile", |r| r.profile = Limit::NONE, &["profile"]);
    case(
        "inheritFrom",
        |r| r.inherit_from = Limit::NONE,
        &["inheritFrom"],
    );
    case(
        "policyTarget",
        |r| r.policy_target = Limit::NONE,
        &["target"],
    );
    case(
        "prohibition",
        |r| r.prohibition = Limit::NONE,
        &["prohibition"],
    );
    case(
        "obligation",
        |r| r.obligation = Limit::NONE,
        &["obligation"],
    );
    case("duty", |r| r.duty = Limit::NONE, &["permission[0].duty"]);
    case(
        "remedy",
        |r| r.remedy = Limit::NONE,
        &["prohibition[0].remedy"],
    );
    case(
        "consequence",
        |r| r.consequence = Limit::NONE,
        &["obligation[0].consequence"],
    );
    case(
        "constraints",
        |r| r.constraints = Limit::NONE,
        &["permission[0].constraint"],
    );
    case(
        "actionRefinement",
        |r| r.action_refinement = Limit::NONE,
        &["permission[0].action[0].refinement"],
    );
}

#[test]
fn a_whole_permission_list_at_zero_is_dropped_with_what_it_holds() {
    let r = rules(|r| r.permission = Limit::NONE);
    let got = stripped(doc(), &r);
    assert!(got.get("permission").is_none());
    assert_eq!(got["assigner"], "urn:a");
}

#[test]
fn graph_and_array_documents_are_walked() {
    let g = json!({ "@graph": [doc(), doc()] });
    assert_eq!(paths(&g, &no_parties()).len(), 12);
    assert_eq!(paths(&g, &no_parties())[0], "@graph[0].assigner");
    let a = json!([doc()]);
    assert_eq!(paths(&a, &no_parties())[0], "[0].assigner");
    let s = stripped(g, &no_parties());
    assert!(out_of_scope(&s, &no_parties()).is_empty());
}

#[test]
fn a_min_above_zero_or_no_max_is_not_a_scope() {
    let r = rules(|r| {
        r.policy_assigner = Limit::exactly(1);
        r.policy_assignee = Limit::at_most(1);
    });
    assert!(out_of_scope(&doc(), &r).is_empty());
}
