//! Companion SHACL shapes in the editor (`Wording::with_shapes`): a right
//! operand a shape closes with `sh:in` is a select of exactly those values,
//! and a strict validator narrows the action and left-operand selects to the
//! lists the shapes close. Rendered on the server.
#![cfg(not(target_arch = "wasm32"))]
use prose_core::edit::{EditDoc, read_model};
use prose_core::profile::{ProfileVocab, Wording};
use prose_core::shapes::ShapesGraph;
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

fn wording(shapes: bool) -> Rc<Wording> {
    let w = Wording::new()
        .with_profile(ProfileVocab::parse(&file("dataspace-profile.jsonld")).unwrap());
    Rc::new(match shapes {
        true => w.with_shapes(ShapesGraph::parse(&file("dataspace-shapes.jsonld")).unwrap()),
        false => w,
    })
}

fn config(shapes: bool, strict: bool) -> EditConfig {
    let c = EditConfig {
        term_input: TermInput::Select,
        ..EditConfig::default().with_wording(wording(shapes))
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
async fn a_right_operand_the_shapes_close_is_a_select_of_its_values() {
    let html = ssr(doc(), config(true, false)).await;
    let right = element(&html, RIGHT);
    assert!(right.starts_with("<select"), "{right}");
    assert!(right.contains("value=\"dx:research\""), "{right}");
    assert!(right.contains("value=\"dx:marketing\""), "{right}");
    // dx:clinical is narrower than dx:research, but sh:in is its list.
    assert!(!right.contains("dx:clinical"), "{right}");
    assert!(!right.contains("dx:member"), "{right}");
    // A left operand no shape selects keeps its free-text slot.
    let other = element(&html, OTHER_RIGHT);
    assert!(!other.starts_with("<select"), "{other}");
}

#[tokio::test]
async fn without_the_shapes_the_same_right_operand_is_free_text() {
    let html = ssr(doc(), config(false, false)).await;
    assert!(!element(&html, RIGHT).starts_with("<select"));
}

#[tokio::test]
async fn a_strict_validator_narrows_the_term_selects_to_the_closed_lists() {
    let html = ssr(doc(), config(true, true)).await;
    let action = element(&html, ACTION);
    assert!(action.contains("value=\"dx:train\""), "{action}");
    assert!(action.contains("value=\"dx:share\""), "{action}");
    assert!(!action.contains("value=\"dx:publish\""), "{action}");
    let left = element(&html, LEFT);
    assert!(left.contains("value=\"dx:purpose\""), "{left}");
    assert!(!left.contains("value=\"dx:price\""), "{left}");
    // Without the shapes the whole vocabulary is offered.
    let html = ssr(doc(), config(false, true)).await;
    assert!(element(&html, ACTION).contains("value=\"dx:publish\""));
    assert!(element(&html, LEFT).contains("value=\"dx:price\""));
}

#[tokio::test]
async fn a_stored_value_outside_the_shapes_list_stays_selected_and_noted() {
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

const OPERATOR: &str = "policy[0].permission[0].constraint[0]#operator";

#[tokio::test]
async fn the_operator_groups_name_the_shapes_when_they_gave_the_list() {
    let html = ssr(doc(), config(true, true)).await;
    let op = element(&html, OPERATOR);
    assert!(
        op.contains("<optgroup label=\"Allowed by the shapes\">"),
        "{op}"
    );
    assert!(
        op.contains("<optgroup label=\"Not allowed by the shapes\" disabled"),
        "{op}"
    );
    assert!(!op.contains("by the profile"), "{op}");
}

// --- literal members of sh:in ----------------------------------------------------

const LITERAL_SHAPES: &str = r#"{
  "@context": {"sh": "http://www.w3.org/ns/shacl#", "odrl": "http://www.w3.org/ns/odrl/2/",
               "dx": "https://example.org/dataspace/"},
  "@graph": [
    {"@id": "dx:CodeShape", "@type": "sh:NodeShape", "sh:property": [
      {"sh:path": {"@id": "odrl:leftOperand"}, "sh:hasValue": {"@id": "dx:code"}},
      {"sh:path": {"@id": "odrl:rightOperand"}, "sh:in": {"@list": ["ABC-1", "DEF-2"]}}
    ]},
    {"@id": "dx:PurposeShape", "@type": "sh:NodeShape", "sh:property": [
      {"sh:path": {"@id": "odrl:leftOperand"}, "sh:hasValue": {"@id": "dx:purpose"}},
      {"sh:path": {"@id": "odrl:rightOperand"}, "sh:in": {"@list": [{"@id": "dx:research"}, "other"]}}
    ]}
  ]
}"#;

fn literal_config(strict: bool) -> EditConfig {
    let w = Wording::new()
        .with_profile(ProfileVocab::parse(&file("dataspace-profile.jsonld")).unwrap())
        .with_shapes(ShapesGraph::parse(LITERAL_SHAPES).unwrap());
    let c = EditConfig {
        term_input: TermInput::Select,
        ..EditConfig::default().with_wording(Rc::new(w))
    };
    match strict {
        true => c.with_strict(Options::strict()),
        false => c,
    }
}

fn code_policy(value: &str) -> Rc<EditDoc> {
    let text = format!(
        r#"{{
  "@context": ["http://www.w3.org/ns/odrl.jsonld", {{"dx": "https://example.org/dataspace/"}}],
  "@type": "Set", "uid": "urn:p", "profile": "https://example.org/dataspace/profile",
  "permission": [{{"target": "urn:a", "action": "dx:train", "constraint": [
    {{"leftOperand": "dx:code", "operator": "eq", "rightOperand": {value}}},
    {{"leftOperand": "dx:purpose", "operator": "eq", "rightOperand": {{"@id": "dx:research"}}}}
  ]}}]
}}"#
    );
    Rc::new(read_model(&text).unwrap())
}

