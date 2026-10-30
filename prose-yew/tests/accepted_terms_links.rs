#![cfg(not(target_arch = "wasm32"))]
//! An `http(s)` URL in the plain string of an `acceptedTerms` constraint is
//! a link, labelled with the URL itself, when the host lists it in `links`;
//! otherwise it stays text. The component fetches nothing.
use prose_yew::{OdrlProse, OdrlProseProps};
use std::collections::BTreeSet;
use std::rc::Rc;

const POLICY: &str = r#"{"@context": "http://www.w3.org/ns/odrl.jsonld",
  "@type": "Set", "uid": "urn:p",
  "permission": [{"action": "use", "target": "urn:a",
    "constraint": [{"leftOperand": "https://w3id.org/edc/v0.0.1/ns/acceptedTerms",
                    "operator": "eq",
                    "rightOperand": "Read https://example.com/terms/v1 and https://example.com/dead"}]}]}"#;

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

fn link(url: &str) -> String {
    format!(r#"<a href="{url}" class="prose-terms">{url}</a>"#)
}

#[tokio::test]
async fn without_links_the_urls_are_text() {
    let html = ssr(&[]).await;
    assert!(!html.contains("<a "), "{html}");
    assert!(
        html.contains("Read https://example.com/terms/v1 and https://example.com/dead"),
        "{html}"
    );
}

#[tokio::test]
async fn a_url_the_host_checked_is_a_link_labelled_with_itself() {
    let html = ssr(&["https://example.com/terms/v1"]).await;
    assert!(html.contains(&link("https://example.com/terms/v1")), "{html}");
    assert!(!html.contains(&link("https://example.com/dead")), "{html}");
    assert!(html.contains("and https://example.com/dead"), "{html}");
}
