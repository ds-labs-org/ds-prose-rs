//! The demo's two tabs, mounted in headless Chrome. Run with
//! `cargo test -p demo --target wasm32-unknown-unknown --test tabs`.
#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{
    Element, FocusEvent, FocusEventInit, HtmlElement, HtmlTextAreaElement, InputEvent,
    InputEventInit,
};

wasm_bindgen_test_configure!(run_in_browser);

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
    c
}

fn q(c: &Element, sel: &str) -> Element {
    c.query_selector(sel)
        .unwrap()
        .unwrap_or_else(|| panic!("no {sel} in:\n{}", c.inner_html()))
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

#[wasm_bindgen_test]
async fn edits_survive_a_trip_through_the_read_tab() {
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
    assert!(textarea_value(&c, "textarea#written").contains("urn:edited"));

    click_tab(&c, "Read").await;
    assert!(
        textarea_value(&c, "textarea#policy").contains("urn:edited"),
        "the Read tab shows the edited policy"
    );
    click_tab(&c, "Edit").await;
    assert_eq!(
        q(&c, "[data-slot=\"policy[0]#uid\"]")
            .text_content()
            .unwrap(),
        "urn:edited"
    );
    assert!(textarea_value(&c, "textarea#written").contains("urn:edited"));
}

#[wasm_bindgen_test]
async fn edit_tab_reports_unreadable_json_and_keeps_the_text() {
    for bad in ["{", "{\"hello\":1}", ""] {
        let c = mount().await;
        set_textarea(&c, "textarea#policy", bad);
        settle().await;
        click_tab(&c, "Edit").await;
        assert!(
            c.query_selector(".prose-error").unwrap().is_some(),
            "an error is shown for {bad:?}:\n{}",
            c.inner_html()
        );
        assert_eq!(textarea_value(&c, "textarea#written"), bad);
        click_tab(&c, "Read").await;
        assert_eq!(
            textarea_value(&c, "textarea#policy"),
            bad,
            "the user's text is not replaced"
        );
    }
}
