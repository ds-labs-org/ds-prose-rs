//! A URL in the plain string of an `acceptedTerms` constraint is checked
//! after the whole policy is read, as an [`Check::AcceptedTerms`], and is a
//! link labelled with itself when it answers. The check is injected here;
//! `links.rs` tests the real one.
use compliance_catalog::sources::Found;
use compliance_catalog::{Check, entry};
use serde_json::json;

fn found() -> Found {
    Found {
        slug: "terms".into(),
        title: "Accepted terms".into(),
        about: String::new(),
        link: "https://example.com/policy".into(),
        policy: json!({ "@context": "http://www.w3.org/ns/odrl.jsonld",
            "@type": "Offer", "uid": "urn:p", "assigner": "urn:x",
            "permission": [{ "action": "use", "target": "urn:a",
                "constraint": [
                    { "leftOperand": "https://w3id.org/edc/v0.0.1/ns/acceptedTerms",
                      "operator": "eq", "rightOperand": "https://example.com/terms/v1" },
                    { "@id": "https://example.com/c/elsewhere" } ] }] }),
    }
}

const TERMS: &str = "https://example.com/terms/v1";

#[test]
fn each_url_is_checked_once_with_its_kind() {
    let mut asked = Vec::new();
    entry(found(), &mut |iri: &str, kind: Check| {
        asked.push((iri.to_string(), kind));
        false
    })
    .unwrap();
    asked.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        asked,
        [
            (
                "https://example.com/c/elsewhere".to_string(),
                Check::Reference
            ),
            (TERMS.to_string(), Check::AcceptedTerms),
        ]
    );
}

#[test]
fn a_url_that_answers_is_a_link_labelled_with_itself() {
    let e = entry(found(), &mut |iri: &str, _: Check| iri == TERMS).unwrap();
    let link = format!(r#"<a href="{TERMS}" class="prose-terms">{TERMS}</a>"#);
    assert!(e.html.contains(&link), "{}", e.html);
    assert!(!e.html.contains("prose-reference"), "{}", e.html);
}

#[test]
fn a_url_that_does_not_answer_is_text() {
    let e = entry(found(), &mut |_: &str, _: Check| false).unwrap();
    assert!(!e.html.contains("<a "), "{}", e.html);
    assert!(e.html.contains(TERMS), "{}", e.html);
}
