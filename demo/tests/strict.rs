//! The demo's strict mode, mounted in headless Chrome: the switches, the
//! status badge, the problems panel with its Go to and fix buttons, and the
//! export gate. Run with
//! `cargo test -p demo --target wasm32-unknown-unknown --test strict`.
#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{
    Element, HtmlButtonElement, HtmlElement, HtmlInputElement, HtmlTextAreaElement, InputEvent,
    InputEventInit, KeyboardEvent, KeyboardEventInit,
};

wasm_bindgen_test_configure!(run_in_browser);

const POLICY: &str = include_str!("../../prose-core/tests/profiles/research-policy.json");
const STRUCTURAL_BAD: &str = include_str!("../../prose-core/tests/strict/structural-bad.json");
const VIOLATES: &str = "Research agreement (violates the profile)";
const CLEAN: &str = "Strict clean";
const FOREIGN_ACTION: &str = "policy[0].permission[2].action[0]#name";
const VALUE: &str = "policy[0].permission[0].constraint[0]#rightOperand[0]";

fn document() -> web_sys::Document {
    web_sys::window().unwrap().document().unwrap()
}

async fn settle() {
    for _ in 0..4 {
        TimeoutFuture::new(0).await;
    }
}

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

fn count(c: &Element, sel: &str) -> u32 {
    c.query_selector_all(sel).unwrap().length()
}

