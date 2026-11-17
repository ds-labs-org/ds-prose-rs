//! Runtime value sources in the editor (`EditConfig::with_value_sources`,
//! issue #76 C): the values a host took from its registry fill the select of
//! a right operand, listed with their labels under `<optgroup>` headings; a
//! value the source no longer lists stays in the document, selected, noted
//! and flagged. A source that only suggests leaves the slot free text.
//! Rendered on the server.
#![cfg(not(target_arch = "wasm32"))]
use prose_core::edit::{EditDoc, EntityRole, read_model};
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

#[tokio::test]
async fn a_closed_source_over_an_open_base_keeps_the_units_a_suggestion() {
    // An overlay only suggests units and operators for the left operand; the
    // host closes its values. The values become a select, and the unit stays
    // free text with its suggestions: the host binds the values alone.
    let overlay = prose_core::profile::Overlay::parse(&format!(
        r#"{{"@context": {{"prose": "https://ds-labs-org.github.io/ds-prose-rs/profile#",
                         "dx": "{DX}"}},
            "@graph": [{{"@id": "dx:participantType", "prose:unit": [{{"@id": "dx:kg"}}]}}]}}"#
    ))
    .unwrap();
    let w = Rc::new(
        Rc::try_unwrap(wording())
            .unwrap_or_else(|_| unreachable!())
            .with_overlay(overlay),
    );
    let c = EditConfig {
        term_input: TermInput::Select,
        ..EditConfig::default().with_wording(w)
    }
    .with_value_sources(dids(true));
    let text = policy(r#"{"@id": "did:web:alpha.example"}"#).replace(
        r#""operator": "eq""#,
        r#""operator": "eq", "unit": "dx:kg""#,
    );
    let html = ssr(Rc::new(read_model(&text).unwrap()), c).await;
    assert!(element(&html, RIGHT).starts_with("<select"));
    let unit = element(&html, "policy[0].permission[0].constraint[0]#unit");
    assert!(!unit.starts_with("<select"), "{unit}");
}

// A closed host source for a party role (assigner, assignee, target) is a
// select of its values too, with a value it no longer lists kept.

fn parties(closed: bool) -> ValueSources {
    let mut s = ValueSources::new();
    for role in [
        EntityRole::Assigner,
        EntityRole::Assignee,
        EntityRole::Target,
    ] {
        s.set_role(
            role,
            ValueSource::new(
                vec![
                    SourceValue::new("did:web:alpha.example")
                        .with_label("Alpha Corp")
                        .with_group("Members"),
                    SourceValue::new("did:web:beta.example").with_label("Beta Ltd"),
                ],
                closed,
            ),
        );
    }
    s
}

fn party_doc(assignee: &str) -> Rc<EditDoc> {
    let text = format!(
        r#"{{"@context": "http://www.w3.org/ns/odrl.jsonld", "@type": "Agreement",
            "uid": "urn:p", "assigner": "did:web:alpha.example",
            "permission": [{{"target": "did:web:beta.example", "assignee": "{assignee}",
                            "action": "use"}}]}}"#
    );
    Rc::new(read_model(&text).unwrap())
}

const ASSIGNER: &str = "policy[0].assigner[0]#iri";
const ASSIGNEE: &str = "policy[0].permission[0].assignee[0]#iri";
const TARGET: &str = "policy[0].permission[0].target[0]#iri";

fn party_config(sources: Option<ValueSources>, strict: bool) -> EditConfig {
    config(sources, strict)
}

#[tokio::test]
async fn a_closed_source_for_a_party_role_is_a_select_of_its_values() {
    let html = ssr(
        party_doc("did:web:beta.example"),
        party_config(Some(parties(true)), false),
    )
    .await;
    for slot in [ASSIGNER, ASSIGNEE, TARGET] {
        let el = element(&html, slot);
        assert!(el.starts_with("<select"), "{slot}: {el}");
        assert!(el.contains("Alpha Corp"), "{el}");
        assert!(el.contains("<optgroup label=\"Members\">"), "{el}");
        assert!(!el.contains("data-outside"), "{el}");
    }
    let assignee = element(&html, ASSIGNEE);
    assert!(
        assignee.contains("<option value=\"did:web:beta.example\" selected=\"selected\""),
        "{assignee}"
    );
}

#[tokio::test]
async fn a_party_the_closed_source_no_longer_lists_stays_selected_noted_and_flagged() {
    let html = ssr(
        party_doc("did:web:gone.example"),
        party_config(Some(parties(true)), true),
    )
    .await;
    let el = element(&html, ASSIGNEE);
    assert!(el.starts_with("<select"), "{el}");
    assert!(el.contains("data-outside=\"true\""), "{el}");
    assert!(
        el.contains("<option value=\"did:web:gone.example\" selected=\"selected\">"),
        "{el}"
    );
    assert!(html.contains("(not in the host") && !html.contains("(not in the profile)"));
    assert!(el.contains("prose-slot-invalid"), "{el}");
}

