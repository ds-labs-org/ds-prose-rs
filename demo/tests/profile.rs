//! The demo's Profile field, mounted in headless Chrome. Run with
//! `cargo test -p demo --target wasm32-unknown-unknown --test profile`.
#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;
use web_sys::{
    DataTransfer, Element, Event, EventInit, FocusEvent, FocusEventInit, HtmlElement,
    HtmlInputElement, HtmlTextAreaElement, InputEvent, InputEventInit,
};

wasm_bindgen_test_configure!(run_in_browser);

const POLICY: &str = include_str!("../../prose-core/tests/profiles/research-policy.json");
const RESEARCH: &str = include_str!("../../prose-core/tests/profiles/research.jsonld");

fn document() -> web_sys::Document {
    web_sys::window().unwrap().document().unwrap()
}

async fn settle() {
    for _ in 0..4 {
        TimeoutFuture::new(0).await;
    }
}

/// Waits for something asynchronous (a file read) to land.
async fn until(c: &Element, what: &str, cond: impl Fn(&Element) -> bool) {
    for _ in 0..100 {
        if cond(c) {
            return;
        }
        TimeoutFuture::new(20).await;
    }
    panic!("timed out waiting for {what}:\n{}", c.inner_html());
}

/// The demo, with the research policy in the Read tab's field.
async fn mount() -> Element {
    let c = document().create_element("div").unwrap();
    document().body().unwrap().append_child(&c).unwrap();
    std::mem::forget(yew::Renderer::<demo::DemoApp>::with_root(c.clone()).render());
    settle().await;
    set_textarea(&c, "textarea#policy", POLICY);
    settle().await;
    c
}

fn q(c: &Element, sel: &str) -> Element {
    c.query_selector(sel)
        .unwrap()
        .unwrap_or_else(|| panic!("no {sel} in:\n{}", c.inner_html()))
}

fn has(c: &Element, sel: &str) -> bool {
    c.query_selector(sel).unwrap().is_some()
}

async fn click(c: &Element, sel: &str) {
    q(c, sel).dyn_into::<HtmlElement>().unwrap().click();
    settle().await;
}

async fn click_tab(c: &Element, label: &str) {
    let tabs = c.query_selector_all("[role=tab]").unwrap();
    for i in 0..tabs.length() {
        let t = tabs.item(i).unwrap().dyn_into::<HtmlElement>().unwrap();
        if t.text_content().unwrap().trim() == label {
            t.click();
            settle().await;
            return;
        }
    }
    panic!("no tab {label}");
}

fn input(el: &Element) {
    let init = InputEventInit::new();
    init.set_bubbles(true);
    el.dispatch_event(&InputEvent::new_with_event_init_dict("input", &init).unwrap())
        .unwrap();
}

fn set_textarea(c: &Element, sel: &str, value: &str) {
    let ta = q(c, sel).dyn_into::<HtmlTextAreaElement>().unwrap();
    ta.set_value(value);
    input(&ta);
}

