//! Mounts the real `<OdrlProseView>` in a real browser and drives its
//! controls with synthetic events. Synthetic events do not insert text, so
//! typing is simulated by setting `textContent` and dispatching `input`;
//! real typing, IME and clipboard permissions are covered by manual checks.
//! Run with `cargo test -p prose-yew --target wasm32-unknown-unknown
//! --features csr --test edit_dom` (see `.cargo/config.toml`).
#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;
use std::rc::Rc;

use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{
    ClipboardEvent, ClipboardEventInit, DataTransfer, Element, Event, EventInit, FocusEvent,
    FocusEventInit, HtmlElement, HtmlSelectElement, InputEvent, InputEventInit, KeyboardEvent,
    KeyboardEventInit,
};
use yew::prelude::*;

use prose_core::edit::{EditEvent, EditRules, ListPath, NodeId, NodePath, SlotPath, read_model};
use prose_yew::{EditConfig, EditMode, OdrlProseView, use_prose_editor};

wasm_bindgen_test_configure!(run_in_browser);

type Log = Rc<RefCell<Vec<EditEvent>>>;

#[derive(Properties, PartialEq, Clone)]
struct HostProps {
    json: AttrValue,
    log: Log,
}

#[function_component(Host)]
fn host(p: &HostProps) -> Html {
    let json = p.json.clone();
    let editor = use_prose_editor(
        move || read_model(&json).expect("a readable fixture"),
        Rc::new(EditRules::default()),
    );
    let onedit = {
        let log = p.log.clone();
        let inner = editor.onedit.clone();
        Callback::from(move |ev: EditEvent| {
            log.borrow_mut().push(ev.clone());
            inner.emit(ev);
        })
    };
    html! {
        <OdrlProseView
            doc={editor.doc.clone()}
            mode={EditMode::Edit}
            config={Rc::new(EditConfig::default())}
            onedit={onedit}
        />
    }
}

const OFFER: &str = include_str!("../../prose-core/tests/fixtures/offer.json");
const TWO_PERMISSIONS: &str = r#"{
  "@type": "Set",
  "uid": "urn:p",
  "permission": [
    {"action": "use", "target": "urn:a"},
    {"action": "display", "target": "urn:b"}
  ]
}"#;

fn document() -> web_sys::Document {
    web_sys::window().unwrap().document().unwrap()
}

async fn settle() {
    TimeoutFuture::new(0).await;
    TimeoutFuture::new(0).await;
}

async fn mount(json: &str) -> (Element, Log) {
    let container = document().create_element("div").unwrap();
    document().body().unwrap().append_child(&container).unwrap();
    let log: Log = Rc::new(RefCell::new(vec![]));
    let props = HostProps {
        json: json.to_string().into(),
        log: log.clone(),
    };
    let handle = yew::Renderer::<Host>::with_root_and_props(container.clone(), props).render();
    std::mem::forget(handle);
    settle().await;
    (container, log)
}

fn q(c: &Element, sel: &str) -> Element {
    c.query_selector(sel)
        .unwrap()
        .unwrap_or_else(|| panic!("no {sel} in:\n{}", c.inner_html()))
}

fn slot(c: &Element, path: &str) -> HtmlElement {
    q(c, &format!("[data-slot=\"{path}\"]"))
        .dyn_into::<HtmlElement>()
        .unwrap()
}

fn bubbling() -> EventInit {
    let init = EventInit::new();
    init.set_bubbles(true);
    init.set_cancelable(true);
    init
}

fn fire_input(el: &Element) {
    let init = InputEventInit::new();
    init.set_bubbles(true);
    el.dispatch_event(&InputEvent::new_with_event_init_dict("input", &init).unwrap())
        .unwrap();
}

fn fire_focusout(el: &Element) {
    let init = FocusEventInit::new();
    init.set_bubbles(true);
    el.dispatch_event(&FocusEvent::new_with_focus_event_init_dict("focusout", &init).unwrap())
        .unwrap();
}

/// Returns whether the default action was left alone.
fn key(el: &Element, key: &str, composing: bool, alt: bool) -> bool {
    let init = KeyboardEventInit::new();
    init.set_bubbles(true);
    init.set_cancelable(true);
    init.set_key(key);
    init.set_is_composing(composing);
    init.set_alt_key(alt);
    el.dispatch_event(&KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).unwrap())
        .unwrap()
}

fn type_into(el: &HtmlElement, text: &str) {
    el.focus().unwrap();
    el.set_text_content(Some(text));
    fire_input(el);
}

fn active_attr(name: &str) -> Option<String> {
    document()
        .active_element()
        .and_then(|e| e.get_attribute(name))
}

fn id_of(json: &str, path: &str) -> NodeId {
    let doc = read_model(json).unwrap();
    doc.node(&path.parse::<NodePath>().unwrap()).unwrap().id()
}

