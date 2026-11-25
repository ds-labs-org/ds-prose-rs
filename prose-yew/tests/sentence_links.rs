#![cfg(not(target_arch = "wasm32"))]
//! A reference in a target's or a party's refinement reads inside the
//! rule's sentence; when the host found its IRI dereferences, it is a link
//! there too. Only the reference is a `prose-reference` link and keeps its
//! IRI in full; the asset or the party of the same IRI is a name, shown short
//! (#45).
use prose_yew::{OdrlProse, OdrlProseProps};
use std::collections::BTreeSet;
use std::rc::Rc;

const POLICY: &str = r#"{"@context": "http://www.w3.org/ns/odrl.jsonld",
  "@type": "Set", "uid": "urn:p",
  "permission": [{"action": "use",
    "target": {"@id": "https://example.com/c/9", "refinement": "https://example.com/c/9"},
    "assignee": {"@id": "urn:bob", "refinement": {"@id": "https://example.com/c/8"}}}]}"#;

async fn ssr(links: &[&str]) -> String {
    let links: BTreeSet<String> = links.iter().map(|s| s.to_string()).collect();
    let props = yew::props!(OdrlProseProps {
        json: POLICY,
        links: Rc::new(links),
    });
    let local = tokio::task::LocalSet::new();
    local
        .run_until(yew::LocalServerRenderer::<OdrlProse>::with_props(props).render())
        .await
}

fn link(iri: &str) -> String {
    format!(r#"<a href="{iri}" class="prose-reference">{iri}</a>"#)
}

#[tokio::test]
async fn a_reference_in_the_sentence_is_a_link_when_checked() {
    let html = ssr(&["https://example.com/c/9", "https://example.com/c/8"]).await;
    assert_eq!(
        html.matches(&link("https://example.com/c/9")).count(),
        1,
        "the reference is a reference link, the asset of the same IRI is not: {html}"
    );
    assert!(
        html.contains(&format!(
            ">c/9</a> where the constraint defined at {} holds",
            link("https://example.com/c/9")
        )),
        "{html}"
    );
    assert!(html.contains(&link("https://example.com/c/8")), "{html}");
}

#[tokio::test]
async fn a_reference_in_the_sentence_is_plain_text_otherwise() {
    let html = ssr(&["https://example.com/c/8"]).await;
    assert!(!html.contains(&link("https://example.com/c/9")), "{html}");
    assert!(
        html.contains("the constraint defined at https://example.com/c/9 holds"),
        "{html}"
    );
    assert!(html.contains(&link("https://example.com/c/8")), "{html}");
    assert_eq!(ssr(&[]).await.matches("<a ").count(), 0);
}