fn text(c: &Element, sel: &str) -> String {
    q(c, sel).text_content().unwrap_or_default()
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

fn textarea_value(c: &Element, sel: &str) -> String {
    q(c, sel).dyn_into::<HtmlTextAreaElement>().unwrap().value()
}

fn button(c: &Element, label: &str) -> HtmlButtonElement {
    let all = c.query_selector_all("button").unwrap();
    (0..all.length())
        .map(|i| {
            all.item(i)
                .unwrap()
                .dyn_into::<HtmlButtonElement>()
                .unwrap()
        })
        .find(|b| b.text_content().as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no {label} button"))
}

async fn preset(c: &Element, label: &str) {
    button(c, label).click();
    settle().await;
}

fn checked(c: &Element, sel: &str) -> bool {
    q(c, sel).dyn_into::<HtmlInputElement>().unwrap().checked()
}

fn disabled(c: &Element, sel: &str) -> bool {
    q(c, sel).has_attribute("disabled")
}

/// Strict on, the closed sample profile loaded, and `label`'s policy.
async fn strict(label: &str) -> Element {
    let c = mount().await;
    click(&c, "#profile-sample-closed").await;
    preset(&c, label).await;
    click(&c, "#profile-strict").await;
    c
}

fn problem_with(c: &Element, needle: &str) -> Element {
    let all = c.query_selector_all("#problems .prose-problem").unwrap();
    (0..all.length())
        .map(|i| all.item(i).unwrap().dyn_into::<Element>().unwrap())
        .find(|p| p.text_content().unwrap_or_default().contains(needle))
        .unwrap_or_else(|| panic!("no problem with {needle} in:\n{}", c.inner_html()))
}

#[wasm_bindgen_test]
async fn toggle_strict_turns_warnings_to_errors() {
    let c = mount().await;
    click(&c, "#profile-sample-closed").await;
    assert!(!checked(&c, "#profile-strict"), "strict is opt-in");
    assert!(!has(&c, "#profile-structural") && !has(&c, "#profile-refuse"));
    click_tab(&c, "Edit").await;
    // Advisory: ex:distill is a warning, and there is no verdict.
    assert!(has(&c, ".prose-issue-warning"), "{}", c.inner_html());
    assert!(!has(&c, ".prose-issue-error"));
    assert!(!has(&c, "#problems") && !has(&c, "#conformance"));

    click(&c, "#profile-strict").await;
    assert!(checked(&c, "#profile-strict"));
    assert!(has(&c, "#profile-structural") && has(&c, "#profile-refuse"));
    assert!(has(&c, ".prose-issue-error"), "{}", c.inner_html());
    let badge = q(&c, "p#conformance");
    assert_eq!(badge.get_attribute("role").as_deref(), Some("status"));
    assert!(
        text(&c, "#conformance").starts_with("1 error"),
        "{}",
        text(&c, "#conformance")
    );
    assert!(text(&c, "#problems").contains("ex:distill"));
    // The Read tab says it too, and takes the same switch.
    click_tab(&c, "Read").await;
    assert!(has(&c, ".prose-violations"), "{}", c.inner_html());
    assert!(text(&c, "#conformance").starts_with("1 error"));
}

#[wasm_bindgen_test]
async fn a_conforming_policy_says_so_in_the_badge() {
    let c = strict(CLEAN).await;
    assert_eq!(
        text(&c, "#conformance"),
        "Conforms to Research Data Profile 0.1.0 (strict)"
    );
    click_tab(&c, "Edit").await;
    assert_eq!(
        text(&c, "#conformance"),
        "Conforms to Research Data Profile 0.1.0 (strict)"
    );
}

#[wasm_bindgen_test]
async fn without_the_profile_nothing_is_certified() {
    let c = mount().await;
    preset(&c, CLEAN).await;
    click(&c, "#profile-strict").await;
    assert_eq!(
        text(&c, "#conformance"),
        "Cannot certify: profile https://example.org/odrl/profile/research is not loaded"
    );
    assert!(disabled(&c, "#copy-conformant"));
    assert!(
        text(&c, "#export-note")
            .contains("Load profile https://example.org/odrl/profile/research to certify"),
    );
}

#[wasm_bindgen_test]
async fn go_to_focuses_slot() {
    let c = strict(VIOLATES).await;
    click_tab(&c, "Edit").await;
    let p = problem_with(&c, "foo:bar");
    p.query_selector(".prose-problem-go")
        .unwrap()
        .unwrap()
        .dyn_into::<HtmlElement>()
        .unwrap()
        .click();
    settle().await;
    let active = document().active_element().expect("something has focus");
    assert_eq!(
        active.get_attribute("data-slot").as_deref(),
        Some(FOREIGN_ACTION),
        "{}",
        active.outer_html()
    );
}

#[wasm_bindgen_test]
async fn fix_clears_finding() {
    let c = strict(VIOLATES).await;
    click_tab(&c, "Edit").await;
    let before = count(&c, "#problems .prose-problem-error");
    assert!(textarea_value(&c, "textarea#written").contains("ex:distil\""));
    let p = problem_with(&c, "ex:distil");
    p.query_selector(".prose-problem-fix")
        .unwrap()
        .expect("a fix for the misspelt term")
        .dyn_into::<HtmlElement>()
        .unwrap()
        .click();
    settle().await;
    assert!(
        count(&c, "#problems .prose-problem-error") < before,
        "{}",
        text(&c, "#problems")
    );
    assert!(!textarea_value(&c, "textarea#written").contains("ex:distil\""));
    // One undo step puts it back.
    click(&c, "button#undo").await;
    assert!(textarea_value(&c, "textarea#written").contains("ex:distil\""));
}

#[wasm_bindgen_test]
async fn copy_conformant_only_when_conforms() {
    let c = strict(VIOLATES).await;
    assert!(disabled(&c, "#copy-conformant"));
    let note = text(&c, "#export-note");
    assert!(note.contains("error"), "{note}");
    preset(&c, CLEAN).await;
    assert!(!disabled(&c, "#copy-conformant"), "{}", c.inner_html());
    assert!(!has(&c, "#export-note"));
    // The same in the Edit tab.
    click_tab(&c, "Edit").await;
    assert!(!disabled(&c, "#copy-conformant"));
    preset(&c, VIOLATES).await;
    assert!(disabled(&c, "#copy-conformant"));
    // The report downloads whatever the verdict.
    assert!(!disabled(&c, "#report-json"));
    click(&c, "#report-json").await;
}

#[wasm_bindgen_test]
async fn strict_off_reverts() {
    let c = strict(VIOLATES).await;
    click_tab(&c, "Edit").await;
    assert!(has(&c, "#problems") && has(&c, "[data-closed]"));
    click(&c, "#profile-strict").await;
    assert!(!has(&c, "#problems") && !has(&c, "#conformance") && !has(&c, "#copy-conformant"));
    assert!(!has(&c, "[data-closed]") && !has(&c, ".prose-slot-note"));
    assert!(!has(&c, "#profile-structural") && !has(&c, "#profile-refuse"));
    click_tab(&c, "Read").await;
    assert!(!has(&c, ".prose-violations"));
}

#[wasm_bindgen_test]
async fn closed_combobox_keeps_typo() {
    let c = strict(VIOLATES).await;
    click_tab(&c, "Edit").await;
    let before = textarea_value(&c, "textarea#written");
    let el = q(&c, &format!("[data-slot=\"{VALUE}\"]"))
        .dyn_into::<HtmlElement>()
        .unwrap();
    assert_eq!(el.get_attribute("data-closed").as_deref(), Some("true"));
    el.focus().unwrap();
    el.set_text_content(Some("ex:acadmic"));
    input(&el);
    let init = KeyboardEventInit::new();
    init.set_bubbles(true);
    init.set_cancelable(true);
    init.set_key("Enter");
    el.dispatch_event(&KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).unwrap())
        .unwrap();
    settle().await;
    // Nothing was written, and the slot says the text is not taken.
    assert_eq!(textarea_value(&c, "textarea#written"), before);
    let again = q(&c, &format!("[data-slot=\"{VALUE}\"]"));
    assert_eq!(again.get_attribute("aria-invalid").as_deref(), Some("true"));
    assert_eq!(
        again.text_content().as_deref(),
        Some("ex:acadmic"),
        "the typo is kept"
    );
}

#[wasm_bindgen_test]
async fn the_structural_tier_is_off_until_switched_on() {
    let c = strict(CLEAN).await;
    set_textarea(&c, "textarea#policy", STRUCTURAL_BAD);
    settle().await;
    click_tab(&c, "Edit").await;
    let codes = |c: &Element| text(c, "#problems");
    assert!(!codes(&c).contains("odrl."), "{}", codes(&c));
    click(&c, "#profile-structural").await;
    assert!(checked(&c, "#profile-structural"));
    assert!(codes(&c).contains("odrl."), "{}", codes(&c));
    click(&c, "#profile-structural").await;
    assert!(!codes(&c).contains("odrl."));
}

#[wasm_bindgen_test]
async fn the_refuse_switch_is_there_and_keeps_the_verdict() {
    let c = strict(VIOLATES).await;
    click_tab(&c, "Edit").await;
    assert!(!checked(&c, "#profile-refuse"), "refusing is opt-in");
    let before = text(&c, "#conformance");
    click(&c, "#profile-refuse").await;
    assert!(checked(&c, "#profile-refuse"));
    assert_eq!(text(&c, "#conformance"), before);
    // The slot that is closed still opens only to what the profile lists.
    assert!(has(&c, "[data-closed]"));
}
