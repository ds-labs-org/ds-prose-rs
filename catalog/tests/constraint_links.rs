//! A catalog page links a constraint the policy refers to but does not
//! declare when, and only when, its IRI answers HTTP 200. The check is
//! injected here; `links.rs` tests the real one.
use compliance_catalog::sources::Found;
use compliance_catalog::{entry, reference};
use serde_json::json;

/// W3C ODRL IM Example 19 as the catalog reads it, with one more constraint
/// that is declared elsewhere.
fn found() -> Found {
    Found {
        slug: "eg19".into(),
        title: "Constraint references (eg19)".into(),
        about: String::new(),
        link: "https://w3c.github.io/poe/model/#x".into(),
        policy: json!({ "@context": "http://www.w3.org/ns/odrl.jsonld", "@graph": [
            { "@type": "Offer", "uid": "http://example.com/policy:88",
              "permission": [{ "target": "http://example.com/book/1999",
                "action": [{ "rdf:value": { "@id": "odrl:reproduce" },
                  "refinement": { "xone": { "@list": [
                    { "@id": "http://example.com/p:88/C1" },
                    { "@id": "http://example.com/p:88/C2" } ] } } }],
                "constraint": [{ "@id": "https://example.com/c/elsewhere" }] }] },
            { "@type": "Constraint", "uid": "http://example.com/p:88/C1",
              "leftOperand": "media", "operator": "eq", "rightOperand": "online" },
            { "@type": "Constraint", "uid": "http://example.com/p:88/C2",
              "leftOperand": "media", "operator": "eq", "rightOperand": "print" }
        ] }),
    }
}

const LINK: &str = r#"<a href="https://example.com/c/elsewhere" class="prose-reference">https://example.com/c/elsewhere</a>"#;

#[test]
fn a_reference_that_answers_200_is_a_link() {
    let mut asked = Vec::new();
    let e = entry(found(), &mut |iri: &str| {
        asked.push(iri.to_string());
        true
    })
    .unwrap();
    assert!(e.html.contains(LINK), "{}", e.html);
    assert!(e.html.contains("the constraint defined at "), "{}", e.html);
    assert!(!e.incomplete, "{}", e.html);
    assert_eq!(
        asked,
        ["https://example.com/c/elsewhere"],
        "declared constraints are read, not checked"
    );
}

#[test]
fn a_reference_that_does_not_answer_200_is_plain_text() {
    let e = entry(found(), &mut |_: &str| false).unwrap();
    assert!(!e.html.contains("<a "), "{}", e.html);
    assert!(
        e.html
            .contains("the constraint defined at https://example.com/c/elsewhere holds"),
        "{}",
        e.html
    );
    assert!(!e.incomplete);
}

#[test]
fn the_declared_constraints_of_example_19_read() {
    let s = reference(
        "w3c",
        "W3C ODRL examples",
        "",
        "W3C",
        vec![found()],
        &mut |_: &str| false,
    );
    let e = &s.entries[0];
    assert!(
        e.html.contains("the media is equal to \"online\""),
        "{}",
        e.html
    );
    assert!(
        e.html.contains("the media is equal to \"print\""),
        "{}",
        e.html
    );
    assert!(!e.html.contains("incomplete"), "{}", e.html);
    assert!(e.warnings.is_empty(), "{:?}", e.warnings);
}
