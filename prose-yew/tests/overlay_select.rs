//! The operator overlay in the editor (`Wording::with_overlay`): a right
//! operand an overlay closes with `prose:closed` is a select of exactly the
//! values it lists, as for a shape (see `shapes_select.rs`). Rendered on the
//! server.
#![cfg(not(target_arch = "wasm32"))]
use prose_core::edit::{EditDoc, read_model};
use prose_core::profile::{Overlay, ProfileVocab, Wording};
use prose_core::validate::Options;
use prose_yew::{EditConfig, EditMode, OdrlProseView, TermInput};
use std::rc::Rc;

async fn ssr(doc: Rc<EditDoc>, config: EditConfig) -> String {
    let local = tokio::task::LocalSet::new();
    let config = Rc::new(config);
    local
        .run_until(
            yew::LocalServerRenderer::<OdrlProseView>::with_props(yew::props!(
                prose_yew::OdrlProseViewProps {
                    doc,
                    mode: EditMode::Edit,
                    config,
                }
            ))
            .render(),
        )
        .await
}

fn file(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../prose-core/tests/shapes/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn wording(overlay: bool) -> Rc<Wording> {
    let w = Wording::new()
        .with_profile(ProfileVocab::parse(&file("dataspace-profile.jsonld")).unwrap());
    let text = std::fs::read_to_string(format!(
        "{}/../prose-core/tests/overlay/dataspace-overlay.jsonld",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    Rc::new(match overlay {
        true => w.with_overlay(Overlay::parse(&text).unwrap()),
        false => w,
    })
}

fn config(overlay: bool, strict: bool) -> EditConfig {
    let c = EditConfig {
        term_input: TermInput::Select,
        ..EditConfig::default().with_wording(wording(overlay))
    };
    match strict {
        true => c.with_strict(Options::strict()),
        false => c,
    }
}

const POLICY: &str = r#"{
  "@context": ["http://www.w3.org/ns/odrl.jsonld", {"dx": "https://example.org/dataspace/"}],
  "@type": "Set", "uid": "urn:p", "profile": "https://example.org/dataspace/profile",
  "permission": [{"target": "urn:a", "action": "dx:train", "constraint": [
    {"leftOperand": "dx:purpose", "operator": "eq", "rightOperand": {"@id": "dx:research"}},
    {"leftOperand": "dx:price", "operator": "eq", "rightOperand": {"@id": "dx:research"}}
  ]}]
}"#;

const ACTION: &str = "policy[0].permission[0].action[0]#name";
const LEFT: &str = "policy[0].permission[0].constraint[0]#leftOperand";
const RIGHT: &str = "policy[0].permission[0].constraint[0]#rightOperand[0]";
const OTHER_RIGHT: &str = "policy[0].permission[0].constraint[1]#rightOperand[0]";

/// The whole element that carries `data-slot="{slot}"`, through its end tag.
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

fn doc() -> Rc<EditDoc> {
    Rc::new(read_model(POLICY).unwrap())
}

#[tokio::test]
async fn a_right_operand_the_overlay_closes_is_a_select_of_its_values() {
    let html = ssr(doc(), config(true, false)).await;
    let right = element(&html, RIGHT);
    assert!(right.starts_with("<select"), "{right}");
    assert!(right.contains("value=\"dx:research\""), "{right}");
    assert!(right.contains("value=\"dx:marketing\""), "{right}");
    // The overlay lists exactly its values: dx:clinical is narrower, not listed.
    assert!(!right.contains("dx:clinical"), "{right}");
    let other = element(&html, OTHER_RIGHT);
    assert!(!other.starts_with("<select"), "{other}");
}

#[tokio::test]
async fn without_the_overlay_the_same_right_operand_is_free_text() {
    let html = ssr(doc(), config(false, false)).await;
    assert!(!element(&html, RIGHT).starts_with("<select"));
}

#[tokio::test]
async fn an_overlay_does_not_close_the_term_lists() {
    // Only shapes close the action and left-operand lists for the policy.
    let html = ssr(doc(), config(true, true)).await;
    assert!(element(&html, ACTION).contains("value=\"dx:publish\""));
    assert!(element(&html, LEFT).contains("value=\"dx:price\""));
}

#[tokio::test]
async fn a_stored_value_outside_the_overlay_list_stays_selected_and_noted() {
    let text = POLICY.replacen(
        "\"rightOperand\": {\"@id\": \"dx:research\"}",
        "\"rightOperand\": {\"@id\": \"dx:clinical\"}",
        1,
    );
    let html = ssr(Rc::new(read_model(&text).unwrap()), config(true, true)).await;
    let right = element(&html, RIGHT);
    assert!(right.contains("data-outside=\"true\""), "{right}");
    assert!(
        right.contains("<option value=\"dx:clinical\" selected=\"selected\">"),
        "{right}"
    );
}
