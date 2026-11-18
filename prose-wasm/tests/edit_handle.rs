//! `ProseEditorHandle`, mounted in headless Chrome the way a JS host uses
//! it. Run with `cargo test -p prose-wasm --target wasm32-unknown-unknown
//! --test edit_handle`.
#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use prose_wasm::ProseEditorHandle;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;
use web_sys::{Element, HtmlElement};

mod common;
use common::{CLASS_SHAPES, chain_profile};

wasm_bindgen_test_configure!(run_in_browser);

const RESEARCH: &str = include_str!("../../prose-core/tests/profiles/research.jsonld");
const CLOSED: &str = include_str!("../../prose-core/tests/profiles/research-closed.jsonld");

const ONE: &str = r#"{"@context":"http://www.w3.org/ns/odrl.jsonld","@type":"Set","uid":"urn:policy:1","permission":[{"target":"urn:asset:1","action":"use"}]}"#;
const OTHER: &str = r#"{"@context":"http://www.w3.org/ns/odrl.jsonld","@type":"Set","uid":"urn:policy:2","prohibition":[{"target":"urn:asset:2","action":"distribute"}]}"#;
const FINE: &str = r#"{"@context":["http://www.w3.org/ns/odrl.jsonld",{"ex":"https://example.org/odrl/profile/research/"}],"@type":"Set","uid":"urn:policy:4","permission":[{"target":"urn:asset:1","action":"ex:fineTune"}]}"#;
const FOREIGN: &str = r#"{"@context":["http://www.w3.org/ns/odrl.jsonld",{"foo":"https://example.net/foo/"}],"@type":"Set","uid":"urn:policy:3","profile":"https://example.org/odrl/profile/research","permission":[{"target":"urn:asset:1","action":"foo:bar"}]}"#;

type Calls = Rc<RefCell<Vec<(String, Option<String>)>>>;

async fn settle() {
    for _ in 0..4 {
        TimeoutFuture::new(0).await;
    }
}

fn js(text: &str) -> JsValue {
    js_sys::JSON::parse(text).expect("JSON")
}

fn container() -> Element {
    let document = web_sys::window().unwrap().document().unwrap();
    let c = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&c).unwrap();
    c
}

fn recorder() -> (Calls, JsValue) {
    let calls: Calls = Rc::default();
    let sink = calls.clone();
    let f = Closure::<dyn FnMut(String, JsValue)>::new(move |json: String, report: JsValue| {
        sink.borrow_mut().push((json, report.as_string()));
    });
    let f: JsValue = f.into_js_value();
    (calls, f)
}

async fn mount(json: &str) -> (Element, ProseEditorHandle, Calls) {
    let c = container();
    let (calls, f) = recorder();
    let h = ProseEditorHandle::new(c.clone(), json, None, f).expect("mounts");
    settle().await;
    (c, h, calls)
}

fn select(c: &Element, css: &str) -> Option<Element> {
    c.query_selector(css).unwrap()
}

async fn click(c: &Element, css: &str) {
    let b = select(c, css).unwrap_or_else(|| panic!("{css} in {}", c.inner_html()));
    b.dyn_into::<HtmlElement>().unwrap().click();
    settle().await;
}

fn count(json: &str, key: &str) -> usize {
    let v: serde_json::Value = serde_json::from_str(json).unwrap();
    v[key].as_array().map_or(0, Vec::len)
}

const ADD_PERMISSION: &str = "[data-add=\"policy[0]@permission\"]";

#[wasm_bindgen_test]
async fn mounting_renders_the_editor_and_does_not_call_back() {
    let (c, _h, calls) = mount(ONE).await;
    assert!(select(&c, ".prose-edit").is_some(), "{}", c.inner_html());
    assert!(calls.borrow().is_empty());
}

#[wasm_bindgen_test]
async fn an_edit_calls_back_once_with_the_json_ld() {
    let (c, h, calls) = mount(ONE).await;
    click(&c, ADD_PERMISSION).await;
    let recorded = calls.borrow().clone();
    assert_eq!(recorded.len(), 1, "{recorded:?}");
    let (json, report) = &recorded[0];
    assert_eq!(count(json, "permission"), 2, "{json}");
    assert_eq!(*report, None, "validation is off");
    assert_eq!(&h.json(), json, "json() is the text last emitted");
}

#[wasm_bindgen_test]
async fn update_with_the_emitted_text_is_a_no_op_and_another_policy_replaces() {
    let (c, mut h, calls) = mount(ONE).await;
    click(&c, ADD_PERMISSION).await;
    let emitted = calls.borrow()[0].0.clone();
    let before = c.inner_html();
    h.update(&emitted, None).expect("the echo");
    settle().await;
    assert_eq!(c.inner_html(), before, "the echo keeps the DOM");
    assert_eq!(calls.borrow().len(), 1);

    h.update(OTHER, None).expect("another policy");
    settle().await;
    assert!(select(&c, "[data-add=\"policy[0]@prohibition\"]").is_some());
    assert!(
        c.text_content().unwrap().contains("distribute"),
        "{}",
        c.inner_html()
    );
    assert_eq!(
        calls.borrow().len(),
        1,
        "a document the host set is not reported"
    );
    assert_eq!(h.json(), OTHER);
    // An edit after that is reported again.
    click(&c, "[data-add=\"policy[0]@prohibition\"]").await;
    assert_eq!(calls.borrow().len(), 2);
    assert_eq!(count(&calls.borrow()[1].0, "prohibition"), 2);
}

#[wasm_bindgen_test]
async fn unreadable_json_throws_and_changes_nothing() {
    let (c, mut h, _calls) = mount(ONE).await;
    let before = c.inner_html();
    assert!(h.update("not json", None).is_err());
    settle().await;
    assert_eq!(c.inner_html(), before);
    assert_eq!(h.json(), ONE);
    let (_, f) = recorder();
    assert!(ProseEditorHandle::new(container(), "{}", None, f).is_err());
}

