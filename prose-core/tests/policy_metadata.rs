//! A policy's `timestamp` (the Dataspace Protocol's contract agreement) and
//! `summary` read as metadata sentences among the policy's notes, ahead of
//! the profile, conflict and inheritance notes, and are no longer reported
//! as unknown properties.
use prose_core::{Document, render};
use serde_json::{Value, json};

fn read(policy: Value) -> Document {
    render(&policy.to_string()).unwrap()
}

fn notes(doc: &Document) -> Vec<&str> {
    doc.policies[0].notes.iter().map(String::as_str).collect()
}

fn agreement_at(key: &str, timestamp: Value) -> Value {
    let mut p = json!({
        "@type": "Agreement",
        "@id": "urn:agreement:1",
        "permission": [{ "action": "use", "target": "urn:asset:1" }]
    });
    p[key] = timestamp;
    p
}

/// The agreement of the Dataspace Protocol's `contract-agreement-message.json`,
/// as the catalog lifts it out of the message.
#[test]
fn dsp_contract_agreement_says_when_it_was_agreed() {
    let doc = read(json!({
        "@context": ["https://w3id.org/dspace/2025/1/context.jsonld"],
        "@id": "urn:uuid:e8dc8655-44c2-46ef-b701-4cffdc2faa44",
        "@type": "Agreement",
        "target": "urn:uuid:3dd1add4-4d2d-569e-d634-8394a8836d23",
        "timestamp": "2023-01-01T01:00:00Z",
        "assigner": "http://example.com/Provider",
        "assignee": "http://example.com/Consumer",
        "permission": [{
            "action": "use",
            "constraint": [{
                "leftOperand": "dateTime",
                "operator": "lteq",
                "rightOperand": "2023-12-31T06:00Z"
            }]
        }]
    }));
    assert_eq!(notes(&doc), ["Agreed on 2023-01-01 at 01:00 UTC."]);
    assert!(
        doc.warnings.iter().all(|w| !w.contains("timestamp")),
        "{:?}",
        doc.warnings
    );
}

#[test]
fn timestamp_keys_prefixed_or_as_full_iris_are_read() {
    for key in [
        "dspace:timestamp",
        "odrl:timestamp",
        "http://www.w3.org/ns/odrl/2/timestamp",
        "https://w3id.org/dspace/2025/1/timestamp",
        "https://w3id.org/dspace/v0.8/timestamp",
    ] {
        let doc = read(agreement_at(key, json!("2026-10-15T10:00:00Z")));
        assert_eq!(notes(&doc), ["Agreed on 2026-10-15 at 10:00 UTC."], "{key}");
        assert!(doc.warnings.is_empty(), "{key}: {:?}", doc.warnings);
    }
}

#[test]
fn a_typed_timestamp_reads_its_value() {
    let doc = read(agreement_at(
        "timestamp",
        json!({ "@value": "2026-10-15T10:00:00+02:00", "@type": "xsd:dateTime" }),
    ));
    assert_eq!(notes(&doc), ["Agreed on 2026-10-15 at 10:00 UTC+02:00."]);
    assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
}

#[test]
fn a_timestamp_keeps_its_seconds_and_a_date_alone_reads_as_a_date() {
    let doc = read(agreement_at("timestamp", json!("2026-10-15T10:00:30.5Z")));
    assert_eq!(notes(&doc), ["Agreed on 2026-10-15 at 10:00:30.5 UTC."]);
    let doc = read(agreement_at("timestamp", json!("2026-10-15T10:00:00")));
    assert_eq!(notes(&doc), ["Agreed on 2026-10-15 at 10:00."]);
    let doc = read(agreement_at("timestamp", json!("2026-10-15")));
    assert_eq!(notes(&doc), ["Agreed on 2026-10-15."]);
}

#[test]
fn a_timestamp_that_is_not_a_date_is_kept_as_written() {
    let doc = read(agreement_at("timestamp", json!("last Tuesday")));
    assert_eq!(notes(&doc), ["Agreed at \"last Tuesday\"."]);
}

#[test]
fn a_timestamp_on_another_kind_of_policy_reads_as_issued() {
    let doc = read(json!({
        "@type": "Offer",
        "uid": "urn:offer:1",
        "timestamp": "2026-10-15T10:00:00Z",
        "permission": [{ "action": "use", "target": "urn:asset:1" }]
    }));
    assert_eq!(notes(&doc), ["Issued on 2026-10-15 at 10:00 UTC."]);
}

#[test]
fn a_summary_reads_as_a_sentence() {
    let doc = read(json!({
        "@type": "Set",
        "uid": "urn:set:1",
        "summary": "Partners may read the dataset",
        "permission": [{ "action": "read", "target": "urn:asset:1" }]
    }));
    assert_eq!(notes(&doc), ["Summary: Partners may read the dataset."]);
    assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
}

#[test]
fn language_tagged_summaries_read_one_each_with_their_language() {
    let doc = read(json!({
        "@type": "Set",
        "uid": "urn:set:1",
        "odrl:summary": [
            { "@value": "Partners may read the dataset.", "@language": "en" },
            { "@value": "Partner dürfen den Datensatz lesen.", "@language": "de" }
        ],
        "permission": [{ "action": "read", "target": "urn:asset:1" }]
    }));
    assert_eq!(
        notes(&doc),
        [
            "Summary (en): Partners may read the dataset.",
            "Summary (de): Partner dürfen den Datensatz lesen."
        ]
    );
    assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
}

#[test]
fn an_unreadable_summary_is_reported() {
    let doc = read(json!({
        "@type": "Set",
        "uid": "urn:set:1",
        "summary": { "a": 1 },
        "permission": [{ "action": "read", "target": "urn:asset:1" }]
    }));
    assert!(notes(&doc).is_empty());
    assert_eq!(doc.warnings, ["skipped an unreadable summary"]);
}

#[test]
fn metadata_comes_before_the_profile_and_conflict_notes() {
    let doc = read(json!({
        "@type": "Agreement",
        "uid": "urn:agreement:1",
        "profile": "https://test.com/profile",
        "conflict": "perm",
        "summary": "Use of one asset.",
        "timestamp": "2026-10-15T10:00:00Z",
        "permission": [{ "action": "use", "target": "urn:asset:1" }]
    }));
    assert_eq!(
        notes(&doc),
        [
            "Agreed on 2026-10-15 at 10:00 UTC.",
            "Summary: Use of one asset.",
            "Written to the profile https://test.com/profile.",
            "If a permission and a prohibition conflict, the permission wins."
        ]
    );
}

/// The W3C Information Model's eg5 writes `summary` on a permission, as an
/// asset relation its profile declares: it is not the policy's summary.
#[test]
fn a_summary_on_a_rule_is_not_policy_metadata() {
    let doc = read(json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Policy",
        "uid": "http://example.com/policy:1011",
        "profile": "http://example.com/odrl:profile:03",
        "permission": [{
            "target": { "@type": "AssetCollection", "uid": "http://example.com/archive1011" },
            "action": "index",
            "summary": "http://example.com/x/database"
        }]
    }));
    assert_eq!(
        notes(&doc),
        ["Written to the profile http://example.com/odrl:profile:03."]
    );
    assert_eq!(
        doc.warnings,
        ["ignored unknown property \"summary\" on a rule"]
    );
}
