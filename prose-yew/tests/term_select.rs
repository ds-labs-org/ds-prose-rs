//! `EditConfig::term_input = TermInput::Select`: the term slots as selects
//! of the profile's terms, rendered on the server. With the default
//! (`TermInput::Free`) nothing changes: `ssr.rs` and its snapshots pin it.
#![cfg(not(target_arch = "wasm32"))]
use prose_core::edit::{EditDoc, read_model};
use prose_core::profile::{ProfileVocab, Wording};
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

fn file(dir: &str, name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../prose-core/tests/{dir}/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn wording_of(text: &str) -> Rc<Wording> {
    Rc::new(Wording::new().with_profile(ProfileVocab::parse(text).unwrap()))
}

fn closed() -> Rc<Wording> {
    wording_of(&file("profiles", "research-closed.jsonld"))
}

fn doc(text: &str) -> Rc<EditDoc> {
    Rc::new(read_model(text).unwrap())
}

fn selects(config: EditConfig) -> EditConfig {
    EditConfig {
        term_input: TermInput::Select,
        ..config
    }
}

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

const FINE_TUNE: &str = "policy[0].permission[0].action[0]#name";
const DISTIL: &str = "policy[0].permission[1].action[0]#name";
const FOREIGN: &str = "policy[0].permission[2].action[0]#name";
const COUNT_LEFT: &str = "policy[0].permission[0].action[0].refinement[0]#leftOperand";
const COUNT_RIGHT: &str = "policy[0].permission[0].action[0].refinement[0]#rightOperand[0]";
const PURPOSE_LEFT: &str = "policy[0].permission[0].constraint[0]#leftOperand";
const PURPOSE_RIGHT: &str = "policy[0].permission[0].constraint[0]#rightOperand[0]";
const KIND: &str = "policy[0]#kind";
const UID: &str = "policy[0]#uid";

#[tokio::test]
async fn free_is_the_default_and_renders_no_term_select() {
    assert_eq!(EditConfig::default().term_input, TermInput::Free);
    let cfg = EditConfig::default().with_wording(closed());
    let html = ssr(doc(&file("strict", "violating.json")), cfg).await;
    assert!(!html.contains("prose-term-select"), "{html}");
    let action = element(&html, FINE_TUNE);
    assert!(action.starts_with("<span"), "{action}");
    assert!(action.contains("contenteditable"), "{action}");
}

#[tokio::test]
async fn term_slots_become_selects_of_the_profile_terms_grouped_by_profile() {
    let cfg = selects(EditConfig::default().with_wording(closed()));
    let html = ssr(doc(&file("strict", "violating.json")), cfg).await;
    for slot in [FINE_TUNE, DISTIL, FOREIGN, COUNT_LEFT, PURPOSE_LEFT, KIND] {
        let el = element(&html, slot);
        assert!(el.starts_with("<select"), "{slot}: {el}");
        assert!(el.contains("prose-term-select"), "{slot}: {el}");
        assert!(!el.contains("contenteditable"), "{slot}: {el}");
    }
    let action = element(&html, FINE_TUNE);
    // The profile's actions under its title, then the core ones; labels
    // shown, the written form as the value.
    let profile = action
        .find("<optgroup label=\"From profile: Research Data Profile\"")
        .unwrap_or_else(|| panic!("{action}"));
    let core = action
        .find("<optgroup label=\"ODRL core\"")
        .unwrap_or_else(|| panic!("{action}"));
    assert!(profile < core, "{action}");
    assert!(action.contains(">Fine-tune</option>"), "{action}");
    assert!(action.contains("value=\"use\""), "{action}");
    // The stored CURIE selects the profile term, which the policy binds.
    assert!(
        action.contains("value=\"ex:fineTune\" selected=\"selected\""),
        "{action}"
    );
    assert!(!action.contains("data-outside"), "{action}");
}

#[tokio::test]
async fn a_stored_term_outside_the_list_is_kept_selected_and_flagged() {
    let cfg = selects(EditConfig::default().with_wording(closed()));
    let html = ssr(doc(&file("strict", "violating.json")), cfg).await;
    let el = element(&html, FOREIGN);
    assert!(el.contains("data-outside=\"true\""), "{el}");
    assert!(
        el.contains(
            "<option value=\"foo:bar\" selected=\"selected\">foo:bar (not in the list)</option>"
        ),
        "{el}"
    );
    let note = format!("prose-note-{FOREIGN}");
    assert!(el.contains(&format!("aria-describedby=\"{note}\"")), "{el}");
    assert!(html.contains(&format!("id=\"{note}\"")), "{html}");
}

#[tokio::test]
async fn right_operands_are_selects_only_where_the_profile_lists_values() {
    let cfg = selects(EditConfig::default().with_wording(closed()));
    let html = ssr(doc(&file("strict", "violating.json")), cfg).await;
    // The research purpose lists its values.
    let purpose = element(&html, PURPOSE_RIGHT);
    assert!(purpose.starts_with("<select"), "{purpose}");
    assert!(purpose.contains(">Academic research</option>"), "{purpose}");
    assert!(
        purpose.contains("value=\"ex:academic\" selected=\"selected\""),
        "{purpose}"
    );
    // A narrower value is indented under the broader one.
    assert!(
        purpose.contains(">\u{a0}\u{a0}\u{a0}Clinical research</option>"),
        "{purpose}"
    );
    // A count is a literal: free text, as are the uid and the parties.
    for slot in [COUNT_RIGHT, UID] {
        let el = element(&html, slot);
        assert!(el.starts_with("<span"), "{slot}: {el}");
        assert!(el.contains("contenteditable"), "{slot}: {el}");
    }
}

#[tokio::test]
async fn strict_validation_lists_only_the_terms_it_takes() {
    let free = selects(EditConfig::default().with_wording(closed()));
    let strict = selects(
        EditConfig::default()
            .with_wording(closed())
            .with_strict(Options::strict()),
    );
    let text = file("strict", "violating.json");
    let open = element(&ssr(doc(&text), free).await, FINE_TUNE);
    let html = ssr(doc(&text), strict).await;
    let closed = element(&html, FINE_TUNE);
    let n = |s: &str| s.matches("<option").count();
    assert!(n(&closed) > 0 && n(&closed) <= n(&open), "{closed}");
    // What the validator refuses is not offered; the stored value still is.
    let foreign = element(&html, FOREIGN);
    assert!(foreign.contains("data-outside=\"true\""), "{foreign}");
    assert!(!foreign.contains("value=\"foo:bar\">"), "{foreign}");
}

const EX: &str = "https://example.org/dep/";

fn deprecating_profile() -> Rc<Wording> {
    wording_of(&format!(
        r#"{{
            "@context": {{"odrl": "http://www.w3.org/ns/odrl/2/", "ex": "{EX}",
                         "owl": "http://www.w3.org/2002/07/owl#",
                         "rdfs": "http://www.w3.org/2000/01/rdf-schema#"}},
            "@id": "{EX}profile",
            "@graph": [
                {{"@id": "ex:current", "@type": "odrl:Action", "rdfs:label": "Current"}},
                {{"@id": "ex:old", "@type": "odrl:Action", "rdfs:label": "Old",
                  "owl:deprecated": true}}
            ]
        }}"#
    ))
}

