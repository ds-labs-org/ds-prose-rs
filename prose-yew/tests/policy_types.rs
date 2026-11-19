//! `EditConfig::with_policy_types` (issue #79): the policy types a host
//! offers. With one type the heading shows it as text and the select is
//! gone; with several only those are listed; a document with another type
//! keeps it, selected, noted and flagged. Rendered on the server.
#![cfg(not(target_arch = "wasm32"))]
use prose_core::edit::{EditDoc, read_model};
use prose_core::profile::{ProfileVocab, Wording};
use prose_core::validate::Options;
use prose_yew::{EditConfig, EditMode, OdrlProseView, TermInput};
use std::rc::Rc;

async fn ssr_mode(doc: Rc<EditDoc>, config: EditConfig, mode: EditMode) -> String {
    let issues = Rc::new(match &config.validator {
        Some(v) => v.validate(&doc).issues(),
        None => vec![],
    });
    let local = tokio::task::LocalSet::new();
    let config = Rc::new(config);
    local
        .run_until(
            yew::LocalServerRenderer::<OdrlProseView>::with_props(yew::props!(
                prose_yew::OdrlProseViewProps {
                    doc,
                    mode,
                    config,
                    issues,
                }
            ))
            .render(),
        )
        .await
}

async fn ssr(doc: Rc<EditDoc>, config: EditConfig) -> String {
    ssr_mode(doc, config, EditMode::Edit).await
}

fn doc(kind: &str) -> Rc<EditDoc> {
    Rc::new(
        read_model(&format!(
            r#"{{
  "@context": ["http://www.w3.org/ns/odrl.jsonld", {{"ex": "https://example.org/p#"}}],
  "@type": "{kind}", "uid": "urn:p",
  "permission": [{{"target": "urn:a", "action": "use"}}]
}}"#
        ))
        .unwrap(),
    )
}

fn config(types: &[&str], term_input: TermInput) -> EditConfig {
    let c = EditConfig {
        term_input,
        ..EditConfig::default()
    };
    c.with_policy_types(types.iter().map(|t| t.to_string()).collect())
}

const KIND: &str = "policy[0]#kind";

fn element(html: &str, slot: &str) -> Option<String> {
    let needle = format!("data-slot=\"{slot}\"");
    let at = html.find(&needle)?;
    let start = html[..at].rfind('<').unwrap();
    let name: String = html[start + 1..]
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    let close = format!("</{name}>");
    let end = at + html[at..].find(&close).unwrap() + close.len();
    Some(html[start..end].to_string())
}

fn option_values(select: &str) -> Vec<String> {
    select
        .split("<option value=\"")
        .skip(1)
        .map(|o| o.split('"').next().unwrap().to_string())
        .collect()
}

fn heading(html: &str) -> String {
    let start = html.find("<h3").expect("a heading");
    let end = html[start..].find("</h3>").unwrap();
    html[start..start + end].to_string()
}

#[tokio::test]
async fn by_default_every_class_is_listed_in_a_select() {
    let c = EditConfig {
        term_input: TermInput::Select,
        ..EditConfig::default()
    };
    let html = ssr(doc("Set"), c).await;
    let sel = element(&html, KIND).expect("a select");
    assert!(sel.starts_with("<select"), "{sel}");
    assert_eq!(
        option_values(&sel),
        [
            "Set",
            "Offer",
            "Agreement",
            "Privacy",
            "Request",
            "Ticket",
            "Assertion"
        ]
    );
}

#[tokio::test]
async fn with_a_free_input_and_no_types_the_kind_stays_as_it_was() {
    let html = ssr(doc("Set"), EditConfig::default()).await;
    let el = element(&html, KIND).expect("a slot");
    assert!(!el.starts_with("<select"), "{el}");
}

#[tokio::test]
async fn one_type_is_text_in_the_heading_and_has_no_select() {
    let html = ssr(doc("Set"), config(&["Set"], TermInput::Select)).await;
    assert!(element(&html, KIND).is_none(), "{html}");
    assert!(!html.contains("<select data-slot=\"policy[0]#kind\""));
    let h = heading(&html);
    assert!(h.contains("Set"), "{h}");
    assert!(!h.contains("<select") && !h.contains("<input"), "{h}");
}

#[tokio::test]
async fn one_type_is_text_with_free_input_too() {
    let html = ssr(doc("Set"), config(&["Set"], TermInput::Free)).await;
    assert!(element(&html, KIND).is_none(), "{html}");
    assert!(heading(&html).contains("Set"));
}

