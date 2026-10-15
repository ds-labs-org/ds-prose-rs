#![cfg(not(target_arch = "wasm32"))]
//! An IRI the prose names (a party, an asset, a right operand) shows as a
//! short label: the one the document declares, else the IRI's last segment.
//! The element keeps the full IRI in its title, and is a link when the
//! host found the IRI dereferences (`links`, as for constraints in #41).
use prose_yew::{OdrlProse, OdrlProseProps};
use std::collections::BTreeSet;
use std::rc::Rc;

const POLICY: &str = r#"{"@context": "http://www.w3.org/ns/odrl.jsonld",
  "@graph": [
    {"@type": "Offer", "uid": "urn:p",
     "assigner": "http://example.com/party:org:abc",
     "permission": [{"action": "play",
       "target": "http://example.com/asset:9898.movie",
       "assignee": {"@id": "urn:person:b", "http://www.w3.org/2000/01/rdf-schema#label": "Billie"},
       "constraint": [{"leftOperand": "spatial", "operator": "eq",
         "rightOperand": {"@id": "https://www.wikidata.org/wiki/Q183"}},
         {"@id": "https://example.com/c/9"}]}]},
    {"@id": "https://www.wikidata.org/wiki/Q183",
     "http://www.w3.org/2004/02/skos/core#prefLabel": "Germany"}
  ]}"#;

async fn ssr(links: &[&str]) -> String {
    ssr_of(POLICY, links).await
}

async fn ssr_of(json: &'static str, links: &[&str]) -> String {
    let links: BTreeSet<String> = links.iter().map(|s| s.to_string()).collect();
    let props = yew::props!(OdrlProseProps {
        json,
        links: Rc::new(links),
    });
    let local = tokio::task::LocalSet::new();
    local
        .run_until(yew::LocalServerRenderer::<OdrlProse>::with_props(props).render())
        .await
}

/// Each element whose title is `iri`: (its start tag, its text).
fn titled<'a>(html: &'a str, iri: &str) -> Vec<(&'a str, &'a str)> {
    let needle = format!(r#"title="{iri}""#);
    html.match_indices(&needle)
        .map(|(at, _)| {
            let open = html[..at].rfind('<').unwrap();
            let close = at + html[at..].find('>').unwrap();
            let end = close + html[close..].find('<').unwrap();
            (&html[open..=close], &html[close + 1..end])
        })
        .collect()
}

#[tokio::test]
async fn an_iri_without_a_label_shows_its_last_segment_and_keeps_the_iri() {
    let html = ssr(&[]).await;
    let asset = titled(&html, "http://example.com/asset:9898.movie");
    assert_eq!(asset.len(), 1, "{html}");
    let (tag, text) = asset[0];
    assert_eq!(text, "9898.movie", "{html}");
    assert!(
        tag.starts_with("<span") && tag.contains(r#"class="prose-iri""#),
        "{tag}"
    );
    assert!(html.contains("</span> may play <span"), "{html}");
    assert!(
        !html.contains(">http://example.com/asset:9898.movie<"),
        "the IRI is not shown in full: {html}"
    );
}

#[tokio::test]
async fn a_declared_label_is_shown_with_the_iri_in_its_title() {
    let html = ssr(&[]).await;
    let billie = titled(&html, "urn:person:b");
    assert_eq!(billie.len(), 1, "{html}");
    let (tag, text) = billie[0];
    assert!(
        tag.starts_with("<span") && tag.contains(r#"class="prose-iri""#),
        "{tag}"
    );
    assert_eq!(text, "Billie");
    let germany = titled(&html, "https://www.wikidata.org/wiki/Q183");
    assert_eq!(germany.len(), 1, "{html}");
    assert_eq!(germany[0].1, "Germany");
    assert!(html.contains("the location is equal to <span"), "{html}");
}

#[tokio::test]
async fn the_party_of_the_intro_is_short_too() {
    let html = ssr(&[]).await;
    let party = titled(&html, "http://example.com/party:org:abc");
    assert_eq!(party.len(), 2, "in the intro and in the sentence: {html}");
    assert!(party.iter().all(|(_, text)| *text == "abc"), "{html}");
    assert!(html.contains("<p>An offer made by <span"), "{html}");
}

#[tokio::test]
async fn an_iri_the_host_found_to_dereference_is_a_link() {
    let html = ssr(&["https://www.wikidata.org/wiki/Q183"]).await;
    let germany = titled(&html, "https://www.wikidata.org/wiki/Q183");
    assert_eq!(germany.len(), 1, "{html}");
    let (tag, text) = germany[0];
    assert!(tag.starts_with("<a "), "{tag}");
    assert!(
        tag.contains(r#"href="https://www.wikidata.org/wiki/Q183""#),
        "{tag}"
    );
    assert!(tag.contains(r#"class="prose-iri""#), "{tag}");
    assert_eq!(text, "Germany");
    // Not checked by the host: no link.
    let html = ssr(&[]).await;
    assert_eq!(html.matches("<a ").count(), 0, "{html}");
}

#[tokio::test]
async fn a_reference_to_a_constraint_keeps_its_full_iri() {
    let html = ssr(&[]).await;
    assert!(
        html.contains("the constraint defined at https://example.com/c/9 holds"),
        "{html}"
    );
}

#[tokio::test]
async fn two_iris_of_one_short_label_are_shown_in_full() {
    let html = ssr_of(
        r#"{"@context": "http://www.w3.org/ns/odrl.jsonld", "@type": "Set", "uid": "urn:p",
          "permission": [{"action": "play",
            "target": ["http://a.example/films/movie", "http://b.example/films/movie"]}]}"#,
        &[],
    )
    .await;
    for iri in [
        "http://a.example/films/movie",
        "http://b.example/films/movie",
    ] {
        let shown = titled(&html, iri);
        assert_eq!(shown.len(), 1, "{html}");
        assert_eq!(shown[0].1, iri, "the IRI is shown in full: {html}");
    }
}