const DEP_POLICY: &str = r#"{"@context":"http://www.w3.org/ns/odrl.jsonld","@type":"Set","uid":"urn:p","permission":[{"target":"urn:a","action":"use"}]}"#;

#[tokio::test]
async fn deprecated_terms_are_listed_last_unless_the_validator_denies_them() {
    let action = "policy[0].permission[0].action[0]#name";
    let cfg = selects(EditConfig::default().with_wording(deprecating_profile()));
    let el = element(&ssr(doc(DEP_POLICY), cfg).await, action);
    let current = el
        .find(">Current</option>")
        .unwrap_or_else(|| panic!("{el}"));
    let group = el
        .find("<optgroup label=\"Deprecated in the profile\"")
        .unwrap_or_else(|| panic!("{el}"));
    let old = el.find(">Old</option>").unwrap_or_else(|| panic!("{el}"));
    assert!(current < group && group < old, "{el}");
    assert!(el.trim_end().ends_with("</optgroup></select>"), "{el}");

    let denied = selects(
        EditConfig::default()
            .with_wording(deprecating_profile())
            .with_strict(Options::strict().deny_deprecated(true)),
    );
    let el = element(&ssr(doc(DEP_POLICY), denied).await, action);
    assert!(el.contains(">Current</option>"), "{el}");
    assert!(!el.contains(">Old</option>"), "{el}");
    assert!(!el.contains("Deprecated in the profile"), "{el}");
}

#[tokio::test]
async fn read_mode_is_unchanged() {
    let text = file("strict", "violating.json");
    let read = |term_input| {
        let cfg = EditConfig {
            term_input,
            ..EditConfig::default().with_wording(closed())
        };
        let doc = doc(&text);
        async move {
            let local = tokio::task::LocalSet::new();
            local
                .run_until(
                    yew::LocalServerRenderer::<OdrlProseView>::with_props(yew::props!(
                        prose_yew::OdrlProseViewProps {
                            doc,
                            mode: EditMode::Read,
                            config: Rc::new(cfg),
                        }
                    ))
                    .render(),
                )
                .await
        }
    };
    assert_eq!(read(TermInput::Free).await, read(TermInput::Select).await);
}
