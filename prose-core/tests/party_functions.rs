//! The party functions of the ODRL Common Vocabulary (Vocabulary 4.3) on a
//! rule or a duty name a party the sentence must not drop: "attribute the
//! asset to X", "compensate X", "accept tracking by X", or, with an action
//! they have no phrasing for, "(informed party: X)".
use prose_core::{Rule, render};
use serde_json::{Value, json};

fn read(policy: Value) -> prose_core::Document {
    render(&policy.to_string()).unwrap()
}

fn agreement(rules: Value) -> Value {
    json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Agreement",
        "uid": "urn:policy:1",
        "assigner": "urn:org:a",
        "assignee": "urn:person:b",
        "obligation": rules
    })
}

fn only_rule(doc: &prose_core::Document) -> &Rule {
    &doc.policies[0].rules[0]
}

/// The W3C Information Model's example of a duty with a consequence
/// (section 2.6.6, eg16712 in the catalog).
#[test]
fn w3c_attributed_and_tracking_parties_are_named() {
    let doc = read(json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Agreement",
        "uid": "http://example.com/policy:66",
        "profile": "http://example.com/odrl:profile:09",
        "permission": [{
            "target": "http://example.com/data:77",
            "assigner": "http://example.com/org:99",
            "assignee": "http://example.com/person:88",
            "action": "distribute",
            "duty": [{
                "action": "attribute",
                "attributedParty": "http://australia.gov.au/",
                "consequence": [{
                    "action": "acceptTracking",
                    "trackingParty": "http://example.com/dept:100"
                }]
            }]
        }]
    }));
    let duty = &only_rule(&doc).follow_ups[0].rules[0];
    assert_eq!(
        duty.sentence,
        "http://example.com/person:88 must attribute the asset to http://australia.gov.au/, as required by http://example.com/org:99."
    );
    let consequence = &duty.follow_ups[0].rules[0];
    assert_eq!(
        consequence.sentence,
        "http://example.com/person:88 must accept tracking by http://example.com/dept:100, as required by http://example.com/org:99."
    );
    assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
}

/// The W3C Information Model's example of an obligation with a
/// compensation consequence (section 2.7.1, eg169b in the catalog).
#[test]
fn w3c_compensated_party_is_the_object_of_compensate() {
    let doc = read(agreement(json!([{
        "action": "delete",
        "target": "http://example.com/document:XZY",
        "consequence": [{
            "action": [{
                "rdf:value": {"@id": "odrl:compensate"},
                "refinement": [{
                    "leftOperand": "payAmount", "operator": "eq",
                    "rightOperand": {"@value": "10.00", "@type": "xsd:decimal"},
                    "unit": "http://dbpedia.org/resource/Euro"
                }]
            }],
            "compensatedParty": "http://wwf.org"
        }]
    }])));
    let consequence = &only_rule(&doc).follow_ups[0].rules[0];
    assert_eq!(
        consequence.sentence,
        "urn:person:b must compensate http://wwf.org, as required by urn:org:a."
    );
    assert_eq!(
        consequence.refinements[0].children[0].text,
        "the payment amount is equal to 10.00 euro"
    );
    assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
}

#[test]
fn a_party_function_reads_with_its_action_and_the_target() {
    let cases = [
        (
            json!({"action": "attribute", "target": "urn:asset:1", "attributedParty": "urn:p"}),
            "urn:person:b must attribute urn:asset:1 to urn:p, as required by urn:org:a.",
        ),
        (
            json!({"action": "compensate", "target": "urn:asset:1", "compensatedParty": "urn:p"}),
            "urn:person:b must compensate urn:p for urn:asset:1, as required by urn:org:a.",
        ),
        (
            json!({"action": "inform", "target": "urn:asset:1", "informedParty": "urn:p"}),
            "urn:person:b must inform urn:p about urn:asset:1, as required by urn:org:a.",
        ),
        (
            json!({"action": "inform", "informedParty": "urn:p"}),
            "urn:person:b must inform urn:p, as required by urn:org:a.",
        ),
        (
            json!({"action": "obtainConsent", "consentingParty": "urn:p"}),
            "urn:person:b must obtain consent from urn:p, as required by urn:org:a.",
        ),
        (
            json!({"action": "acceptTracking", "target": "urn:asset:1", "trackingParty": "urn:p"}),
            "urn:person:b must accept tracking urn:asset:1 by urn:p, as required by urn:org:a.",
        ),
    ];
    for (rule, want) in cases {
        let doc = read(agreement(json!([rule])));
        assert_eq!(only_rule(&doc).sentence, want);
        assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
    }
}

