//! `prose:closed`, the ds-prose closing marker: on a left operand it makes
//! the operand's `prose:` hints binding in strict validation. A ds-prose
//! convention, not ODRL; without it the hints stay suggestions.
use prose_core::profile::{ProfileLimits, ProfileVocab, Term};
use serde_json::{Value, json};

const EX: &str = "https://example.org/odrl/profile/research/";
const PROSE: &str = "https://ds-labs-org.github.io/ds-prose-rs/profile#";

fn profile_text(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/profiles/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn from(v: Value) -> ProfileVocab {
    ProfileVocab::from_value(&v, &ProfileLimits::default()).unwrap()
}

fn term<'a>(v: &'a ProfileVocab, local: &str) -> &'a Term {
    v.term(&format!("{EX}{local}"))
        .unwrap_or_else(|| panic!("no term {local}"))
}

#[test]
fn closed_read_on_marked_left_operands() {
    let v = ProfileVocab::parse(&profile_text("research-closed.jsonld")).unwrap();
    assert!(v.warnings().is_empty(), "{:?}", v.warnings());
    assert!(term(&v, "dataSubjectCount").hints.closed);
    assert!(term(&v, "researchPurpose").hints.closed);
    assert!(!term(&v, "retentionPeriod").hints.closed);
}

#[test]
fn closed_false_when_absent() {
    let v = ProfileVocab::parse(&profile_text("research.jsonld")).unwrap();
    assert!(v.terms().all(|t| !t.hints.closed));
    let v = ProfileVocab::parse(&profile_text("odrl22-excerpt.json")).unwrap();
    assert!(v.terms().all(|t| !t.hints.closed));
}

#[test]
fn closed_makes_a_different_vocabulary() {
    let open = ProfileVocab::parse(&profile_text("research.jsonld")).unwrap();
    let closed = ProfileVocab::parse(&profile_text("research-closed.jsonld")).unwrap();
    assert_ne!(open, closed);
}

#[test]
fn closed_values_written_several_ways() {
    let ctx = json!({"odrl": "http://www.w3.org/ns/odrl/2/", "ex": EX, "prose": PROSE,
                     "xsd": "http://www.w3.org/2001/XMLSchema#"});
    let v = from(json!({
        "@context": ctx,
        "@graph": [
            {"@id": "ex:a", "@type": "odrl:LeftOperand", "prose:closed": true},
            {"@id": "ex:b", "@type": "odrl:LeftOperand", "prose:closed": "true"},
            {"@id": "ex:c", "@type": "odrl:LeftOperand",
             "prose:closed": {"@value": "1", "@type": "xsd:boolean"}},
            {"@id": "ex:d", "@type": "odrl:LeftOperand", "prose:closed": false},
            {"@id": "ex:e", "@type": "odrl:LeftOperand", "prose:closed": "yes"},
            {"@id": "ex:f", "@type": "odrl:LeftOperand", "prose:closed": {"@id": "ex:true"}},
            // Only a left operand carries hints.
            {"@id": "ex:g", "@type": "odrl:Action", "prose:closed": true}
        ]
    }));
    for (local, want) in [
        ("a", true),
        ("b", true),
        ("c", true),
        ("d", false),
        ("e", false),
        ("f", false),
        ("g", false),
    ] {
        assert_eq!(term(&v, local).hints.closed, want, "{local}");
    }
}

#[cfg(feature = "turtle")]
#[test]
fn closed_parsed_jsonld_and_turtle_equal() {
    let ttl = ProfileVocab::parse(&profile_text("research-closed.ttl")).unwrap();
    let jsonld = ProfileVocab::parse(&profile_text("research-closed.jsonld")).unwrap();
    assert!(ttl.warnings().is_empty(), "{:?}", ttl.warnings());
    assert_eq!(ttl, jsonld);
    assert!(term(&ttl, "researchPurpose").hints.closed);
    let open = ProfileVocab::parse(&profile_text("research.ttl")).unwrap();
    assert_ne!(ttl, open);
}