fn set_texts(log: &Log) -> Vec<(String, String)> {
    log.borrow()
        .iter()
        .filter_map(|e| match e {
            EditEvent::SetText { slot, value, .. } => Some((slot.to_string(), value.clone())),
            _ => None,
        })
        .collect()
}

#[wasm_bindgen_test]
async fn blur_commits_exactly_one_set_text() {
    let (c, log) = mount(OFFER).await;
    let el = slot(&c, "policy[0]#uid");
    type_into(&el, "urn:new");
    assert!(log.borrow().is_empty(), "nothing is committed while typing");
    fire_focusout(&el);
    settle().await;
    let events = log.borrow().clone();
    assert_eq!(
        events,
        vec![EditEvent::SetText {
            slot: "policy[0]#uid".parse::<SlotPath>().unwrap(),
            expect: id_of(OFFER, "policy[0]"),
            value: "urn:new".into(),
        }]
    );
    // The slot was remounted and shows the document's text, one text node.
    let again = slot(&c, "policy[0]#uid");
    assert_eq!(again.text_content().unwrap(), "urn:new");
    assert_eq!(again.child_nodes().length(), 1);
}

#[wasm_bindgen_test]
async fn enter_commits_without_a_line_break_and_keeps_focus() {
    let (c, log) = mount(OFFER).await;
    let el = slot(&c, "policy[0]#uid");
    type_into(&el, "urn:enter");
    let not_prevented = key(&el, "Enter", false, false);
    settle().await;
    assert!(!not_prevented, "Enter's default is prevented");
    assert_eq!(
        set_texts(&log),
        vec![("policy[0]#uid".into(), "urn:enter".into())]
    );
    assert_eq!(active_attr("data-slot").as_deref(), Some("policy[0]#uid"));
    assert_eq!(
        slot(&c, "policy[0]#uid").text_content().unwrap(),
        "urn:enter"
    );
}

#[wasm_bindgen_test]
async fn escape_reverts_and_emits_nothing() {
    let (c, log) = mount(OFFER).await;
    let el = slot(&c, "policy[0]#uid");
    let before = el.text_content().unwrap();
    type_into(&el, "something else");
    key(&el, "Escape", false, false);
    settle().await;
    assert_eq!(slot(&c, "policy[0]#uid").text_content().unwrap(), before);
    // The old span is gone; a late focusout does not commit anything.
    fire_focusout(&el);
    settle().await;
    assert!(log.borrow().is_empty());
}

#[wasm_bindgen_test]
async fn paste_inserts_text_never_markup() {
    let (c, log) = mount(OFFER).await;
    let el = slot(&c, "policy[0]#uid");
    el.focus().unwrap();
    let before = el.text_content().unwrap();
    let sel = web_sys::window().unwrap().get_selection().unwrap().unwrap();
    sel.select_all_children(&el).unwrap();
    sel.collapse_to_end().unwrap();
    let dt = DataTransfer::new().unwrap();
    dt.set_data("text/plain", "<b>x</b>\nY").unwrap();
    let init = ClipboardEventInit::new();
    init.set_bubbles(true);
    init.set_cancelable(true);
    init.set_clipboard_data(Some(&dt));
    let ev = ClipboardEvent::new_with_event_init_dict("paste", &init).unwrap();
    let not_prevented = el.dispatch_event(&ev).unwrap();
    assert!(!not_prevented, "the browser's own paste is prevented");
    assert_eq!(el.text_content().unwrap(), format!("{before}<b>x</b> Y"));
    assert_eq!(el.children().length(), 0, "no markup was inserted");
    fire_focusout(&el);
    settle().await;
    assert_eq!(
        set_texts(&log),
        vec![("policy[0]#uid".into(), format!("{before}<b>x</b> Y"))]
    );
}

#[wasm_bindgen_test]
async fn enter_while_composing_does_not_commit() {
    let (c, log) = mount(OFFER).await;
    let el = slot(&c, "policy[0]#uid");
    type_into(&el, "urn:ime");
    key(&el, "Enter", true, false);
    settle().await;
    assert!(log.borrow().is_empty());
}

