//! Runtime value sources in the editor (`EditConfig::with_value_sources`,
//! issue #76 C): the values a host took from its registry fill the select of
//! a right operand, listed with their labels under `<optgroup>` headings; a
//! value the source no longer lists stays in the document, selected, noted
//! and flagged. A source that only suggests leaves the slot free text.
//! Rendered on the server.
#![cfg(not(target_arch = "wasm32"))]
use prose_core::edit::{EditDoc, read_model};
use prose_core::profile::{ProfileVocab, SourceValue, ValueSource, ValueSources, Wording};
use prose_core::validate::Options;
use prose_yew::{EditConfig, EditMode, OdrlProseView, TermInput};
use std::rc::Rc;

async fn ssr(doc: Rc<EditDoc>, config: EditConfig) -> String {
    // The host passes the validator's findings, as the wasm editor does.
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
                    mode: EditMode::Edit,
                    config,
                    issues,
                }
            ))
            .render(),
        )
        .await
}

const DX: &str = "https://example.org/dataspace/";

fn wording() -> Rc<Wording> {
    let text = std::fs::read_to_string(format!(
        "{}/../prose-core/tests/shapes/dataspace-profile.jsonld",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    Rc::new(Wording::new().with_profile(ProfileVocab::parse(&text).unwrap()))
}

fn dids(closed: bool) -> ValueSources {
    let mut s = ValueSources::new();
    s.set_operand(
        &format!("{DX}participantType"),
        ValueSource::new(
            vec![
                SourceValue::new("did:web:alpha.example")
                    .with_label("Alpha Corp")
                    .with_group("Members"),
                SourceValue::new("did:web:beta.example")
                    .with_label("Beta Ltd")
                    .with_group("Members"),
                SourceValue::new("did:web:gamma.example").with_group("Observers"),
            ],
            closed,
        ),
    );
    s
}

fn config(sources: Option<ValueSources>, strict: bool) -> EditConfig {
    let c = EditConfig {
        term_input: TermInput::Select,
        ..EditConfig::default().with_wording(wording())
    };
    let c = match sources {
        Some(s) => c.with_value_sources(s),
        None => c,
    };
    match strict {
        true => c.with_strict(Options::strict()),
        false => c,
    }
}

fn policy(value: &str) -> String {
    format!(
        r#"{{
  "@context": ["http://www.w3.org/ns/odrl.jsonld", {{"dx": "{DX}"}}],
  "@type": "Set", "uid": "urn:p", "profile": "{DX}profile",
  "permission": [{{"target": "urn:a", "action": "dx:train", "constraint": [
    {{"leftOperand": "dx:participantType", "operator": "eq", "rightOperand": {value}}}
  ]}}]
}}"#
    )
}

fn doc(value: &str) -> Rc<EditDoc> {
    Rc::new(read_model(&policy(value)).unwrap())
}

const RIGHT: &str = "policy[0].permission[0].constraint[0]#rightOperand[0]";

fn element(html: &str, slot: &str) -> String {
    let needle = format!("data-slot=\"{slot}\"");
    let at = html
        .find(&needle)
        .unwrap_or_else(|| panic!("no {slot}:\n{html}"));
    let start = html[..at].rfind('<').unwrap();
    let name: String = html[start + 1..]
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    let close = format!("</{name}>");
    let end = at + html[at..].find(&close).unwrap() + close.len();
    html[start..end].to_string()
}

#[tokio::test]
async fn a_closed_host_source_fills_the_select_with_labels_under_groups() {
    let html = ssr(
        doc(r#"{"@id": "did:web:alpha.example"}"#),
        config(Some(dids(true)), false),
    )
    .await;
    let right = element(&html, RIGHT);
    assert!(right.starts_with("<select"), "{right}");
    for want in [
        "<optgroup label=\"Members\">",
        "<optgroup label=\"Observers\">",
        "value=\"did:web:alpha.example\"",
        "Alpha Corp",
        "Beta Ltd",
        // No label: the value itself.
        ">did:web:gamma.example</option>",
    ] {
        assert!(right.contains(want), "{want} in {right}");
    }
    assert!(
        right.contains("<option value=\"did:web:alpha.example\" selected=\"selected\""),
        "{right}"
    );
    assert!(!right.contains("data-outside"), "{right}");
}

#[tokio::test]
async fn without_the_source_the_same_right_operand_is_free_text() {
    let html = ssr(
        doc(r#"{"@id": "did:web:alpha.example"}"#),
        config(None, false),
    )
    .await;
    assert!(!element(&html, RIGHT).starts_with("<select"));
}

#[tokio::test]
async fn a_suggest_only_source_leaves_the_slot_free_text() {
    let html = ssr(
        doc(r#"{"@id": "did:web:alpha.example"}"#),
        config(Some(dids(false)), false),
    )
    .await;
    assert!(!element(&html, RIGHT).starts_with("<select"));
}

#[tokio::test]
async fn a_value_the_source_no_longer_lists_stays_selected_and_is_noted() {
    let html = ssr(
        doc(r#"{"@id": "did:web:gone.example"}"#),
        config(Some(dids(true)), true),
    )
    .await;
    let right = element(&html, RIGHT);
    assert!(right.contains("data-outside=\"true\""), "{right}");
    assert!(
        right.contains("<option value=\"did:web:gone.example\" selected=\"selected\">"),
        "{right}"
    );
    // The note says whose list it left, not the profile's.
    assert!(
        html.contains("(not in the host&#x27;s list)") || html.contains("(not in the host's list)"),
        "{html}"
    );
    assert!(!html.contains("(not in the profile)"), "{html}");
    // Flagged by the validator the editor carries.
    assert!(right.contains("prose-slot-invalid"), "{right}");
}

#[tokio::test]
async fn a_plain_string_value_in_the_list_is_the_selected_one() {
    let html = ssr(
        doc(r#""did:web:beta.example""#),
        config(Some(dids(true)), true),
    )
    .await;
    let right = element(&html, RIGHT);
    assert!(
        right.contains("<option value=\"did:web:beta.example\" selected=\"selected\""),
        "{right}"
    );
    assert!(!right.contains("data-outside"), "{right}");
}

#[tokio::test]
async fn the_sources_survive_a_new_wording() {
    // with_wording after with_value_sources keeps them.
    let c = EditConfig::default()
        .with_value_sources(dids(true))
        .with_wording(wording());
    let c = EditConfig {
        term_input: TermInput::Select,
        ..c
    };
    let html = ssr(doc(r#"{"@id": "did:web:alpha.example"}"#), c).await;
    assert!(element(&html, RIGHT).starts_with("<select"));
}