#[wasm_bindgen_test]
async fn limits_hide_the_controls_and_bad_limits_throw() {
    let (c, mut h, _calls) = mount(ONE).await;
    assert!(select(&c, "[data-add=\"policy[0]@assigner\"]").is_some());
    h.set_limits(js(
        r#"{"policies":{"min":1,"max":1},"policyAssigner":{"max":0},"policyAssignee":{"max":0}}"#,
    ))
    .expect("limits");
    settle().await;
    for css in [
        "[data-add=\"policies\"]",
        "[data-add=\"policy[0]@assigner\"]",
        "[data-add=\"policy[0]@assignee\"]",
    ] {
        assert!(select(&c, css).is_none(), "{css}");
    }
    assert!(select(&c, ADD_PERMISSION).is_some());
    assert!(h.set_limits(js(r#"{"bogus":{}}"#)).is_err());
    h.set_limits(JsValue::NULL).expect("reset");
    settle().await;
    assert!(select(&c, "[data-add=\"policy[0]@assigner\"]").is_some());
}

#[wasm_bindgen_test]
async fn set_profile_words_the_slots_and_keeps_the_document() {
    let (c, mut h, calls) = mount(FINE).await;
    let slot = "[data-slot=\"policy[0].permission[0].action[0]#name\"]";
    let before = select(&c, slot)
        .expect("the action slot")
        .get_attribute("title")
        .unwrap();
    let summary = h.set_profile(Some(RESEARCH.to_string())).expect("loads");
    assert!(summary.contains("\"Research Data Profile\""), "{summary}");
    settle().await;
    let after = select(&c, slot)
        .expect("the action slot")
        .get_attribute("title")
        .unwrap();
    assert_ne!(before, after, "the profile explains the term in the title");
    assert!(calls.borrow().is_empty(), "a profile is not an edit");
    assert!(h.set_profile(Some("<!doctype html><p>x".into())).is_err());
    assert_eq!(h.set_profile(None).unwrap(), "null");
}

#[wasm_bindgen_test]
async fn validation_shows_problems_and_reports_with_each_edit() {
    let (c, mut h, calls) = mount(FOREIGN).await;
    h.set_profile(Some(CLOSED.to_string())).expect("profile");
    assert!(h.report().is_null(), "validation is off");
    h.set_validation(js("{}")).expect("validation");
    settle().await;
    assert!(
        select(&c, ".prose-problems").is_some(),
        "{}",
        c.inner_html()
    );
    let report = h.report().as_string().expect("a report");
    assert!(report.contains("ds-prose/validation-report@1"), "{report}");
    assert!(report.contains("profile.unknown-term"), "{report}");
    assert!(calls.borrow().is_empty());

    click(&c, ADD_PERMISSION).await;
    let recorded = calls.borrow().clone();
    assert_eq!(recorded.len(), 1);
    let sent = recorded[0].1.as_deref().expect("a report with the edit");
    assert!(sent.contains("ds-prose/validation-report@1"), "{sent}");

    h.set_validation(JsValue::NULL).expect("off");
    settle().await;
    assert!(h.report().is_null());
    assert!(select(&c, ".prose-problems").is_none());
    assert!(h.set_validation(js(r#"{"nope":1}"#)).is_err());
}

#[wasm_bindgen_test]
async fn destroy_removes_the_dom_and_is_idempotent() {
    let (c, mut h, _calls) = mount(ONE).await;
    h.destroy();
    h.destroy();
    settle().await;
    assert!(select(&c, ".prose-edit").is_none());
}

#[wasm_bindgen_test]
async fn the_echo_keeps_the_editor_state_but_another_document_resets_it() {
    let (c, mut h, calls) = mount(ONE).await;
    // An empty policy type is refused; the editor shows why. A document
    // set by the host clears that state, so it tells a reset from a no-op.
    let kind = select(&c, "[data-slot=\"policy[0]#kind\"]")
        .expect("the type slot")
        .dyn_into::<HtmlElement>()
        .unwrap();
    kind.focus().unwrap();
    kind.set_text_content(Some(""));
    let input = web_sys::InputEventInit::new();
    input.set_bubbles(true);
    kind.dispatch_event(&web_sys::InputEvent::new_with_event_init_dict("input", &input).unwrap())
        .unwrap();
    let out = web_sys::FocusEventInit::new();
    out.set_bubbles(true);
    kind.dispatch_event(
        &web_sys::FocusEvent::new_with_focus_event_init_dict("focusout", &out).unwrap(),
    )
    .unwrap();
    settle().await;
    assert!(
        select(&c, ".prose-error[role=\"alert\"]").is_some(),
        "the refusal: {}",
        c.inner_html()
    );
    assert!(calls.borrow().is_empty(), "a refused edit is not reported");

    h.update(ONE, None).expect("the echo");
    settle().await;
    assert!(
        select(&c, ".prose-error[role=\"alert\"]").is_some(),
        "the echo changed nothing"
    );

    h.update(OTHER, None).expect("another document");
    settle().await;
    assert!(
        select(&c, ".prose-error[role=\"alert\"]").is_none(),
        "another document starts clean"
    );
}

#[wasm_bindgen_test]
async fn go_to_moves_the_focus_to_the_finding() {
    let (c, mut h, _calls) = mount(FOREIGN).await;
    h.set_profile(Some(CLOSED.to_string())).expect("profile");
    h.set_validation(js("{}")).expect("validation");
    settle().await;
    let go = select(&c, ".prose-problem-go").expect("a Go to button");
    go.dyn_into::<HtmlElement>().unwrap().click();
    settle().await;
    let active = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .active_element()
        .expect("something has the focus");
    assert!(
        select(&c, ".prose-edit").unwrap().contains(Some(&active)),
        "the focus is in the editor, not on the button: {}",
        active.outer_html()
    );
    assert!(
        active.has_attribute("data-slot") || active.has_attribute("data-node"),
        "{}",
        active.outer_html()
    );
}

#[wasm_bindgen_test]
async fn dropping_the_handle_unmounts_the_editor() {
    let (c, h, _calls) = mount(ONE).await;
    drop(h);
    settle().await;
    assert!(select(&c, ".prose-edit").is_none());
}

const NO_RULES_DOC: &str = r#"{"@context":"http://www.w3.org/ns/odrl.jsonld","@type":"Set"}"#;

#[wasm_bindgen_test]
async fn the_no_rule_note_goes_once_a_rule_is_added() {
    let (c, _h, _calls) = mount(NO_RULES_DOC).await;
    assert!(
        select(&c, ".prose-warnings").is_some(),
        "{}",
        c.inner_html()
    );
    click(&c, ADD_PERMISSION).await;
    assert!(
        select(&c, ".prose-warnings").is_none(),
        "{}",
        c.inner_html()
    );
}

#[wasm_bindgen_test]
async fn an_advisory_validation_without_findings_shows_no_problems_heading() {
    let (c, mut h, _calls) = mount(ONE).await;
    h.set_validation(js(r#"{"strict":false}"#))
        .expect("validation");
    settle().await;
    assert!(
        select(&c, ".prose-problems-heading").is_none(),
        "{}",
        c.inner_html()
    );
    assert!(select(&c, ".prose-problems").is_none());
}

#[wasm_bindgen_test]
async fn a_policy_uid_limit_of_zero_hides_the_identifier() {
    let (c, mut h, _calls) = mount(NO_RULES_DOC).await;
    assert!(select(&c, "[data-slot=\"policy[0]#uid\"]").is_some());
    h.set_limits(js(r#"{"policyUid":{"max":0}}"#))
        .expect("limits");
    settle().await;
    assert!(select(&c, "[data-slot=\"policy[0]#uid\"]").is_none());
}

const ACTION: &str = "[data-slot=\"policy[0].permission[0].action[0]#name\"]";

#[wasm_bindgen_test]
async fn term_input_select_turns_the_term_slots_into_dropdowns() {
    let (c, mut h, calls) = mount(FINE).await;
    h.set_profile(Some(RESEARCH.to_string())).expect("profile");
    settle().await;
    // Free text by default.
    let action = select(&c, ACTION).expect("the action slot");
    assert_eq!(action.tag_name(), "SPAN");
    assert!(action.has_attribute("contenteditable"));

    h.set_term_input("select").expect("select");
    settle().await;
    let action = select(&c, ACTION)
        .expect("the action select")
        .dyn_into::<web_sys::HtmlSelectElement>()
        .unwrap();
    assert!(action.class_list().contains("prose-term-select"));
    assert_eq!(action.value(), "ex:fineTune");
    // No term slot takes typed text.
    for css in ["[data-slot=\"policy[0]#kind\"]", ACTION] {
        assert!(
            select(&c, &format!("{css}[contenteditable]")).is_none(),
            "{css}: {}",
            c.inner_html()
        );
    }
    assert!(
        calls.borrow().is_empty(),
        "switching the mode is not an edit"
    );

    // Picking an option is one SetText, reported as JSON-LD.
    action.set_value("ex:train");
    let init = web_sys::EventInit::new();
    init.set_bubbles(true);
    action
        .dispatch_event(&web_sys::Event::new_with_event_init_dict("change", &init).unwrap())
        .unwrap();
    settle().await;
    let recorded = calls.borrow().clone();
    assert_eq!(recorded.len(), 1, "{recorded:?}");
    assert!(recorded[0].0.contains("\"ex:train\""), "{}", recorded[0].0);

    // An unknown mode throws and keeps the select.
    assert!(h.set_term_input("dropdown").is_err());
    settle().await;
    assert_eq!(select(&c, ACTION).unwrap().tag_name(), "SELECT");

    // Back to free text.
    h.set_term_input("free").expect("free");
    settle().await;
    assert_eq!(select(&c, ACTION).unwrap().tag_name(), "SPAN");
}

#[wasm_bindgen_test]
async fn term_input_survives_a_new_profile_and_validation() {
    let (c, mut h, _calls) = mount(FOREIGN).await;
    h.set_term_input("select").expect("select");
    h.set_profile(Some(CLOSED.to_string())).expect("profile");
    h.set_validation(js("{}")).expect("validation");
    settle().await;
    let action = select(&c, ACTION).expect("the action select");
    assert_eq!(action.tag_name(), "SELECT", "{}", c.inner_html());
    // The stored term the profile does not define is kept and flagged.
    assert_eq!(
        action.get_attribute("data-outside").as_deref(),
        Some("true")
    );
    assert_eq!(
        action
            .dyn_into::<web_sys::HtmlSelectElement>()
            .unwrap()
            .value(),
        "foo:bar"
    );
    let report = h.report().as_string().expect("a report");
    assert!(report.contains("profile.unknown-term"), "{report}");
}

/// Parties at the policy, rule and duty level, in the three spellings.
const PARTIES: &str = r#"{"@context":["http://www.w3.org/ns/odrl.jsonld",{"o":"http://www.w3.org/ns/odrl/2/"}],"@type":"Set","uid":"urn:policy:9","assigner":"urn:a","odrl:assignee":"urn:b","permission":[{"target":"urn:asset:1","action":"use","http://www.w3.org/ns/odrl/2/assigner":"urn:a","assignee":"urn:b","duty":[{"action":"inform","o:assignee":"urn:b"}]}]}"#;
const NO_PARTIES: &str = r#"{"policyAssigner":{"max":0},"policyAssignee":{"max":0},"ruleAssigner":{"max":0},"ruleAssignee":{"max":0}}"#;

fn removed_paths(h: &ProseEditorHandle) -> Vec<String> {
    let v: serde_json::Value = serde_json::from_str(&h.removed()).unwrap();
    v.as_array()
        .unwrap()
        .iter()
        .map(|o| o["path"].as_str().unwrap().to_owned())
        .collect()
}

fn error_paths(e: &JsValue) -> Vec<String> {
    js_sys::Array::from(&js_sys::Reflect::get(e, &"paths".into()).unwrap())
        .iter()
        .map(|p| p.as_string().unwrap())
        .collect()
}

#[wasm_bindgen_test]
async fn out_of_scope_show_is_the_default_and_keeps_the_sections() {
    let (c, mut h, calls) = mount(PARTIES).await;
    h.set_limits(js(NO_PARTIES)).expect("limits");
    settle().await;
    assert_eq!(h.removed(), "[]");
    assert_eq!(h.json(), PARTIES);
    click(&c, ADD_PERMISSION).await;
    let json = calls.borrow()[0].0.clone();
    assert!(
        json.contains("\"urn:a\"") && json.contains("\"urn:b\""),
        "{json}"
    );
    assert!(h.set_out_of_scope("drop").is_err());
}

#[wasm_bindgen_test]
async fn out_of_scope_strip_drops_the_sections_and_reports_them() {
    let (c, mut h, calls) = mount(PARTIES).await;
    h.set_out_of_scope("strip").expect("mode");
    h.set_limits(js(NO_PARTIES)).expect("limits");
    settle().await;
    assert_eq!(
        removed_paths(&h),
        [
            "assigner",
            "odrl:assignee",
            "permission[0].http://www.w3.org/ns/odrl/2/assigner",
            "permission[0].assignee",
            "permission[0].duty[0].o:assignee",
        ]
    );
    assert!(calls.borrow().is_empty(), "a strip is not an edit");
    for gone in ["\"urn:a\"", "\"urn:b\""] {
        assert!(!h.json().contains(gone), "{gone} in {}", h.json());
    }
    assert!(
        !c.text_content().unwrap().contains("urn:a "),
        "{}",
        c.inner_html()
    );
    // jsonChange never carries them.
    click(&c, ADD_PERMISSION).await;
    let json = calls.borrow()[0].0.clone();
    assert!(
        !json.contains("\"urn:a\"") && !json.contains("\"urn:b\""),
        "{json}"
    );
    assert_eq!(count(&json, "permission"), 2, "{json}");
}

#[wasm_bindgen_test]
async fn out_of_scope_strip_applies_to_later_updates_and_the_echo_is_a_no_op() {
    let (c, mut h, _calls) = mount(ONE).await;
    h.set_out_of_scope("strip").unwrap();
    h.set_limits(js(NO_PARTIES)).unwrap();
    h.update(PARTIES, None).expect("a document with parties");
    settle().await;
    assert_eq!(removed_paths(&h).len(), 5, "{}", h.removed());
    assert!(!h.json().contains("\"urn:a\""));
    let before = c.inner_html();
    h.update(PARTIES, None).expect("the same text again");
    settle().await;
    assert_eq!(c.inner_html(), before);
    assert_eq!(removed_paths(&h).len(), 5, "a no-op keeps the report");
    h.update(ONE, None).unwrap();
    assert_eq!(h.removed(), "[]", "the report is of the last load");
}

#[wasm_bindgen_test]
async fn out_of_scope_reject_refuses_a_document_and_names_the_paths() {
    let (c, mut h, _calls) = mount(ONE).await;
    h.set_out_of_scope("reject").expect("mode");
    h.set_limits(js(NO_PARTIES)).expect("ONE has no parties");
    settle().await;
    let before = c.inner_html();
    let err = h.update(PARTIES, None).expect_err("refused");
    assert_eq!(error_paths(&err).len(), 5, "{err:?}");
    assert!(error_paths(&err).contains(&"permission[0].assignee".to_string()));
    let message = js_sys::Reflect::get(&err, &"message".into())
        .unwrap()
        .as_string()
        .unwrap();
    assert!(message.contains("permission[0].assignee"), "{message}");
    assert_eq!(
        js_sys::Reflect::get(&err, &"name".into())
            .unwrap()
            .as_string()
            .unwrap(),
        "OutOfScopeError"
    );
    settle().await;
    assert_eq!(c.inner_html(), before, "the document stays");
    assert_eq!(h.json(), ONE);
}

#[wasm_bindgen_test]
async fn out_of_scope_reject_refuses_limits_the_document_on_screen_breaks() {
    let (c, mut h, _calls) = mount(PARTIES).await;
    h.set_out_of_scope("reject")
        .expect("default limits: all in scope");
    settle().await;
    let before = c.inner_html();
    assert!(h.set_limits(js(NO_PARTIES)).is_err());
    settle().await;
    assert_eq!(c.inner_html(), before);
    // The limits did not stick: the add slot of the policy assigner is back.
    assert!(select(&c, "[data-add=\"policy[0]@assigner\"]").is_some());
}

#[wasm_bindgen_test]
async fn out_of_scope_reject_on_a_document_out_of_scope_throws_and_changes_nothing() {
    let (c, mut h, _calls) = mount(PARTIES).await;
    // Under 'show' the limits only hide the add buttons: the parties stay.
    h.set_limits(js(NO_PARTIES)).expect("show keeps them");
    settle().await;
    let before = c.inner_html();
    let err = h
        .set_out_of_scope("reject")
        .expect_err("the document on screen is out of scope");
    assert_eq!(error_paths(&err).len(), 5, "{err:?}");
    assert_eq!(
        js_sys::Reflect::get(&err, &"name".into())
            .unwrap()
            .as_string()
            .unwrap(),
        "OutOfScopeError"
    );
    settle().await;
    assert_eq!(c.inner_html(), before, "the document stays");
    assert_eq!(h.json(), PARTIES);
    assert_eq!(h.removed(), "[]");
    // The mode is still 'show': an out-of-scope document is loaded, not refused.
    h.update(PARTIES, None).expect("still show");
    h.update(ONE, None).unwrap();
    h.update(PARTIES, None)
        .expect("still show, after another document");
    // Dropping the limits lets 'reject' take hold.
    h.set_limits(JsValue::NULL).unwrap();
    h.set_out_of_scope("reject")
        .expect("nothing is out of scope now");
}

#[wasm_bindgen_test]
async fn out_of_scope_other_limits_at_zero_are_covered() {
    let (c, mut h, _calls) = mount(ONE).await;
    h.set_out_of_scope("strip").unwrap();
    h.set_limits(js(r#"{"policyUid":{"max":0},"ruleTarget":{"max":0}}"#))
        .unwrap();
    settle().await;
    assert_eq!(removed_paths(&h), ["uid", "permission[0].target"]);
    assert!(!h.json().contains("urn:policy:1"));
    assert!(select(&c, ".prose-edit").is_some());
}

/// Types `text` into the slot `css` and leaves it, which commits it.
async fn type_into(c: &Element, css: &str, text: &str) {
    let el = select(c, css)
        .unwrap_or_else(|| panic!("{css} in {}", c.inner_html()))
        .dyn_into::<HtmlElement>()
        .unwrap();
    el.focus().unwrap();
    el.set_text_content(Some(text));
    let input = web_sys::InputEventInit::new();
    input.set_bubbles(true);
    el.dispatch_event(&web_sys::InputEvent::new_with_event_init_dict("input", &input).unwrap())
        .unwrap();
    let out = web_sys::FocusEventInit::new();
    out.set_bubbles(true);
    el.dispatch_event(
        &web_sys::FocusEvent::new_with_focus_event_init_dict("focusout", &out).unwrap(),
    )
    .unwrap();
    settle().await;
}

const GROUPED: &str = r#"{"@context":"http://www.w3.org/ns/odrl.jsonld","@type":"Set","uid":"urn:policy:5","permission":[{"target":"urn:asset:1","action":"use","constraint":[{"and":[{"leftOperand":"count","operator":"lteq","rightOperand":3}]}]}]}"#;
const TOP_GROUP: &str = "[data-add=\"policy[0].permission[0]@constraint\"][data-item=\"group\"]";
const INNER_GROUP: &str =
    "[data-add=\"policy[0].permission[0].constraint[0]@child\"][data-item=\"group\"]";

#[wasm_bindgen_test]
async fn a_logical_depth_limit_hides_the_group_buttons_that_would_nest_too_deep() {
    let (c, mut h, _calls) = mount(GROUPED).await;
    assert!(select(&c, TOP_GROUP).is_some());
    assert!(select(&c, INNER_GROUP).is_some());
    h.set_limits(js(r#"{"logicalDepth":{"max":1}}"#))
        .expect("limits");
    settle().await;
    assert!(select(&c, TOP_GROUP).is_some(), "one deep is allowed");
    assert!(select(&c, INNER_GROUP).is_none(), "two deep is not");
    // Ungrouping what the document has stays.
    assert!(
        c.inner_html().contains("Ungroup condition 1"),
        "{}",
        c.inner_html()
    );
    h.set_limits(js(r#"{"logicalDepth":{"max":0}}"#))
        .expect("limits");
    settle().await;
    assert!(select(&c, TOP_GROUP).is_none());
    // The document is kept as it was: the limit is not a scope.
    assert!(h.json().contains("\"and\""));
    assert!(h.set_limits(js(r#"{"logicalDepth":{"max":-1}}"#)).is_err());
    assert!(h.set_limits(js(r#"{"logicalDepth":{"max":1.5}}"#)).is_err());
    h.set_limits(JsValue::NULL).expect("reset");
    settle().await;
    assert!(select(&c, INNER_GROUP).is_some());
}

#[wasm_bindgen_test]
async fn a_logical_depth_limit_holds_for_the_groups_the_editor_adds() {
    let (c, mut h, calls) = mount(GROUPED).await;
    h.set_limits(js(r#"{"logicalDepth":{"max":1}}"#))
        .expect("limits");
    settle().await;
    // One deep is allowed: a second group beside the first.
    click(&c, TOP_GROUP).await;
    assert_eq!(calls.borrow().len(), 1);
    let json = h.json();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let constraints = v["permission"][0]["constraint"].as_array().unwrap();
    assert_eq!(constraints.len(), 2, "{json}");
    // And what the editor added is no deeper than the limit: no group
    // button on either group, new or old.
    assert!(select(&c, INNER_GROUP).is_none());
    assert!(
        select(
            &c,
            "[data-add=\"policy[0].permission[0].constraint[1]@child\"][data-item=\"group\"]"
        )
        .is_none(),
        "{}",
        c.inner_html()
    );
}

const CLOSED_POLICY: &str = r#"{"@context":["http://www.w3.org/ns/odrl.jsonld",{"ex":"https://example.org/odrl/profile/research/"}],"@type":"Set","uid":"urn:policy:s","profile":"https://example.org/odrl/profile/research","permission":[{"target":"urn:asset:1","assigner":"urn:party:a","action":"use","constraint":[{"leftOperand":"ex:researchPurpose","operator":"isAnyOf","rightOperand":[{"@id":"ex:academic"}]}]}]}"#;

#[wasm_bindgen_test]
async fn refuse_terms_is_a_switch_that_leaves_the_document_and_the_rest_alone() {
    // The slots already close under strict validation; the switch makes the
    // reducer refuse a host's event (a fix, say) that writes an inadmissible
    // term. It changes nothing on screen by itself and survives the other
    // setters.
    let (c, mut h, calls) = mount(CLOSED_POLICY).await;
    h.set_refuse_terms(true);
    settle().await; // no validation: nothing to gate, nothing breaks
    h.set_profile(Some(CLOSED.to_string())).expect("profile");
    h.set_validation(js("{}")).expect("validation");
    h.set_term_input("select").expect("select");
    h.set_limits(js(r#"{"logicalDepth":{"max":2}}"#))
        .expect("limits");
    settle().await;
    let before = c.inner_html();
    h.set_refuse_terms(true);
    settle().await;
    assert_eq!(c.inner_html(), before, "setting it again changes nothing");
    h.set_refuse_terms(false);
    h.set_refuse_terms(true);
    settle().await;
    assert_eq!(c.inner_html(), before);
    assert!(calls.borrow().is_empty(), "a setting is not an edit");
    // The editor still edits.
    click(&c, "[data-add=\"policy[0]@permission\"]").await;
    assert_eq!(calls.borrow().len(), 1);
}

// --- companion SHACL shapes (setShapes) ------------------------------------------

const DX_PROFILE: &str = include_str!("../../prose-core/tests/shapes/dataspace-profile.jsonld");
const DX_SHAPES: &str = include_str!("../../prose-core/tests/shapes/dataspace-shapes.jsonld");
const DX_POLICY: &str = r#"{"@context":["http://www.w3.org/ns/odrl.jsonld",{"dx":"https://example.org/dataspace/"}],"@type":"Set","uid":"urn:p","profile":"https://example.org/dataspace/profile","permission":[{"target":"urn:a","action":"dx:train","constraint":[{"leftOperand":"dx:purpose","operator":"eq","rightOperand":{"@id":"dx:clinical"}}]}]}"#;
const DX_RIGHT: &str = "[data-slot=\"policy[0].permission[0].constraint[0]#rightOperand[0]\"]";

#[wasm_bindgen_test]
async fn set_shapes_closes_a_right_operand_to_a_select_and_other_values_are_errors() {
    let (c, mut h, calls) = mount(DX_POLICY).await;
    h.set_profile(Some(DX_PROFILE.to_string()))
        .expect("profile");
    h.set_term_input("select").expect("select");
    h.set_validation(js("{}")).expect("validation");
    settle().await;
    // Without shapes the value is free text, and nothing is wrong with it.
    assert_ne!(select(&c, DX_RIGHT).unwrap().tag_name(), "SELECT");
    assert!(
        !h.report()
            .as_string()
            .unwrap()
            .contains("value-not-allowed")
    );

    let summary = h.set_shapes(Some(DX_SHAPES.to_string())).expect("shapes");
    settle().await;
    let s: serde_json::Value = serde_json::from_str(&summary).unwrap();
    assert_eq!(s["constraints"], 5, "{summary}");
    assert_eq!(s["warnings"], serde_json::json!([]));
    let right = select(&c, DX_RIGHT).expect("the right operand");
    assert_eq!(right.tag_name(), "SELECT", "{}", c.inner_html());
    let options = right.query_selector_all("option").unwrap();
    let values: Vec<String> = (0..options.length())
        .filter_map(|i| options.item(i))
        .filter_map(|o| o.dyn_into::<web_sys::HtmlOptionElement>().ok())
        .map(|o| o.value())
        .collect();
    assert!(values.contains(&"dx:research".to_string()), "{values:?}");
    assert!(values.contains(&"dx:marketing".to_string()), "{values:?}");
    // The stored value is outside the list: kept, selected and an error.
    assert_eq!(right.get_attribute("data-outside").as_deref(), Some("true"));
    let report = h.report().as_string().expect("a report");
    assert!(report.contains("prose.value-not-allowed"), "{report}");
    assert!(report.contains("\"source\":\"shapes\""), "{report}");
    assert!(calls.borrow().is_empty(), "loading shapes is not an edit");

    // A new profile keeps the shapes.
    h.set_profile(Some(DX_PROFILE.to_string()))
        .expect("profile");
    settle().await;
    assert_eq!(select(&c, DX_RIGHT).unwrap().tag_name(), "SELECT");

    // Clearing them gives the free text back.
    assert_eq!(h.set_shapes(None).expect("clear"), "null");
    settle().await;
    assert_ne!(select(&c, DX_RIGHT).unwrap().tag_name(), "SELECT");
    assert!(
        !h.report()
            .as_string()
            .unwrap()
            .contains("value-not-allowed")
    );
}

#[wasm_bindgen_test]
async fn unreadable_shapes_throw_and_keep_the_previous() {
    let (c, mut h, _calls) = mount(DX_POLICY).await;
    h.set_profile(Some(DX_PROFILE.to_string())).unwrap();
    h.set_term_input("select").unwrap();
    h.set_shapes(Some(DX_SHAPES.to_string())).unwrap();
    settle().await;
    for bad in ["{", "<!doctype html><p>hi</p>", DX_PROFILE] {
        assert!(h.set_shapes(Some(bad.to_string())).is_err(), "{bad}");
        settle().await;
        assert_eq!(
            select(&c, DX_RIGHT).unwrap().tag_name(),
            "SELECT",
            "the previous shapes stay after {bad}"
        );
    }
}

#[wasm_bindgen_test]
async fn shapes_close_the_action_list_for_the_policy() {
    let (c, mut h, _calls) = mount(DX_POLICY).await;
    h.set_profile(Some(DX_PROFILE.to_string())).unwrap();
    h.set_term_input("select").unwrap();
    h.set_validation(js("{}")).unwrap();
    h.set_shapes(Some(DX_SHAPES.to_string())).unwrap();
    settle().await;
    let action = select(&c, ACTION).expect("the action select");
    let html = action.inner_html();
    assert!(
        html.contains("dx:train") && html.contains("dx:share"),
        "{html}"
    );
    assert!(!html.contains("dx:publish"), "{html}");
}

const LITERAL_SHAPES: &str = r#"{"@context":{"sh":"http://www.w3.org/ns/shacl#","odrl":"http://www.w3.org/ns/odrl/2/","dx":"https://example.org/dataspace/"},"@graph":[{"@id":"dx:S","@type":"sh:NodeShape","sh:property":[{"sh:path":{"@id":"odrl:leftOperand"},"sh:hasValue":{"@id":"dx:purpose"}},{"sh:path":{"@id":"odrl:rightOperand"},"sh:in":{"@list":["urn:code:1","DEF-2"]}}]}]}"#;

#[wasm_bindgen_test]
async fn shapes_with_literal_values_make_a_select_and_a_pick_is_written_as_text() {
    let policy = DX_POLICY.replace(r#"{"@id":"dx:clinical"}"#, r#""tbd""#);
    let (c, mut h, calls) = mount(&policy).await;
    h.set_profile(Some(DX_PROFILE.to_string())).unwrap();
    h.set_term_input("select").unwrap();
    h.set_validation(js("{}")).unwrap();
    let summary = h.set_shapes(Some(LITERAL_SHAPES.to_string())).unwrap();
    settle().await;
    let s: serde_json::Value = serde_json::from_str(&summary).unwrap();
    assert_eq!(s["warnings"], serde_json::json!([]), "{summary}");
    let right = select(&c, DX_RIGHT).expect("the right operand");
    assert_eq!(right.tag_name(), "SELECT", "{}", c.inner_html());
    let html = right.inner_html();
    assert!(
        html.contains("value=\"urn:code:1\"") && html.contains("value=\"DEF-2\""),
        "{html}"
    );
    // "tbd" is outside the list: kept, selected, and an error.
    assert_eq!(right.get_attribute("data-outside").as_deref(), Some("true"));
    let report = h.report().as_string().expect("a report");
    assert!(report.contains("prose.value-not-allowed"), "{report}");
    assert!(calls.borrow().is_empty());
    // A pick that looks like an IRI is still the literal the shape lists.
    let el: web_sys::HtmlSelectElement = right.dyn_into().unwrap();
    el.set_value("urn:code:1");
    let ev = web_sys::Event::new_with_event_init_dict("change", &{
        let i = web_sys::EventInit::new();
        i.set_bubbles(true);
        i
    })
    .unwrap();
    el.dispatch_event(&ev).unwrap();
    settle().await;
    assert_eq!(calls.borrow().len(), 1);
    let json = h.json();
    assert!(
        json.contains(r#""rightOperand":"urn:code:1""#)
            || json.contains(r#""rightOperand": "urn:code:1""#),
        "a string, not {{\"@id\"}}: {json}"
    );
    assert!(!json.contains("@id\":\"urn:code:1"), "{json}");
}

// --- operator overlays (setOverlays) ---------------------------------------------

const DX_OVERLAY: &str = include_str!("../../prose-core/tests/overlay/dataspace-overlay.jsonld");

#[wasm_bindgen_test]
async fn set_overlays_closes_a_right_operand_to_a_select_and_other_values_are_errors() {
    let (c, mut h, calls) = mount(DX_POLICY).await;
    h.set_profile(Some(DX_PROFILE.to_string()))
        .expect("profile");
    h.set_term_input("select").expect("select");
    h.set_validation(js("{}")).expect("validation");
    settle().await;
    assert_ne!(select(&c, DX_RIGHT).unwrap().tag_name(), "SELECT");

    let summary = h
        .set_overlays(vec![DX_OVERLAY.to_string()])
        .expect("overlay");
    settle().await;
    let s: serde_json::Value = serde_json::from_str(&summary).unwrap();
    assert_eq!(s[0]["terms"], 3, "{summary}");
    assert_eq!(s[0]["warnings"], serde_json::json!([]));
    let right = select(&c, DX_RIGHT).expect("the right operand");
    assert_eq!(right.tag_name(), "SELECT", "{}", c.inner_html());
    let html = right.inner_html();
    assert!(
        html.contains("dx:research") && html.contains("dx:marketing"),
        "{html}"
    );
    assert_eq!(right.get_attribute("data-outside").as_deref(), Some("true"));
    let report = h.report().as_string().expect("a report");
    assert!(report.contains("prose.value-not-allowed"), "{report}");
    assert!(report.contains("\"source\":\"overlay\""), "{report}");
    assert!(
        calls.borrow().is_empty(),
        "loading an overlay is not an edit"
    );

    // A new profile keeps the overlay.
    h.set_profile(Some(DX_PROFILE.to_string()))
        .expect("profile");
    settle().await;
    assert_eq!(select(&c, DX_RIGHT).unwrap().tag_name(), "SELECT");

    // Shapes win over it and the overlay survives their removal.
    h.set_shapes(Some(DX_SHAPES.to_string())).expect("shapes");
    settle().await;
    assert!(
        h.report()
            .as_string()
            .unwrap()
            .contains("\"source\":\"shapes\"")
    );
    h.set_shapes(None).expect("clear shapes");
    settle().await;
    assert_eq!(select(&c, DX_RIGHT).unwrap().tag_name(), "SELECT");

    // Clearing gives the free text back.
    assert_eq!(h.set_overlays(Vec::new()).expect("clear"), "[]");
    settle().await;
    assert_ne!(select(&c, DX_RIGHT).unwrap().tag_name(), "SELECT");
}

#[wasm_bindgen_test]
async fn unreadable_overlays_throw_and_keep_the_previous() {
    let (c, mut h, _calls) = mount(DX_POLICY).await;
    h.set_profile(Some(DX_PROFILE.to_string())).unwrap();
    h.set_term_input("select").unwrap();
    h.set_overlays(vec![DX_OVERLAY.to_string()]).unwrap();
    settle().await;
    for bad in ["{", "<!doctype html><p>hi</p>", DX_PROFILE] {
        assert!(h.set_overlays(vec![bad.to_string()]).is_err(), "{bad}");
        settle().await;
        assert_eq!(
            select(&c, DX_RIGHT).unwrap().tag_name(),
            "SELECT",
            "the previous overlays stay after {bad}"
        );
    }
}

#[wasm_bindgen_test]
async fn overlay_summaries_are_recomputed_when_the_profile_changes() {
    let ghost = r#"{"@context":{"prose":"https://ds-labs-org.github.io/ds-prose-rs/profile#"},"@id":"https://example.org/odrl/profile/research/fineTune","prose:closed":true}"#;
    let (_c, mut h, _calls) = mount(ONE).await;
    assert_eq!(h.overlay_summaries(), "[]");
    let at_load = h.set_overlays(vec![ghost.to_string()]).unwrap();
    assert!(at_load.contains("no loaded profile declares"), "{at_load}");
    assert_eq!(h.overlay_summaries(), at_load);
    // The research profile declares fineTune as an action, not a left
    // operand: the warning changes with the profile.
    h.set_profile(Some(RESEARCH.to_string())).unwrap();
    let after = h.overlay_summaries();
    assert_ne!(after, at_load);
    assert!(after.contains("not a left operand"), "{after}");
    assert!(!after.contains("no loaded profile declares"), "{after}");
}

const UNBOUND: &str = r#"{"@context":"http://www.w3.org/ns/odrl.jsonld","@type":"Set","uid":"urn:policy:7","profile":"https://example.org/odrl/profile/research","permission":[{"target":"urn:asset:1","action":"ex:train"}]}"#;
const FULL_TRAIN: &str = "https://example.org/odrl/profile/research/train";

// The slots close under strict validation whatever the switch says, so a
// typed term never reaches the reducer: the editor refuses it with
// `setRefuseTerms` on or off. What the switch alone decides, a host event
// the view did not vet, is pinned in prose-yew (`refuseterms_host_event_*`
// against `flag_commits_and_marks`) and prose-core (`edit_strict`); the
// handle offers no such path. Here: both settings refuse the typed term and
// still take one the profile allows.
#[wasm_bindgen_test]
async fn a_term_the_closed_profile_does_not_allow_is_not_sent_with_the_switch_either_way() {
    for refuse in [true, false] {
        let (c, mut h, calls) = mount(FINE).await;
        h.set_profile(Some(CLOSED.to_string())).expect("profile");
        h.set_validation(js("{}")).expect("validation");
        h.set_refuse_terms(refuse);
        settle().await;
        let action = "[data-slot=\"policy[0].permission[0].action[0]#name\"]";
        type_into(&c, action, "ex:nonsense").await;
        assert!(calls.borrow().is_empty(), "refuse={refuse}");
        assert!(!h.json().contains("ex:nonsense"), "refuse={refuse}");
        type_into(&c, action, "ex:train").await;
        assert_eq!(calls.borrow().len(), 1, "refuse={refuse}");
        assert!(calls.borrow()[0].0.contains("ex:train"));
    }
}

// The gate must not turn back what the profile allows: the fix for an unbound
// prefix writes the full IRI of a declared term, and goes through with the
// switch on.
#[wasm_bindgen_test]
async fn with_refuse_terms_a_fix_that_writes_an_allowed_term_still_applies() {
    let (c, mut h, calls) = mount(UNBOUND).await;
    h.set_profile(Some(CLOSED.to_string())).expect("profile");
    h.set_validation(js("{}")).expect("validation");
    h.set_refuse_terms(true);
    settle().await;
    let report = h.report().as_string().expect("a report");
    assert!(report.contains("profile.unbound-prefix"), "{report}");
    click(&c, ".prose-problem-fix").await;
    assert_eq!(calls.borrow().len(), 1, "{}", c.inner_html());
    assert!(h.json().contains(FULL_TRAIN), "{}", h.json());
}

// --- runtime value sources (setValueSource) --------------------------------------

const PURPOSE: &str = "https://example.org/dataspace/purpose";
const PURPOSES: &str = r#"[
  {"value":"https://example.org/dataspace/research","label":"Research","group":"Purposes"},
  {"value":"https://example.org/dataspace/marketing","label":"Marketing","group":"Purposes"},
  {"value":"https://example.org/dataspace/clinical","label":"Clinical research","group":"Registry"}
]"#;
const CLOSED_SOURCE: &str = r#"{"closed":true}"#;
// The same list, without clinical: a value the registry dropped.
const FEWER: &str = r#"[
  {"value":"https://example.org/dataspace/research","label":"Research","group":"Purposes"}
]"#;

async fn mounted_with_select() -> (Element, ProseEditorHandle, Calls) {
    let (c, mut h, calls) = mount(DX_POLICY).await;
    h.set_profile(Some(DX_PROFILE.to_string())).unwrap();
    h.set_term_input("select").unwrap();
    h.set_validation(js("{}")).unwrap();
    settle().await;
    (c, h, calls)
}

#[wasm_bindgen_test]
async fn a_closed_value_source_fills_the_select_with_labelled_groups() {
    let (c, mut h, calls) = mounted_with_select().await;
    assert_ne!(select(&c, DX_RIGHT).unwrap().tag_name(), "SELECT");
    let summary = h
        .set_value_source(PURPOSE, js(PURPOSES), Some(js(CLOSED_SOURCE)))
        .expect("source");
    settle().await;
    let s: serde_json::Value = serde_json::from_str(&summary).unwrap();
    assert_eq!(s["values"], 3, "{summary}");
    assert_eq!(s["closed"], true);
    let right = select(&c, DX_RIGHT).expect("the right operand");
    assert_eq!(right.tag_name(), "SELECT", "{}", c.inner_html());
    let html = right.inner_html();
    assert!(html.contains("<optgroup label=\"Purposes\">"), "{html}");
    assert!(html.contains("<optgroup label=\"Registry\">"), "{html}");
    assert!(html.contains("Marketing"), "{html}");
    // clinical is listed: nothing outside, no error.
    assert_eq!(right.get_attribute("data-outside"), None, "{html}");
    assert!(calls.borrow().is_empty(), "loading a source is not an edit");
}

#[wasm_bindgen_test]
async fn a_value_removed_from_the_source_is_kept_flagged_and_not_dropped() {
    let (c, mut h, calls) = mounted_with_select().await;
    h.set_value_source(PURPOSE, js(PURPOSES), Some(js(CLOSED_SOURCE)))
        .unwrap();
    settle().await;
    let report = h.report().as_string().expect("a report");
    assert!(!report.contains("prose.value-not-allowed"), "{report}");
    // The registry drops clinical.
    h.set_value_source(PURPOSE, js(FEWER), Some(js(CLOSED_SOURCE)))
        .unwrap();
    settle().await;
    let right = select(&c, DX_RIGHT).expect("the right operand");
    assert_eq!(right.get_attribute("data-outside").as_deref(), Some("true"));
    assert!(
        right.inner_html().contains("dx:clinical"),
        "the value is kept as an option: {}",
        right.inner_html()
    );
    assert!(
        c.text_content().unwrap().contains("not in the host's list"),
        "{}",
        c.inner_html()
    );
    let report = h.report().as_string().expect("a report");
    assert!(report.contains("prose.value-not-allowed"), "{report}");
    assert!(report.contains("\"source\":\"host\""), "{report}");
    // Not dropped: the document says what it said, and nothing was emitted.
    assert!(h.json().contains("dx:clinical"), "{}", h.json());
    assert!(calls.borrow().is_empty());
    // Listed again: the flag goes.
    h.set_value_source(PURPOSE, js(PURPOSES), Some(js(CLOSED_SOURCE)))
        .unwrap();
    settle().await;
    assert!(
        !h.report()
            .as_string()
            .unwrap()
            .contains("prose.value-not-allowed")
    );
}

#[wasm_bindgen_test]
async fn a_suggest_only_source_leaves_the_value_free_text_and_null_clears() {
    let (c, mut h, _calls) = mounted_with_select().await;
    h.set_value_source(PURPOSE, js(PURPOSES), None).unwrap();
    settle().await;
    assert_ne!(select(&c, DX_RIGHT).unwrap().tag_name(), "SELECT");
    h.set_value_source(PURPOSE, js(PURPOSES), Some(js(CLOSED_SOURCE)))
        .unwrap();
    settle().await;
    assert_eq!(select(&c, DX_RIGHT).unwrap().tag_name(), "SELECT");
    assert_eq!(
        h.set_value_source(PURPOSE, JsValue::NULL, None).unwrap(),
        "null"
    );
    settle().await;
    assert_ne!(select(&c, DX_RIGHT).unwrap().tag_name(), "SELECT");
}

#[wasm_bindgen_test]
async fn the_sources_survive_a_new_profile_and_unreadable_ones_throw() {
    let (c, mut h, _calls) = mounted_with_select().await;
    h.set_value_sources(js(&format!(
        r#"{{"{PURPOSE}": {{"values": {PURPOSES}, "closed": true}}}}"#
    )))
    .unwrap();
    settle().await;
    h.set_profile(Some(DX_PROFILE.to_string())).unwrap();
    settle().await;
    assert_eq!(select(&c, DX_RIGHT).unwrap().tag_name(), "SELECT");
    assert!(h.set_value_source(PURPOSE, js("[1]"), None).is_err());
    assert!(h.set_value_sources(js(r#"{"k": 3}"#)).is_err());
    settle().await;
    assert_eq!(
        select(&c, DX_RIGHT).unwrap().tag_name(),
        "SELECT",
        "the previous sources stay"
    );
    h.set_value_sources(JsValue::NULL).unwrap();
    settle().await;
    assert_ne!(select(&c, DX_RIGHT).unwrap().tag_name(), "SELECT");
}

#[wasm_bindgen_test]
async fn picking_a_listed_value_writes_it_to_the_document() {
    let (c, mut h, calls) = mounted_with_select().await;
    h.set_value_source(PURPOSE, js(PURPOSES), Some(js(CLOSED_SOURCE)))
        .unwrap();
    settle().await;
    let right = select(&c, DX_RIGHT).unwrap();
    let el: web_sys::HtmlSelectElement = right.dyn_into().unwrap();
    el.set_value("https://example.org/dataspace/marketing");
    let ev = web_sys::Event::new_with_event_init_dict("change", &{
        let i = web_sys::EventInit::new();
        i.set_bubbles(true);
        i
    })
    .unwrap();
    el.dispatch_event(&ev).unwrap();
    settle().await;
    let recorded = calls.borrow().clone();
    assert_eq!(recorded.len(), 1, "{recorded:?}");
    assert!(
        recorded[0]
            .0
            .contains("https://example.org/dataspace/marketing"),
        "{}",
        recorded[0].0
    );
}

#[wasm_bindgen_test]
async fn out_of_scope_strip_follows_a_context_alias() {
    let doc = r#"{"@context":["http://www.w3.org/ns/odrl.jsonld",{"giver":"odrl:assigner"}],"@type":"Set","uid":"urn:policy:9","giver":"urn:a","permission":[{"target":"urn:asset:1","action":"use","giver":"urn:a"}]}"#;
    let (_c, mut h, _calls) = mount(doc).await;
    h.set_out_of_scope("strip").expect("mode");
    h.set_limits(js(NO_PARTIES)).expect("limits");
    settle().await;
    assert_eq!(removed_paths(&h), ["giver", "permission[0].giver"]);
    assert!(!h.json().contains("\"urn:a\""), "{}", h.json());
}

#[wasm_bindgen_test]
async fn the_editor_handle_reports_what_the_profiles_make_unusable_in_the_shapes_summary() {
    let (_c, mut h, _calls) = mount(DX_POLICY).await;
    h.set_profile(Some(chain_profile(70))).unwrap();
    let summary = h.set_shapes(Some(CLASS_SHAPES.to_string())).unwrap();
    let v: serde_json::Value = serde_json::from_str(&summary).unwrap();
    let warned = v["warnings"].as_array().unwrap().iter().any(|m| {
        let m = m.as_str().unwrap();
        m.contains("sh:class") && m.contains("more members")
    });
    assert!(warned, "{summary}");
    let again = |h: &ProseEditorHandle| h.shapes_summary().contains("more members");
    h.set_profile(Some(chain_profile(5))).unwrap();
    assert!(!again(&h), "new profiles change what the shapes can say");
    h.set_profile(Some(chain_profile(70))).unwrap();
    assert!(again(&h));
    h.set_shapes(None).unwrap();
    assert_eq!(h.shapes_summary(), "null");
}