#[wasm_bindgen_test]
async fn plus_and_minus_emit_events_and_move_focus() {
    let (c, log) = mount(OFFER).await;
    let list: ListPath = "policy[0].permission[0]@constraint".parse().unwrap();
    q(&c, "[data-add=\"policy[0].permission[0]@constraint\"]")
        .dyn_into::<HtmlElement>()
        .unwrap()
        .click();
    settle().await;
    assert!(matches!(
        log.borrow().last(),
        Some(EditEvent::Add { list: l, index: 1, .. }) if *l == list
    ));
    assert_eq!(
        active_attr("data-slot").as_deref(),
        Some("policy[0].permission[0].constraint[1]#leftOperand")
    );

    q(
        &c,
        "[aria-label=\"Remove condition 2 of permission 1 of policy 1\"]",
    )
    .dyn_into::<HtmlElement>()
    .unwrap()
    .click();
    settle().await;
    assert!(matches!(
        log.borrow().last(),
        Some(EditEvent::Remove { list: l, index: 1, .. }) if *l == list
    ));
    assert_eq!(
        active_attr("data-node").as_deref(),
        Some("policy[0].permission[0].constraint[0]")
    );
    assert!(
        c.query_selector("[data-node=\"policy[0].permission[0].constraint[1]\"]")
            .unwrap()
            .is_none()
    );
    let live = q(&c, ".prose-live").text_content().unwrap();
    assert_eq!(live, "condition removed");
}

#[wasm_bindgen_test]
async fn a_dirty_slot_commits_before_a_sibling_is_removed() {
    let (c, log) = mount(TWO_PERMISSIONS).await;
    let el = slot(&c, "policy[0].permission[1].action[0]#name");
    type_into(&el, "print");
    // Pressing a button moves focus first: the slot commits under its own
    // path, then the click removes the other rule.
    fire_focusout(&el);
    q(&c, "[aria-label=\"Remove permission 1 of policy 1\"]")
        .dyn_into::<HtmlElement>()
        .unwrap()
        .click();
    settle().await;
    let events = log.borrow().clone();
    assert!(matches!(
        &events[0],
        EditEvent::SetText { slot, value, .. }
            if slot.to_string() == "policy[0].permission[1].action[0]#name" && value == "print"
    ));
    assert!(matches!(&events[1], EditEvent::Remove { index: 0, .. }));
    assert_eq!(events.len(), 2);
    // The remaining rule is the one that was edited.
    assert_eq!(
        slot(&c, "policy[0].permission[0].action[0]#name")
            .text_content()
            .unwrap(),
        "print"
    );
    assert!(
        c.query_selector("[data-slot=\"policy[0].permission[1].action[0]#name\"]")
            .unwrap()
            .is_none()
    );
}

#[wasm_bindgen_test]
async fn suggestions_are_picked_with_the_keyboard() {
    let (c, log) = mount(TWO_PERMISSIONS).await;
    let path = "policy[0].permission[0].action[0]#name";
    let el = slot(&c, path);
    type_into(&el, "dis");
    settle().await;
    let options = c.query_selector_all("[role=option]").unwrap();
    assert_eq!(options.length(), 2, "display, distribute");
    assert_eq!(el.get_attribute("aria-expanded").as_deref(), Some("true"));
    key(&el, "ArrowDown", false, false);
    settle().await;
    key(&el, "Enter", false, false);
    settle().await;
    assert_eq!(
        set_texts(&log),
        vec![(path.to_string(), "distribute".to_string())]
    );
    assert_eq!(slot(&c, path).text_content().unwrap(), "distribute");
    assert!(c.query_selector("[role=listbox]").unwrap().is_none());
}

#[wasm_bindgen_test]
async fn changing_the_operator_emits_set_choice() {
    let (c, log) = mount(OFFER).await;
    let select: HtmlSelectElement = slot(&c, "policy[0].permission[0].constraint[0]#operator")
        .dyn_into()
        .unwrap();
    assert_eq!(select.value(), "lt");
    select.set_value("neq");
    select
        .dispatch_event(&Event::new_with_event_init_dict("change", &bubbling()).unwrap())
        .unwrap();
    settle().await;
    assert!(matches!(
        log.borrow().last(),
        Some(EditEvent::SetChoice { slot, value, .. })
            if slot.to_string() == "policy[0].permission[0].constraint[0]#operator" && value == "neq"
    ));
    let select: HtmlSelectElement = slot(&c, "policy[0].permission[0].constraint[0]#operator")
        .dyn_into()
        .unwrap();
    assert_eq!(select.value(), "neq");
}

const ACTION: &str = "policy[0].permission[0].action[0]#name";

fn key_code(el: &Element, key: &str, code: u32) -> bool {
    let init = KeyboardEventInit::new();
    init.set_bubbles(true);
    init.set_cancelable(true);
    init.set_key(key);
    init.set_key_code(code);
    el.dispatch_event(&KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).unwrap())
        .unwrap()
}

fn paste_at_end(el: &HtmlElement, text: &str) {
    let sel = web_sys::window().unwrap().get_selection().unwrap().unwrap();
    sel.select_all_children(el).unwrap();
    sel.collapse_to_end().unwrap();
    let dt = DataTransfer::new().unwrap();
    dt.set_data("text/plain", text).unwrap();
    let init = ClipboardEventInit::new();
    init.set_bubbles(true);
    init.set_cancelable(true);
    init.set_clipboard_data(Some(&dt));
    el.dispatch_event(&ClipboardEvent::new_with_event_init_dict("paste", &init).unwrap())
        .unwrap();
}