#[tokio::test]
async fn a_suggest_only_or_missing_party_source_leaves_the_slot_free_text() {
    for sources in [Some(parties(false)), None] {
        let html = ssr(
            party_doc("did:web:beta.example"),
            party_config(sources, false),
        )
        .await;
        assert!(!element(&html, ASSIGNEE).starts_with("<select"));
    }
}

#[tokio::test]
async fn only_the_role_the_host_closed_is_a_select() {
    let mut s = ValueSources::new();
    s.set_role(
        EntityRole::Assignee,
        ValueSource::new(vec![SourceValue::new("did:web:beta.example")], true),
    );
    let html = ssr(
        party_doc("did:web:beta.example"),
        party_config(Some(s), false),
    )
    .await;
    assert!(element(&html, ASSIGNEE).starts_with("<select"));
    assert!(!element(&html, ASSIGNER).starts_with("<select"));
    assert!(!element(&html, TARGET).starts_with("<select"));
}

/// Shapes closing `dx:purpose` to research and marketing, with an open host
/// source that labels research and lists a value the shapes do not.
fn open_over_closed(strict: bool) -> EditConfig {
    let shapes = prose_core::shapes::ShapesGraph::parse(
        r#"{
  "@context": {"sh": "http://www.w3.org/ns/shacl#", "odrl": "http://www.w3.org/ns/odrl/2/",
               "dx": "https://example.org/dataspace/"},
  "@graph": [{"@id": "dx:S", "@type": "sh:NodeShape", "sh:property": [
    {"sh:path": {"@id": "odrl:leftOperand"}, "sh:hasValue": {"@id": "dx:purpose"}},
    {"sh:path": {"@id": "odrl:rightOperand"},
     "sh:in": {"@list": [{"@id": "dx:research"}, {"@id": "dx:marketing"}]}}]}]
}"#,
    )
    .unwrap();
    let mut s = ValueSources::new();
    s.set_operand(
        &format!("{DX}purpose"),
        ValueSource::new(
            vec![
                SourceValue::new(&format!("{DX}research"))
                    .with_label("Research (registry)")
                    .with_group("Core"),
                SourceValue::new(&format!("{DX}elsewhere")).with_label("Elsewhere"),
            ],
            false,
        ),
    );
    let w = Wording::clone(&wording()).with_shapes(shapes);
    let c = EditConfig {
        term_input: TermInput::Select,
        ..EditConfig::default().with_wording(Rc::new(w))
    }
    .with_value_sources(s);
    match strict {
        true => c.with_strict(Options::strict()),
        false => c,
    }
}

#[tokio::test]
async fn an_open_host_source_over_closed_shapes_keeps_its_labels_for_the_listed_values() {
    let text = policy(r#"{"@id": "https://example.org/dataspace/research"}"#)
        .replace("dx:participantType", "dx:purpose");
    let doc = Rc::new(read_model(&text).unwrap());
    let html = ssr(doc, open_over_closed(false)).await;
    let right = element(&html, RIGHT);
    assert!(right.starts_with("<select"), "{right}");
    assert!(right.contains("Research (registry)"), "{right}");
    assert!(right.contains("<optgroup label=\"Core\">"), "{right}");
    // The shapes' list is the one offered: the unlisted value is not.
    assert!(!right.contains("Elsewhere"), "{right}");
    assert!(right.contains("Marketing"), "{right}");
}

/// A closed host source that lists profile terms, one without a label.
#[tokio::test]
async fn a_host_value_without_a_label_takes_the_profile_label() {
    let mut s = ValueSources::new();
    s.set_operand(
        &format!("{DX}purpose"),
        ValueSource::new(
            vec![
                SourceValue::new(&format!("{DX}research")),
                SourceValue::new(&format!("{DX}marketing")).with_label("Marketing (registry)"),
                SourceValue::new("urn:unknown:value"),
            ],
            true,
        ),
    );
    let c = EditConfig {
        term_input: TermInput::Select,
        ..EditConfig::default().with_wording(wording())
    }
    .with_value_sources(s);
    let text = policy(r#"{"@id": "https://example.org/dataspace/research"}"#)
        .replace("dx:participantType", "dx:purpose");
    let html = ssr(Rc::new(read_model(&text).unwrap()), c).await;
    let right = element(&html, RIGHT);
    assert!(right.starts_with("<select"), "{right}");
    assert!(right.contains(">Research<"), "the profile's label: {right}");
    assert!(
        right.contains("Marketing (registry)"),
        "the host's label wins: {right}"
    );
    assert!(
        right.contains(">urn:unknown:value<"),
        "no label anywhere: {right}"
    );
}