fn button(c: &Element, label: &str) -> Element {
    let all = c.query_selector_all("button").unwrap();
    (0..all.length())
        .map(|i| all.item(i).unwrap().dyn_into::<Element>().unwrap())
        .find(|b| b.text_content().as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no {label} button"))
}

fn textarea_value(c: &Element, sel: &str) -> String {
    q(c, sel).dyn_into::<HtmlTextAreaElement>().unwrap().value()
}

fn prose(c: &Element) -> String {
    c.query_selector(".ds-prose")
        .unwrap()
        .and_then(|e| e.text_content())
        .unwrap_or_default()
}

fn status(c: &Element) -> String {
    q(c, "#profile-status").text_content().unwrap_or_default()
}

const SUGGESTED: &str = "optgroup[label=\"Suggested by the profile\"]";

#[wasm_bindgen_test]
async fn sample_changes_read_prose_and_shows_glossary() {
    let c = mount().await;
    assert!(prose(&c).contains("may fine tune"), "{}", prose(&c));
    assert!(!has(&c, ".prose-glossary"));
    let st = q(&c, "#profile-status");
    assert_eq!(st.get_attribute("role").as_deref(), Some("status"));
    assert_eq!(st.get_attribute("aria-live").as_deref(), Some("polite"));

    click(&c, "#profile-sample").await;
    assert_eq!(textarea_value(&c, "textarea#profile-json"), RESEARCH);
    assert!(prose(&c).contains("may fine-tune"), "{}", prose(&c));
    assert!(has(&c, ".prose-glossary"));
    assert!(
        status(&c).starts_with("Using Research Data Profile 0.1.0: 3 actions"),
        "{}",
        status(&c)
    );
}

#[wasm_bindgen_test]
async fn turtle_and_odrl22_presets_apply() {
    let c = mount().await;
    click(&c, "#profile-sample-ttl").await;
    assert!(textarea_value(&c, "textarea#profile-json").starts_with("@prefix"));
    assert!(prose(&c).contains("may fine-tune"));
    click(&c, "#profile-odrl22").await;
    assert!(
        status(&c).starts_with("Using ODRL Version 2.2: 72 actions"),
        "{}",
        status(&c)
    );
    // Core terms keep their wording; the glossary explains them.
    let glossary = q(&c, ".prose-glossary").text_content().unwrap();
    assert!(glossary.contains("Commercialize"), "{glossary}");
}

#[wasm_bindgen_test]
async fn sample_changes_edit_suggestions() {
    let c = mount().await;
    click_tab(&c, "Edit").await;
    assert!(!has(&c, SUGGESTED));
    // The field is shared: it is there in the Edit tab too.
    click(&c, "#profile-sample").await;
    assert!(has(&c, SUGGESTED), "{}", c.inner_html());
    let sentence = q(&c, ".ds-prose").text_content().unwrap();
    assert!(sentence.contains("fine-tune"), "{sentence}");
    // Back in the Read tab, the same profile applies.
    click_tab(&c, "Read").await;
    assert!(has(&c, ".prose-glossary"));
}

#[wasm_bindgen_test]
async fn invalid_text_alerts_keeps_text() {
    let c = mount().await;
    click(&c, "#profile-sample").await;
    let bad = "<!doctype html><title>Profile</title>";
    set_textarea(&c, "textarea#profile-json", bad);
    settle().await;
    // Typing does not parse: the profile still applies.
    assert!(has(&c, ".prose-glossary"));
    assert!(!has(&c, "#profile-error"));
    click(&c, "#profile-apply").await;
    let e = q(&c, "#profile-error");
    assert_eq!(e.get_attribute("role").as_deref(), Some("alert"));
    assert!(e.text_content().unwrap().contains("HTML page"));
    assert_eq!(textarea_value(&c, "textarea#profile-json"), bad);
    assert!(!has(&c, ".prose-glossary"), "no hidden last-good profile");
    assert!(prose(&c).contains("may fine tune"));
}

#[wasm_bindgen_test]
async fn clear_restores_defaults_keeps_doc() {
    let c = mount().await;
    click_tab(&c, "Edit").await;
    let slot = q(&c, "[data-slot=\"policy[0]#uid\"]")
        .dyn_into::<HtmlElement>()
        .unwrap();
    slot.focus().unwrap();
    slot.set_text_content(Some("urn:edited"));
    input(&slot);
    let init = FocusEventInit::new();
    init.set_bubbles(true);
    slot.dispatch_event(&FocusEvent::new_with_focus_event_init_dict("focusout", &init).unwrap())
        .unwrap();
    settle().await;
    click(&c, "#profile-sample").await;
    assert!(has(&c, SUGGESTED));
    click(&c, "#profile-clear").await;
    assert_eq!(textarea_value(&c, "textarea#profile-json"), "");
    assert!(!has(&c, SUGGESTED));
    assert_eq!(
        q(&c, "[data-slot=\"policy[0]#uid\"]")
            .text_content()
            .unwrap(),
        "urn:edited",
        "the document is kept"
    );
    let undo = button(&c, "Undo");
    assert!(!undo.has_attribute("disabled"), "the undo history is kept");
}

#[wasm_bindgen_test]
async fn file_input_via_datatransfer() {
    let c = mount().await;
    let parts = js_sys::Array::of1(&JsValue::from_str(RESEARCH));
    let file = web_sys::File::new_with_str_sequence(&parts, "research.jsonld").unwrap();
    let dt = DataTransfer::new().unwrap();
    dt.items().add_with_file(&file).unwrap();
    let el = q(&c, "input#profile-file")
        .dyn_into::<HtmlInputElement>()
        .unwrap();
    assert_eq!(el.type_(), "file");
    el.set_files(Some(&dt.files().unwrap()));
    let init = EventInit::new();
    init.set_bubbles(true);
    el.dispatch_event(&Event::new_with_event_init_dict("change", &init).unwrap())
        .unwrap();
    until(&c, "the file's profile", |c| has(c, ".prose-glossary")).await;
    assert_eq!(textarea_value(&c, "textarea#profile-json"), RESEARCH);
    assert!(status(&c).starts_with("Using Research Data Profile"));
}