#[wasm_bindgen_test]
async fn enter_commits_the_typed_text_not_the_first_suggestion() {
    let (c, log) = mount(TWO_PERMISSIONS).await;
    let el = slot(&c, ACTION);
    type_into(&el, "dis");
    settle().await;
    assert!(c.query_selector("[role=listbox]").unwrap().is_some());
    key(&el, "Enter", false, false);
    settle().await;
    assert_eq!(
        set_texts(&log),
        vec![(ACTION.to_string(), "dis".to_string())]
    );
}

#[wasm_bindgen_test]
async fn enter_on_a_cleared_slot_commits_the_empty_text() {
    let (c, log) = mount(TWO_PERMISSIONS).await;
    let el = slot(&c, ACTION);
    type_into(&el, "");
    settle().await;
    key(&el, "Enter", false, false);
    settle().await;
    assert_eq!(set_texts(&log), vec![(ACTION.to_string(), String::new())]);
}

#[wasm_bindgen_test]
async fn paste_refreshes_the_suggestions_so_enter_commits_the_pasted_text() {
    let (c, log) = mount(TWO_PERMISSIONS).await;
    let el = slot(&c, ACTION);
    type_into(&el, "dis");
    settle().await;
    paste_at_end(&el, "Xmine");
    settle().await;
    // The list was for "dis"; for "disXmine" nothing matches, so it is gone.
    assert!(c.query_selector("[role=listbox]").unwrap().is_none());
    key(&el, "Enter", false, false);
    settle().await;
    assert_eq!(
        set_texts(&log),
        vec![(ACTION.to_string(), "disXmine".to_string())]
    );
}

#[wasm_bindgen_test]
async fn an_ime_confirming_enter_with_key_code_229_does_not_commit() {
    let (c, log) = mount(OFFER).await;
    let el = slot(&c, "policy[0]#uid");
    type_into(&el, "urn:ime");
    key_code(&el, "Enter", 229);
    settle().await;
    assert!(log.borrow().is_empty());
}

#[wasm_bindgen_test]
async fn one_edited_character_does_not_rewrite_the_rest_of_the_value() {
    let json = "{\"@type\":\"Set\",\"uid\":\"urn:p\\tq\",\"permission\":[{\"action\":\"use\",\"target\":\"urn:a\"}]}";
    let (c, log) = mount(json).await;
    let el = slot(&c, "policy[0]#uid");
    assert_eq!(el.text_content().unwrap(), "urn:p\tq");
    type_into(&el, "urn:p\tqx");
    fire_focusout(&el);
    settle().await;
    assert_eq!(
        set_texts(&log),
        vec![("policy[0]#uid".to_string(), "urn:p\tqx".to_string())]
    );
}

#[wasm_bindgen_test]
async fn a_select_keeps_focus_when_it_changes() {
    let (c, log) = mount(OFFER).await;
    let path = "policy[0].permission[0].constraint[0]#operator";
    let select: HtmlSelectElement = slot(&c, path).dyn_into().unwrap();
    select.focus().unwrap();
    select.set_value("neq");
    select
        .dispatch_event(&Event::new_with_event_init_dict("change", &bubbling()).unwrap())
        .unwrap();
    settle().await;
    assert!(
        matches!(log.borrow().last(), Some(EditEvent::SetChoice { value, .. }) if value == "neq")
    );
    assert_eq!(active_attr("data-slot").as_deref(), Some(path));
    // A second step with the arrow keys works from the same element.
    let select: HtmlSelectElement = slot(&c, path).dyn_into().unwrap();
    select.set_value("gt");
    select
        .dispatch_event(&Event::new_with_event_init_dict("change", &bubbling()).unwrap())
        .unwrap();
    settle().await;
    assert_eq!(active_attr("data-slot").as_deref(), Some(path));
}

#[wasm_bindgen_test]
async fn changing_a_rule_kind_leaves_focus_on_its_select() {
    let (c, _log) = mount(TWO_PERMISSIONS).await;
    let select: HtmlSelectElement = q(
        &c,
        "select.prose-rule-kind[data-node=\"policy[0].permission[0]\"]",
    )
    .dyn_into()
    .unwrap();
    select.focus().unwrap();
    select.set_value("prohibition");
    select
        .dispatch_event(&Event::new_with_event_init_dict("change", &bubbling()).unwrap())
        .unwrap();
    settle().await;
    let active = document().active_element().expect("something has focus");
    assert_eq!(active.tag_name(), "SELECT");
    assert_eq!(
        active.get_attribute("data-node").as_deref(),
        Some("policy[0].prohibition[0]")
    );
}
