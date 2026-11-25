//! The editor reads a policy's `timestamp` and `summary` as `render` does:
//! no "ignored unknown property" warning (so no reader note in validation
//! either), the same metadata sentences ahead of the policy's notes, and the
//! keys kept as written when the policy is written back.
use prose_core::edit::*;
use prose_core::profile::Wording;
use prose_core::render;
use prose_core::validate::{Level, Options, RuleId, validate_json};
use serde_json::{Value, json};

fn dsp_agreement() -> Value {
    json!({
        "@context": ["https://w3id.org/dspace/2025/1/context.jsonld"],
        "@id": "urn:uuid:e8dc8655-44c2-46ef-b701-4cffdc2faa44",
        "@type": "Agreement",
        "target": "urn:uuid:3dd1add4-4d2d-569e-d634-8394a8836d23",
        "timestamp": "2023-01-01T01:00:00Z",
        "summary": "Use until the end of 2023",
        "assigner": "http://example.com/Provider",
        "assignee": "http://example.com/Consumer",
        "permission": [{ "action": "use" }]
    })
}

fn metadata_notes(doc: &EditDoc) -> Vec<String> {
    let all: Vec<String> = sentences(doc)[0]
        .notes
        .iter()
        .map(Sentence::plain)
        .collect();
    let first_slot_note = all
        .iter()
        .position(|n| n.starts_with("Applies to "))
        .expect("the target note");
    all[..first_slot_note].to_vec()
}

#[test]
fn the_editor_does_not_warn_of_timestamp_or_summary() {
    let doc = read_model(&dsp_agreement().to_string()).unwrap();
    assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
}

#[test]
fn the_editor_shows_the_metadata_render_shows() {
    let json = dsp_agreement().to_string();
    let doc = read_model(&json).unwrap();
    let rendered = render(&json).unwrap();
    assert_eq!(
        metadata_notes(&doc),
        [
            "Agreed on 2023-01-01 at 01:00 UTC.",
            "Summary: Use until the end of 2023."
        ]
    );
    assert_eq!(metadata_notes(&doc), rendered.policies[0].notes);
}

#[test]
fn prefixed_and_tagged_metadata_reads_in_the_editor_too() {
    let v = json!({
        "@context": { "odrl": "http://www.w3.org/ns/odrl/2/" },
        "@type": "Offer",
        "uid": "urn:offer:1",
        "dspace:timestamp": { "@value": "2026-10-15T10:00:00Z", "@type": "xsd:dateTime" },
        "odrl:summary": [
            { "@value": "Read only.", "@language": "en" },
            { "@value": "Nur lesen.", "@language": "de" }
        ],
        "permission": [{ "action": "read", "target": "urn:asset:1" }]
    });
    let doc = read_model(&v.to_string()).unwrap();
    assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
    assert!(doc.unread.is_empty(), "{:?}", doc.unread);
    assert_eq!(
        metadata_notes(&doc),
        [
            "Issued on 2026-10-15 at 10:00 UTC.",
            "Summary (en): Read only.",
            "Summary (de): Nur lesen."
        ]
    );
}

#[test]
fn an_unreadable_summary_is_reported_by_the_editor() {
    let v = json!({
        "@type": "Set",
        "uid": "urn:set:1",
        "summary": { "a": 1 },
        "permission": [{ "action": "read", "target": "urn:asset:1" }]
    });
    let doc = read_model(&v.to_string()).unwrap();
    assert_eq!(doc.warnings, ["skipped an unreadable summary"]);
    assert!(metadata_notes(&doc).is_empty());
}

#[test]
fn the_agreement_sentence_follows_the_edited_kind() {
    let mut doc = read_model(&dsp_agreement().to_string()).unwrap();
    doc.policies[0].kind = "Offer".into();
    assert_eq!(
        metadata_notes(&doc)[0],
        "Issued on 2023-01-01 at 01:00 UTC."
    );
}

#[test]
fn written_back_the_metadata_keys_are_kept() {
    let doc = read_model(&dsp_agreement().to_string()).unwrap();
    let out = write_jsonld(&doc);
    assert_eq!(out["timestamp"], "2023-01-01T01:00:00Z");
    assert_eq!(out["summary"], "Use until the end of 2023");
}

#[test]
fn validation_has_no_reader_note_for_the_metadata() {
    let report = validate_json(
        &dsp_agreement().to_string(),
        &Wording::new(),
        &Options::default().level(RuleId::InputReaderNote, Level::Warning),
    );
    let notes: Vec<&str> = report
        .findings
        .iter()
        .filter(|f| f.rule == RuleId::InputReaderNote)
        .map(|f| f.message.as_str())
        .filter(|m| m.contains("timestamp") || m.contains("summary"))
        .collect();
    assert!(notes.is_empty(), "{notes:?}");
}
