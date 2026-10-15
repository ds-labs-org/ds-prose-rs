#![cfg(not(target_arch = "wasm32"))]
//! A constraint referred to by an IRI the document does not declare reads
//! as defined at that IRI. The host says which IRIs dereference (the
//! component fetches nothing); those are links, any other is plain text.
use prose_yew::{OdrlProse, OdrlProseProps};
use std::collections::BTreeSet;
use std::rc::Rc;

const POLICY: &str = r#"{"@context": "http://www.w3.org/ns/odrl.jsonld",
  "@type": "Set", "uid": "urn:p",
  "permission": [{"action": "use", "target": "urn:a",
    "constraint": [{"@id": "https://example.com/c/1"},
                   {"or": ["https://example.com/c/2",
                           {"leftOperand": "count", "operator": "lteq", "rightOperand": 1}]}]}]}"#;

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
async fn without_links_every_iri_is_plain_text() {
    let html = ssr(&[]).await;
    assert!(!html.contains("<a "), "{html}");
    assert!(
        html.contains("the constraint defined at https://example.com/c/1 holds"),
        "{html}"
    );
    assert!(
        html.contains("the constraint defined at https://example.com/c/2 holds"),
        "{html}"
    );
}

#[tokio::test]
async fn an_iri_the_host_checked_is_a_link() {
    let html = ssr(&["https://example.com/c/2"]).await;
    assert!(html.contains(&link("https://example.com/c/2")), "{html}");
    assert!(html.contains("the constraint defined at "), "{html}");
    assert!(!html.contains(&link("https://example.com/c/1")), "{html}");
    assert!(
        html.contains("the constraint defined at https://example.com/c/1 holds"),
        "{html}"
    );
}

#[tokio::test]
async fn links_to_iris_the_policy_does_not_refer_to_change_nothing() {
    assert_eq!(ssr(&["https://example.com/other"]).await, ssr(&[]).await);
}
