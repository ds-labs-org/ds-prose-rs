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
        el.contains("<option value=\"foo:bar\" selected=\"selected\">foo:bar</option>"),
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

#[tokio::test]
async fn an_open_left_operand_keeps_its_listed_values_as_suggestions() {
    // research.jsonld lists the purposes without closing the operand.
    let open = wording_of(&file("profiles", "research.jsonld"));
    let cfg = selects(EditConfig::default().with_wording(open));
    let html = ssr(doc(&file("strict", "violating.json")), cfg).await;
    let purpose = element(&html, PURPOSE_RIGHT);
    assert!(purpose.starts_with("<span"), "{purpose}");
    assert!(purpose.contains("contenteditable"), "{purpose}");
    // The term slots are selects all the same.
    assert!(
        element(&html, PURPOSE_LEFT).starts_with("<select"),
        "{html}"
    );
}

#[tokio::test]
async fn without_a_profile_the_note_says_not_in_the_list() {
    let html = ssr(
        doc(&file("strict", "violating.json")),
        selects(EditConfig::default()),
    )
    .await;
    let note = format!("id=\"prose-note-{FOREIGN}\"");
    let at = html.find(&note).unwrap_or_else(|| panic!("{html}"));
    let rest = &html[at..];
    let text = &rest[rest.find('>').unwrap() + 1..rest.find("</small>").unwrap()];
    assert_eq!(text, "(not in the list)");
    assert!(!html.contains("(not in the profile)"), "{html}");
}

#[tokio::test]
async fn a_select_takes_the_error_and_warning_classes_of_a_slot() {
    use prose_core::edit::{Issue, IssueTarget, Severity};
    let issue = |slot: &str, severity| Issue {
        target: IssueTarget::Slot(slot.parse().unwrap()),
        severity,
        message: "x".into(),
    };
    let issues = Rc::new(vec![
        issue(FOREIGN, Severity::Error),
        issue(DISTIL, Severity::Warning),
    ]);
    let config = Rc::new(selects(EditConfig::default().with_wording(closed())));
    let doc = doc(&file("strict", "violating.json"));
    let local = tokio::task::LocalSet::new();
    let html = local
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
        .await;
    let error = element(&html, FOREIGN);
    assert!(error.contains("prose-slot-invalid"), "{error}");
    assert!(error.contains("aria-invalid=\"true\""), "{error}");
    let warning = element(&html, DISTIL);
    assert!(warning.contains("prose-slot-warning"), "{warning}");
    assert!(!warning.contains("aria-invalid"), "{warning}");
}

#[tokio::test]
async fn a_value_under_a_deprecated_one_is_not_left_indented() {
    let ex = "https://example.org/tree/";
    let profile = wording_of(&format!(
        r#"{{
            "@context": {{"odrl": "http://www.w3.org/ns/odrl/2/", "ex": "{ex}",
                         "owl": "http://www.w3.org/2002/07/owl#",
                         "skos": "http://www.w3.org/2004/02/skos/core#",
                         "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
                         "prose": "https://ds-labs-org.github.io/ds-prose-rs/profile#"}},
            "@id": "{ex}profile",
            "@graph": [
                {{"@id": "ex:lo", "@type": "odrl:LeftOperand", "rdfs:label": "Lo",
                  "prose:rightOperand": [{{"@id": "ex:old"}}, {{"@id": "ex:other"}}],
                  "prose:closed": true}},
                {{"@id": "ex:old", "@type": "odrl:RightOperand", "rdfs:label": "Old",
                  "owl:deprecated": true}},
                {{"@id": "ex:child", "@type": "odrl:RightOperand", "rdfs:label": "Child",
                  "skos:broader": {{"@id": "ex:old"}}}},
                {{"@id": "ex:other", "@type": "odrl:RightOperand", "rdfs:label": "Other"}}
            ]
        }}"#
    ));
    let policy = format!(
        r#"{{"@context":["http://www.w3.org/ns/odrl.jsonld",{{"ex":"{ex}"}}],"@type":"Set","uid":"urn:p","permission":[{{"target":"urn:a","action":"use","constraint":[{{"leftOperand":"ex:lo","operator":"eq","rightOperand":{{"@id":"ex:other"}}}}]}}]}}"#
    );
    let html = ssr(
        doc(&policy),
        selects(EditConfig::default().with_wording(profile)),
    )
    .await;
    let el = element(&html, PURPOSE_RIGHT);
    assert!(el.starts_with("<select"), "{el}");
    assert!(el.contains(">Child</option>"), "{el}");
    assert!(!el.contains("\u{a0}Child"), "{el}");
    assert!(
        el.contains("<optgroup label=\"Deprecated in the profile\""),
        "{el}"
    );
}