#[test]
fn every_party_function_is_named_with_any_action() {
    let functions = [
        ("attributedParty", "attributed party"),
        ("attributingParty", "attributing party"),
        ("compensatedParty", "compensated party"),
        ("compensatingParty", "compensating party"),
        ("consentingParty", "consenting party"),
        ("consentedParty", "consented party"),
        ("contractingParty", "contracting party"),
        ("contractedParty", "contracted party"),
        ("informedParty", "informed party"),
        ("informingParty", "informing party"),
        ("trackingParty", "tracking party"),
        ("trackedParty", "tracked party"),
    ];
    for (key, label) in functions {
        let doc = read(json!({
            "@context": "http://www.w3.org/ns/odrl.jsonld",
            "@type": "Set",
            "uid": "urn:policy:1",
            "permission": [{"action": "use", "target": "urn:asset:1", key: "urn:p"}]
        }));
        assert_eq!(
            only_rule(&doc).sentence,
            format!("Anyone may use urn:asset:1 ({label}: urn:p)."),
            "{key}"
        );
        assert!(doc.warnings.is_empty(), "{key}: {:?}", doc.warnings);
    }
}

#[test]
fn several_functions_without_a_phrasing_share_one_parenthesis() {
    let doc = read(json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Agreement",
        "uid": "urn:policy:1",
        "assigner": "urn:org:a",
        "assignee": "urn:person:b",
        "permission": [{
            "action": "attribute",
            "target": "urn:asset:1",
            "attributingParty": "urn:p2",
            "attributedParty": "urn:p1",
            "contractedParty": "urn:p3"
        }]
    }));
    assert_eq!(
        only_rule(&doc).sentence,
        "urn:person:b may attribute urn:asset:1 to urn:p1 (attributing party: urn:p2; contracted party: urn:p3), as permitted by urn:org:a."
    );
}

#[test]
fn more_than_one_action_takes_the_generic_clause() {
    let doc = read(agreement(json!([{
        "action": ["attribute", "inform"],
        "target": "urn:asset:1",
        "attributedParty": "urn:p"
    }])));
    assert_eq!(
        only_rule(&doc).sentence,
        "urn:person:b must attribute and inform urn:asset:1 (attributed party: urn:p), as required by urn:org:a."
    );
}

#[test]
fn prefixed_and_full_iri_keys_arrays_and_party_objects_are_read() {
    let doc = read(agreement(json!([
        {"action": "odrl:attribute", "target": "urn:asset:1",
         "odrl:attributedParty": ["urn:p1", {"@id": "urn:p2"}]},
        {"action": "http://www.w3.org/ns/odrl/2/compensate",
         "http://www.w3.org/ns/odrl/2/compensatedParty": {"uid": "urn:p3"}},
        {"action": "inform",
         "informedParty": {"@list": [{"@type": "PartyCollection", "source": "urn:team"}]}}
    ])));
    let sentences: Vec<&str> = doc.policies[0]
        .rules
        .iter()
        .map(|r| r.sentence.as_str())
        .collect();
    assert_eq!(
        sentences,
        [
            "urn:person:b must attribute urn:asset:1 to urn:p1 and urn:p2, as required by urn:org:a.",
            "urn:person:b must compensate urn:p3, as required by urn:org:a.",
            "urn:person:b must inform any party in the collection urn:team, as required by urn:org:a.",
        ]
    );
    assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
}

#[test]
fn a_refined_party_reads_its_refinement_and_carries_its_references() {
    let doc = read(agreement(json!([{
        "action": "inform",
        "informedParty": {
            "@id": "urn:p",
            "odrl:refinement": [
                {"leftOperand": "industry", "operator": "eq", "rightOperand": "press"},
                "http://example.com/c7"
            ]
        }
    }])));
    let rule = only_rule(&doc);
    assert_eq!(
        rule.sentence,
        "urn:person:b must inform urn:p where the industry is equal to \"press\"; the constraint defined at http://example.com/c7 holds, as required by urn:org:a."
    );
    assert_eq!(rule.references, ["http://example.com/c7"]);
    assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
}

#[test]
fn a_remedy_names_its_party() {
    let doc = read(json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Set",
        "uid": "urn:policy:1",
        "prohibition": [{
            "action": "distribute",
            "target": "urn:asset:1",
            "remedy": [{"action": "compensate", "compensatedParty": "urn:p"}]
        }]
    }));
    let remedy = &only_rule(&doc).follow_ups[0].rules[0];
    assert_eq!(remedy.sentence, "The assignee must compensate urn:p.");
    assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
}

#[test]
fn a_rebound_odrl_prefix_is_still_reported() {
    let doc = read(json!({
        "@context": {"odrl": "https://example.com/not-odrl/"},
        "@type": "Set",
        "uid": "urn:policy:1",
        "permission": [{"action": "use", "target": "urn:asset:1", "odrl:informedParty": "urn:p"}]
    }));
    assert_eq!(only_rule(&doc).sentence, "Anyone may use urn:asset:1.");
    assert_eq!(
        doc.warnings,
        ["ignored unknown property \"odrl:informedParty\" on a rule"]
    );
}