#[tokio::test]
async fn one_type_the_document_does_not_hold_keeps_a_select_with_the_outside_option() {
    let html = ssr(doc("Offer"), config(&["Set"], TermInput::Select)).await;
    let sel = element(&html, KIND).expect("a select");
    assert!(sel.starts_with("<select"), "{sel}");
    assert!(sel.contains("data-outside=\"true\""), "{sel}");
    assert_eq!(option_values(&sel), ["Offer", "Set"]);
    assert!(html.contains("prose-slot-note"), "{html}");
}

#[tokio::test]
async fn one_type_written_another_way_is_still_text() {
    for kind in ["odrl:Set", "http://www.w3.org/ns/odrl/2/Set"] {
        let html = ssr(doc(kind), config(&["Set"], TermInput::Select)).await;
        assert!(element(&html, KIND).is_none(), "{kind}: {html}");
    }
}

#[tokio::test]
async fn several_types_list_only_those_in_the_hosts_order() {
    let html = ssr(
        doc("Offer"),
        config(&["Offer", "odrl:Agreement", "Set"], TermInput::Select),
    )
    .await;
    let sel = element(&html, KIND).unwrap();
    assert_eq!(option_values(&sel), ["Offer", "Agreement", "Set"]);
    assert!(!sel.contains("data-outside"), "{sel}");
    assert!(sel.contains("<option value=\"Offer\" selected=\"selected\""));
}

#[tokio::test]
async fn several_types_make_a_select_of_a_free_slot() {
    let html = ssr(doc("Set"), config(&["Set", "Offer"], TermInput::Free)).await;
    let sel = element(&html, KIND).unwrap();
    assert!(sel.starts_with("<select"), "{sel}");
    assert_eq!(option_values(&sel), ["Set", "Offer"]);
}

#[tokio::test]
async fn a_type_outside_the_list_stays_selected_with_the_host_note() {
    let html = ssr(
        doc("Agreement"),
        config(&["Set", "Offer"], TermInput::Select),
    )
    .await;
    let sel = element(&html, KIND).unwrap();
    assert!(sel.contains("data-outside=\"true\""), "{sel}");
    assert_eq!(option_values(&sel), ["Agreement", "Set", "Offer"]);
    assert!(
        sel.contains("aria-describedby=\"prose-note-policy[0]#kind\""),
        "{sel}"
    );
    assert!(
        html.contains("prose-slot-note") && html.contains("(not in the host&#x27;s list)")
            || html.contains("(not in the host's list)"),
        "{html}"
    );
}

#[tokio::test]
async fn an_outside_type_is_flagged_with_validation_on() {
    let c = config(&["Set"], TermInput::Select).with_strict(Options::strict());
    let html = ssr(doc("Offer"), c).await;
    let sel = element(&html, KIND).unwrap();
    assert!(sel.contains("aria-invalid=\"true\""), "{sel}");
    assert!(sel.contains("prose-slot-invalid"), "{sel}");
    assert!(html.contains("is not a policy type"), "{html}");
}

#[tokio::test]
async fn an_allowed_type_is_not_flagged() {
    let c = config(&["Set", "Offer"], TermInput::Select).with_strict(Options::strict());
    let html = ssr(doc("Offer"), c).await;
    assert!(!html.contains("is not a policy type"), "{html}");
}

#[tokio::test]
async fn read_mode_shows_the_heading_as_before() {
    let plain = ssr_mode(doc("Offer"), EditConfig::default(), EditMode::Read).await;
    let limited = ssr_mode(
        doc("Offer"),
        config(&["Set"], TermInput::Select),
        EditMode::Read,
    )
    .await;
    assert_eq!(plain, limited);
}

#[tokio::test]
async fn the_uid_and_the_type_are_both_text_when_the_host_holds_both() {
    let mut c = config(&["Set"], TermInput::Select);
    c.rules.policy_uid = prose_core::edit::Limit::NONE;
    let html = ssr(doc("Set"), c).await;
    let h = heading(&html);
    assert!(!h.contains("<select") && !h.contains("<input"), "{h}");
    assert!(h.contains("Set") && h.contains("urn:p"), "{h}");
}