#[tokio::test]
async fn a_right_operand_the_shapes_close_to_literals_is_a_select_of_them() {
    let html = ssr(code_policy("\"ABC-1\""), literal_config(false)).await;
    let right = element(&html, RIGHT);
    assert!(right.starts_with("<select"), "{right}");
    assert!(
        right.contains("<option value=\"ABC-1\" selected=\"selected\">"),
        "{right}"
    );
    assert!(right.contains("<option value=\"DEF-2\""), "{right}");
    assert!(!right.contains("data-outside"), "{right}");
}

#[tokio::test]
async fn a_list_with_iris_and_literals_offers_both() {
    let html = ssr(code_policy("\"ABC-1\""), literal_config(true)).await;
    let right = element(&html, OTHER_RIGHT);
    assert!(right.starts_with("<select"), "{right}");
    assert!(right.contains("value=\"dx:research\""), "{right}");
    assert!(right.contains("<option value=\"other\""), "{right}");
}

#[tokio::test]
async fn a_literal_outside_the_list_stays_selected_and_noted() {
    let html = ssr(code_policy("\"XYZ-9\""), literal_config(true)).await;
    let right = element(&html, RIGHT);
    assert!(right.contains("data-outside=\"true\""), "{right}");
    assert!(
        right.contains("<option value=\"XYZ-9\" selected=\"selected\">"),
        "{right}"
    );
}

#[tokio::test]
async fn without_the_literal_shapes_the_code_is_free_text() {
    let html = ssr(code_policy("\"ABC-1\""), config(true, false)).await;
    // The fixture shapes give dx:code a pattern only: no list, no select.
    assert!(!element(&html, RIGHT).starts_with("<select"));
}

#[tokio::test]
async fn advisory_validation_does_not_narrow_the_term_selects_to_the_closed_lists() {
    // The shapes close the action and left-operand lists as an unknown
    // term, which binds in strict mode only: advisory validation (explicit
    // or none) offers the whole vocabulary, as it does for a profile's own
    // closed lists.
    let advisory = config(true, false).with_strict(Options::advisory());
    for config in [config(true, false), advisory] {
        let html = ssr(doc(), config).await;
        let action = element(&html, ACTION);
        assert!(action.contains("value=\"dx:publish\""), "{action}");
        assert!(action.contains("value=\"dx:train\""), "{action}");
        let left = element(&html, LEFT);
        assert!(left.contains("value=\"dx:price\""), "{left}");
        assert!(left.contains("value=\"dx:purpose\""), "{left}");
    }
}
