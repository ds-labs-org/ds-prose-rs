//! A plain string right operand of an `acceptedTerms` constraint may hold
//! the URL of the terms accepted. The reading keeps each `http(s)` URL in it
//! verbatim and lists it on [`Condition::urls`], so a host that finds it
//! answers can show it as a link. The reader fetches nothing.
use prose_core::{Condition, render};

fn condition(left: &str, right: &str) -> Condition {
    let json = format!(
        r#"{{"@context": "http://www.w3.org/ns/odrl.jsonld", "@type": "Set", "uid": "urn:p",
  "permission": [{{"action": "use", "target": "urn:a",
    "constraint": [{{"leftOperand": "{left}", "operator": "eq", "rightOperand": {right}}}]}}]}}"#
    );
    let doc = render(&json).unwrap();
    doc.policies[0].rules[0].conditions[0].clone()
}

const EDC: &str = "https://w3id.org/edc/v0.0.1/ns/acceptedTerms";

#[test]
fn a_url_in_a_plain_string_is_listed_and_read_verbatim() {
    let c = condition(EDC, r#""https://example.com/terms/v1""#);
    assert_eq!(c.urls, ["https://example.com/terms/v1"]);
    assert!(
        c.text.ends_with("https://example.com/terms/v1"),
        "{}",
        c.text
    );
}

#[test]
fn any_namespace_and_a_compact_iri_name_accepted_terms() {
    for left in [
        "ex:acceptedTerms",
        "http://example.com/vocab#acceptedTerms",
        "acceptedTerms",
    ] {
        let c = condition(left, r#""http://example.com/t""#);
        assert_eq!(c.urls, ["http://example.com/t"], "{left}");
    }
}

#[test]
fn urls_inside_a_sentence_are_found_without_trailing_punctuation() {
    let c = condition(
        EDC,
        r#""See https://example.com/a, then (https://example.com/b)." "#,
    );
    assert_eq!(c.urls, ["https://example.com/a", "https://example.com/b"]);
    for u in &c.urls {
        assert!(c.text.contains(u.as_str()), "{}", c.text);
    }
}

#[test]
fn every_plain_string_of_a_list_is_read_and_a_typed_string_is_plain_too() {
    let c = condition(
        EDC,
        r#"["https://example.com/a", {"@value": "https://example.com/b"},
            {"@value": "https://example.com/c", "@type": "xsd:string"}]"#,
    );
    assert_eq!(
        c.urls,
        [
            "https://example.com/a",
            "https://example.com/b",
            "https://example.com/c"
        ]
    );
}

#[test]
fn an_iri_value_another_scheme_or_another_operand_lists_nothing() {
    for (left, right) in [
        (EDC, r#"{"@id": "https://example.com/terms"}"#),
        (EDC, r#""ftp://example.com/terms""#),
        (EDC, r#""urn:terms:1""#),
        (
            EDC,
            r#"{"@value": "https://example.com/x", "@type": "xsd:anyURI"}"#,
        ),
        ("purpose", r#""https://example.com/terms""#),
        ("ex:notAcceptedTerms", r#""https://example.com/terms""#),
    ] {
        assert!(condition(left, right).urls.is_empty(), "{left} {right}");
    }
}
