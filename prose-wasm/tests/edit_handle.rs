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