#[tokio::test]
async fn strict_filters_the_listed_types_further() {
    let text = std::fs::read_to_string(format!(
        "{}/../prose-core/tests/profiles/research.jsonld",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let w = Rc::new(Wording::new().with_profile(ProfileVocab::parse(&text).unwrap()));
    let ex = "https://example.org/research#ResearchAgreement";
    let c = EditConfig {
        term_input: TermInput::Select,
        ..EditConfig::default().with_wording(w)
    }
    .with_policy_types(vec!["Set".into(), "Offer".into(), ex.into()])
    .with_strict(Options::strict());
    let html = ssr(doc("Set"), c).await;
    let sel = element(&html, KIND).unwrap();
    let values = option_values(&sel);
    // Only listed ones, never the profile's other classes.
    assert!(
        values
            .iter()
            .all(|v| ["Set", "Offer"].contains(&v.as_str()) || v.contains("Research")),
        "{values:?}"
    );
    assert!(values.len() <= 3, "{values:?}");
}

#[tokio::test]
async fn with_policy_types_keeps_the_sources_already_set() {
    use prose_core::profile::{SourceValue, ValueSource, ValueSources};
    let mut s = ValueSources::new();
    s.set_role(
        prose_core::edit::EntityRole::Assignee,
        ValueSource::new(vec![SourceValue::new("did:web:a")], true),
    );
    let c = EditConfig {
        term_input: TermInput::Select,
        ..EditConfig::default().with_value_sources(s)
    }
    .with_policy_types(vec!["Set".into()]);
    assert!(
        c.wording
            .role_source(prose_core::edit::EntityRole::Assignee)
            .is_some()
    );
    assert!(c.wording.policy_type_source().is_some());
    assert_eq!(c.rules.policy_types, ["http://www.w3.org/ns/odrl/2/Set"]);
    // And clearing leaves the other source alone.
    let c = c.with_policy_types(vec![]);
    assert!(c.wording.policy_type_source().is_none());
    assert!(c.rules.policy_types.is_empty());
    assert!(
        c.wording
            .role_source(prose_core::edit::EntityRole::Assignee)
            .is_some()
    );
}

#[tokio::test]
async fn duplicates_are_removed_after_expansion_and_the_order_is_kept() {
    let c = EditConfig::default().with_policy_types(vec![
        "Offer".into(),
        "odrl:Offer".into(),
        "Set".into(),
        "http://www.w3.org/ns/odrl/2/Set".into(),
    ]);
    assert_eq!(
        c.rules.policy_types,
        [
            "http://www.w3.org/ns/odrl/2/Offer",
            "http://www.w3.org/ns/odrl/2/Set"
        ]
    );
}

const DEP: &str = "https://example.org/dep/";

fn deprecating_classes() -> Rc<Wording> {
    let text = format!(
        r#"{{
            "@context": {{"odrl": "http://www.w3.org/ns/odrl/2/", "ex": "{DEP}",
                         "owl": "http://www.w3.org/2002/07/owl#",
                         "rdfs": "http://www.w3.org/2000/01/rdf-schema#"}},
            "@id": "{DEP}profile",
            "@graph": [
                {{"@id": "ex:Current", "@type": ["rdfs:Class", "owl:Class"],
                  "rdfs:label": "Current class", "rdfs:subClassOf": {{"@id": "odrl:Offer"}}}},
                {{"@id": "ex:Old", "@type": ["rdfs:Class", "owl:Class"],
                  "rdfs:label": "Old class", "rdfs:subClassOf": {{"@id": "odrl:Offer"}},
                  "owl:deprecated": true}}
            ]
        }}"#
    );
    Rc::new(Wording::new().with_profile(ProfileVocab::parse(&text).unwrap()))
}

#[tokio::test]
async fn the_profiles_other_classes_are_not_listed_deprecated_or_not() {
    let base = || EditConfig {
        term_input: TermInput::Select,
        ..EditConfig::default().with_wording(deprecating_classes())
    };
    // Without a limit the profile's classes are listed, the old one last.
    let html = ssr(doc("Set"), base()).await;
    let sel = element(&html, KIND).unwrap();
    assert!(
        sel.contains("Old class") && sel.contains("Current class"),
        "{sel}"
    );
    // With one, only what the host lists, with the profile's label.
    let c = base().with_policy_types(vec!["Set".into(), format!("{DEP}Current")]);
    let html = ssr(doc("Set"), c).await;
    let sel = element(&html, KIND).unwrap();
    assert!(!sel.contains("Old class"), "{sel}");
    assert!(!sel.contains("optgroup"), "{sel}");
    assert!(sel.contains(">Current class</option>"), "{sel}");
    assert_eq!(option_values(&sel).len(), 2, "{sel}");
}

#[tokio::test]
async fn value_sources_set_after_the_types_do_not_lift_the_limit_from_validation() {
    use prose_core::profile::ValueSources;
    let c = config(&["Set"], TermInput::Select)
        .with_value_sources(ValueSources::new())
        .with_strict(Options::strict());
    assert_eq!(c.rules.policy_types.len(), 1);
    assert!(c.wording.policy_type_source().is_some());
    let html = ssr(doc("Offer"), c).await;
    assert!(html.contains("is not a policy type"), "{html}");
}
