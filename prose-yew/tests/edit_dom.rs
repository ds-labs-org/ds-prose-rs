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
        Some(EditEvent::Add { list: l, index: 1, expect: Some(_), .. }) if *l == list
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

// --- audit findings -------------------------------------------------------------

use prose_yew::ProseEditor;
use web_sys::{MouseEvent, MouseEventInit, MutationObserver, MutationObserverInit};

type Handle = Rc<RefCell<Option<ProseEditor>>>;

#[derive(Properties, PartialEq, Clone)]
struct HostBheProps {
    json: AttrValue,
    log: Log,
    handle: Handle,
    /// Events of this kind never reach the hook (a host that refuses them).
    drop_remove: bool,
}

/// The README host: `use_prose_editor`, an Undo button beside the view, and
/// an `onedit` wrapper that can drop events. Exposes the editor to the test.
#[function_component(HostB)]
fn host_b(p: &HostBheProps) -> Html {
    let json = p.json.clone();
    let editor = use_prose_editor(
        move || read_model(&json).expect("a readable fixture"),
        Rc::new(EditRules::default()),
    );
    *p.handle.borrow_mut() = Some(editor.clone());
    let onedit = {
        let log = p.log.clone();
        let inner = editor.onedit.clone();
        let drop_remove = p.drop_remove;
        Callback::from(move |ev: EditEvent| {
            log.borrow_mut().push(ev.clone());
            if drop_remove && matches!(ev, EditEvent::Remove { .. }) {
                return;
            }
            inner.emit(ev);
        })
    };
    let undo = {
        let undo = editor.undo.clone();
        Callback::from(move |_| undo.emit(()))
    };
    html! {
        <>
            <button id="host-undo" type="button" disabled={!editor.can_undo} onclick={undo}>{ "Undo" }</button>
            <OdrlProseView
                doc={editor.doc.clone()}
                mode={EditMode::Edit}
                config={Rc::new(EditConfig::default())}
                onedit={onedit}
            />
        </>
    }
}

async fn mount_b(json: &str, drop_remove: bool) -> (Element, Log, Handle) {
    let container = document().create_element("div").unwrap();
    document().body().unwrap().append_child(&container).unwrap();
    let log: Log = Rc::new(RefCell::new(vec![]));
    let handle: Handle = Rc::new(RefCell::new(None));
    let props = HostBheProps {
        json: json.to_string().into(),
        log: log.clone(),
        handle: handle.clone(),
        drop_remove,
    };
    std::mem::forget(
        yew::Renderer::<HostB>::with_root_and_props(container.clone(), props).render(),
    );
    settle().await;
    (container, log, handle)
}

fn click(el: &Element) {
    el.clone().dyn_into::<HtmlElement>().unwrap().click();
}

#[wasm_bindgen_test]
async fn a_refused_structural_event_has_no_later_effect() {
    let (c, _log, _h) = mount_b(OFFER, true).await;
    // Something to undo, so the host's Undo button can take focus.
    for v in ["neq", "gt"] {
        let select: HtmlSelectElement = slot(&c, "policy[0].permission[0].constraint[0]#operator")
            .dyn_into()
            .unwrap();
        select.set_value(v);
        select
            .dispatch_event(&Event::new_with_event_init_dict("change", &bubbling()).unwrap())
            .unwrap();
        settle().await;
    }
    // The host drops this Remove: nothing changes.
    click(&q(
        &c,
        "[aria-label=\"Remove condition 1 of permission 1 of policy 1\"]",
    ));
    settle().await;
    assert!(
        c.query_selector("[data-node=\"policy[0].permission[0].constraint[0]\"]")
            .unwrap()
            .is_some()
    );
    // A later, unrelated change of the document (Undo) must not run the
    // dropped event's focus move or announcement.
    let undo = q(&c, "#host-undo").dyn_into::<HtmlElement>().unwrap();
    undo.focus().unwrap();
    undo.click();
    settle().await;
    assert_eq!(
        document()
            .active_element()
            .and_then(|e| e.get_attribute("id"))
            .as_deref(),
        Some("host-undo"),
        "focus stays on the button the user pressed"
    );
    assert_eq!(q(&c, ".prose-live").text_content().unwrap(), "");
}

#[wasm_bindgen_test]
async fn a_host_change_under_a_dirty_slot_does_not_lose_the_typed_text() {
    let (c, log, h) = mount_b(OFFER, false).await;
    let path = "policy[0]#uid";
    let el = slot(&c, path);
    el.focus().unwrap();
    el.first_child()
        .unwrap()
        .set_node_value(Some("urn:typed-by-user"));
    fire_input(&el);
    settle().await;
    let editor = h.borrow().clone().unwrap();
    editor.onedit.emit(EditEvent::SetText {
        slot: path.parse::<SlotPath>().unwrap(),
        expect: id_of(OFFER, "policy[0]"),
        value: "urn:changed-externally".into(),
    });
    settle().await;
    settle().await;
    assert_eq!(
        slot(&c, path).text_content().unwrap(),
        "urn:typed-by-user",
        "the user's text is still there"
    );
    fire_focusout(&slot(&c, path));
    settle().await;
    assert_eq!(
        set_texts(&log),
        vec![(path.to_string(), "urn:typed-by-user".to_string())],
        "and is committed on blur"
    );
}

#[wasm_bindgen_test]
async fn the_same_announcement_twice_changes_the_live_region_twice() {
    let (c, _log, _h) = mount_b(OFFER, false).await;
    let live = q(&c, ".prose-live");
    // The observer's callback runs in a microtask, so it is the one that counts.
    let batches = Rc::new(std::cell::Cell::new(0u32));
    let observer = {
        let batches = batches.clone();
        let cb = wasm_bindgen::closure::Closure::<
            dyn FnMut(wasm_bindgen::JsValue, wasm_bindgen::JsValue),
        >::new(move |_, _| batches.set(batches.get() + 1));
        let o = MutationObserver::new(cb.as_ref().unchecked_ref()).unwrap();
        std::mem::forget(cb);
        o
    };
    let init = MutationObserverInit::new();
    init.set_child_list(true);
    init.set_character_data(true);
    init.set_subtree(true);
    observer.observe_with_options(&live, &init).unwrap();
    let add = "[data-add=\"policy[0].permission[0]@constraint\"]";
    for round in 1..=2 {
        batches.set(0);
        click(&q(&c, add));
        settle().await;
        settle().await;
        assert_eq!(live.text_content().unwrap(), "condition added");
        assert!(
            batches.get() > 0,
            "round {round}: assistive technology saw a change"
        );
    }
}

#[wasm_bindgen_test]
async fn hovering_an_option_moves_the_active_descendant() {
    let (c, _log) = mount(TWO_PERMISSIONS).await;
    let el = slot(&c, ACTION);
    type_into(&el, "d");
    settle().await;
    let options = c.query_selector_all("[role=option]").unwrap();
    assert!(options.length() >= 3);
    let third = options.item(2).unwrap().dyn_into::<Element>().unwrap();
    let init = MouseEventInit::new();
    init.set_bubbles(true);
    third
        .dispatch_event(&MouseEvent::new_with_mouse_event_init_dict("mousemove", &init).unwrap())
        .unwrap();
    settle().await;
    let want = third.id();
    assert_eq!(
        slot(&c, ACTION)
            .get_attribute("aria-activedescendant")
            .as_deref(),
        Some(want.as_str())
    );
    let third = q(&c, &format!("[id=\"{want}\"]"));
    assert_eq!(
        third.get_attribute("aria-selected").as_deref(),
        Some("true")
    );
    assert!(third.class_name().contains("prose-suggestion-active"));
}

#[wasm_bindgen_test]
async fn the_rule_kind_select_does_not_offer_a_kind_the_rules_forbid() {
    use prose_core::edit::Limit;
    let container = document().create_element("div").unwrap();
    document().body().unwrap().append_child(&container).unwrap();
    let mut cfg = EditConfig::default();
    cfg.rules.prohibition = Limit::NONE;
    let cfg = Rc::new(cfg);
    let doc = Rc::new(read_model(TWO_PERMISSIONS).unwrap());
    let props = yew::props!(prose_yew::OdrlProseViewProps {
        doc: doc,
        mode: EditMode::Edit,
        config: cfg,
    });
    std::mem::forget(
        yew::Renderer::<OdrlProseView>::with_root_and_props(container.clone(), props).render(),
    );
    settle().await;
    let select: HtmlSelectElement = q(
        &container,
        "select.prose-rule-kind[data-node=\"policy[0].permission[0]\"]",
    )
    .dyn_into()
    .unwrap();
    assert_eq!(
        select.query_selector_all("option").unwrap().length(),
        2,
        "{}",
        select.outer_html()
    );
    assert!(
        container
            .query_selector("option[value=\"prohibition\"]")
            .unwrap()
            .is_none()
    );
}

#[derive(Properties, PartialEq)]
struct SpinProps {
    tick: usize,
}

/// A slow sibling: more than Yew's 16 ms budget per scheduler run, so the
/// scheduler yields to the browser (a task boundary) before the view renders.
#[function_component(Spin)]
fn spin(_: &SpinProps) -> Html {
    let t = web_sys::js_sys::Date::now();
    while web_sys::js_sys::Date::now() - t < 40.0 {}
    html! {}
}

#[function_component(SlowHost)]
fn slow_host(p: &HostProps) -> Html {
    let json = p.json.clone();
    let editor = use_prose_editor(
        move || read_model(&json).expect("a readable fixture"),
        Rc::new(EditRules::default()),
    );
    let ticks = use_mut_ref(|| 0usize);
    *ticks.borrow_mut() += 1;
    let tick = *ticks.borrow();
    html! {
        <>
            <Spin tick={tick} />
            <OdrlProseView
                doc={editor.doc.clone()}
                mode={EditMode::Edit}
                config={Rc::new(EditConfig::default())}
                onedit={editor.onedit.clone()}
            />
        </>
    }
}

#[wasm_bindgen_test]
async fn a_slow_render_does_not_lose_the_focus_move_or_the_announcement() {
    let container = document().create_element("div").unwrap();
    document().body().unwrap().append_child(&container).unwrap();
    let props = HostProps {
        json: OFFER.into(),
        log: Rc::new(RefCell::new(vec![])),
    };
    std::mem::forget(
        yew::Renderer::<SlowHost>::with_root_and_props(container.clone(), props).render(),
    );
    settle().await;
    click(&q(
        &container,
        "[data-add=\"policy[0].permission[0]@constraint\"]",
    ));
    // The slow render is split by a task boundary; wait it out.
    TimeoutFuture::new(200).await;
    assert_eq!(
        active_attr("data-slot").as_deref(),
        Some("policy[0].permission[0].constraint[1]#leftOperand")
    );
    assert_eq!(
        q(&container, ".prose-live").text_content().unwrap(),
        "condition added"
    );
}

// --- audit round 4 ---------------------------------------------------------------

#[wasm_bindgen_test]
async fn a_refused_event_expires_without_any_user_input() {
    let (c, _log, h) = mount_b(OFFER, true).await;
    click(&q(
        &c,
        "[aria-label=\"Remove condition 1 of permission 1 of policy 1\"]",
    ));
    settle().await;
    // No user input at all; the pending event must lapse by itself.
    TimeoutFuture::new(900).await;
    let before = document().active_element().map(|e| e.tag_name());
    // A collaborator, autosave or host timer changes the document.
    let editor = h.borrow().clone().unwrap();
    editor.onedit.emit(EditEvent::SetText {
        slot: "policy[0]#uid".parse::<SlotPath>().unwrap(),
        expect: id_of(OFFER, "policy[0]"),
        value: "urn:changed-externally".into(),
    });
    settle().await;
    settle().await;
    assert_eq!(
        q(&c, ".prose-live").text_content().unwrap(),
        "",
        "a removal that never happened is not announced"
    );
    assert_eq!(
        document().active_element().map(|e| e.tag_name()),
        before,
        "focus stays where it was"
    );
    assert!(
        c.query_selector("[data-node=\"policy[0].permission[0].constraint[0]\"]")
            .unwrap()
            .is_some()
    );
}

/// Wrap `document.addEventListener` and `removeEventListener` so the test can
/// count the capture-phase pointer, mouse, key and click listeners that are
/// registered right now. `window.__live()` returns that number.
fn count_document_listeners() {
    web_sys::js_sys::eval(
        r#"
        (function () {
          if (window.__live) { window.__reset(); return; }
          const live = new Map();
          const kinds = ["pointerdown", "mousedown", "keydown", "click"];
          const key = (t, f, o) => t + (o === true || (o && o.capture) ? "/c" : "/b");
          const add = document.addEventListener.bind(document);
          const rm = document.removeEventListener.bind(document);
          document.addEventListener = function (t, f, o) {
            if (kinds.includes(t)) {
              const k = key(t, f, o);
              if (!live.has(k)) live.set(k, new Set());
              live.get(k).add(f);
              if (o && o.once) {
                // A once-listener unregisters itself when it fires.
                return add(t, function (e) { live.get(k).delete(f); return f.call(this, e); }, o);
              }
            }
            return add(t, f, o);
          };
          document.removeEventListener = function (t, f, o) {
            if (kinds.includes(t)) {
              const s = live.get(key(t, f, o));
              if (s) s.delete(f);
            }
            return rm(t, f, o);
          };
          window.__live = () => { let n = 0; for (const [k, s] of live) if (k.endsWith("/c")) n += s.size; return n; };
          window.__reset = () => live.clear();
        })()
        "#,
    )
    .unwrap();
}

fn live_listeners() -> u32 {
    web_sys::js_sys::eval("window.__live()")
        .unwrap()
        .as_f64()
        .unwrap() as u32
}

fn keydown_on_document() {
    let init = KeyboardEventInit::new();
    init.set_bubbles(true);
    init.set_key("Tab");
    document()
        .dispatch_event(
            &KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).unwrap(),
        )
        .unwrap();
}

#[wasm_bindgen_test]
async fn a_keydown_removes_every_expiry_listener() {
    count_document_listeners();
    let (c, _log, _h) = mount_b(OFFER, true).await;
    click(&q(
        &c,
        "[aria-label=\"Remove condition 1 of permission 1 of policy 1\"]",
    ));
    settle().await;
    assert!(
        live_listeners() > 0,
        "a refused event waits for the next input"
    );
    keydown_on_document();
    assert_eq!(live_listeners(), 0, "no expiry listener is left behind");
}

#[wasm_bindgen_test]
async fn unmounting_removes_the_expiry_listeners() {
    count_document_listeners();
    let container = document().create_element("div").unwrap();
    document().body().unwrap().append_child(&container).unwrap();
    let props = HostBheProps {
        json: OFFER.into(),
        log: Rc::new(RefCell::new(vec![])),
        handle: Rc::new(RefCell::new(None)),
        drop_remove: true,
    };
    let app = yew::Renderer::<HostB>::with_root_and_props(container.clone(), props).render();
    settle().await;
    click(&q(
        &container,
        "[aria-label=\"Remove condition 1 of permission 1 of policy 1\"]",
    ));
    settle().await;
    assert!(live_listeners() > 0);
    app.destroy();
    settle().await;
    assert_eq!(live_listeners(), 0, "the view's listeners die with it");
}

#[wasm_bindgen_test]
async fn an_answered_event_removes_its_expiry_listeners() {
    count_document_listeners();
    let (c, _log, _h) = mount_b(OFFER, false).await;
    click(&q(&c, "[data-add=\"policy[0].permission[0]@constraint\"]"));
    settle().await;
    settle().await;
    assert_eq!(
        q(&c, ".prose-live").text_content().unwrap(),
        "condition added"
    );
    assert_eq!(
        live_listeners(),
        0,
        "the host answered; nothing waits any more"
    );
}

// --- profiles -------------------------------------------------------------------

use prose_core::profile::{ProfileVocab, Wording};

const RESEARCH: &str = include_str!("../../prose-core/tests/profiles/research.jsonld");
const RESEARCH_POLICY: &str = include_str!("../../prose-core/tests/profiles/research-policy.json");

fn research_wording() -> Rc<Wording> {
    Rc::new(Wording::new().with_profile(ProfileVocab::parse(RESEARCH).unwrap()))
}

#[derive(Properties, PartialEq, Clone)]
struct WordedProps {
    json: AttrValue,
    log: Log,
    /// Compared by identity, as a host keeps one per loaded profile.
    wording: Option<WordingRef>,
}

#[derive(Clone)]
struct WordingRef(Rc<Wording>);

impl PartialEq for WordingRef {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// A host with an undo button whose configuration follows its `wording`,
/// as the demo's Edit tab builds it.
#[function_component(WordedHost)]
fn worded_host(p: &WordedProps) -> Html {
    let json = p.json.clone();
    let editor = use_prose_editor(
        move || read_model(&json).expect("a readable fixture"),
        Rc::new(EditRules::default()),
    );
    let config = use_memo(p.wording.clone(), |w| match w {
        Some(w) => EditConfig::default().with_wording(w.0.clone()),
        None => EditConfig::default(),
    });
    let onedit = {
        let log = p.log.clone();
        let inner = editor.onedit.clone();
        Callback::from(move |ev: EditEvent| {
            log.borrow_mut().push(ev.clone());
            inner.emit(ev);
        })
    };
    let undo = {
        let undo = editor.undo.clone();
        Callback::from(move |_| undo.emit(()))
    };
    html! {
        <>
            <button id="worded-undo" type="button" disabled={!editor.can_undo} onclick={undo}>{ "Undo" }</button>
            <OdrlProseView
                doc={editor.doc.clone()}
                mode={EditMode::Edit}
                config={config}
                onedit={onedit}
            />
        </>
    }
}

async fn mount_worded(
    json: &str,
    wording: Option<Rc<Wording>>,
) -> (Element, Log, yew::AppHandle<WordedHost>) {
    let container = document().create_element("div").unwrap();
    document().body().unwrap().append_child(&container).unwrap();
    let log: Log = Rc::new(RefCell::new(vec![]));
    let props = WordedProps {
        json: json.to_string().into(),
        log: log.clone(),
        wording: wording.map(WordingRef),
    };
    let handle =
        yew::Renderer::<WordedHost>::with_root_and_props(container.clone(), props).render();
    settle().await;
    (container, log, handle)
}

fn mousedown(el: &Element) {
    let init = MouseEventInit::new();
    init.set_bubbles(true);
    init.set_cancelable(true);
    el.dispatch_event(&MouseEvent::new_with_mouse_event_init_dict("mousedown", &init).unwrap())
        .unwrap();
}

/// The options of the open list: (label without the hint, class, depth).
fn shown_options(c: &Element) -> Vec<(String, String)> {
    let list = c.query_selector_all("[role=option]").unwrap();
    (0..list.length())
        .map(|i| {
            let el = list.item(i).unwrap().dyn_into::<Element>().unwrap();
            let label = el
                .first_child()
                .and_then(|n| n.text_content())
                .unwrap_or_default();
            (label, el.class_name())
        })
        .collect()
}

#[wasm_bindgen_test]
async fn typing_fi_lists_fine_tune_first_and_pick_writes_ex_fine_tune_or_iri() {
    let distill = "policy[0].permission[1].action[0]#name";
    let (c, log, _h) = mount_worded(RESEARCH_POLICY, Some(research_wording())).await;
    let el = slot(&c, distill);
    type_into(&el, "fi");
    settle().await;
    let options = shown_options(&c);
    assert_eq!(options[0].0, "Fine-tune", "{options:?}");
    // The profile's group is headed by its title.
    let group = q(&c, "[role=listbox] [role=group]");
    let heading = group.get_attribute("aria-labelledby").unwrap();
    assert_eq!(
        q(&c, &format!("[id=\"{heading}\"]"))
            .text_content()
            .as_deref(),
        Some("From profile: Research Data Profile")
    );
    // The definition is the hint.
    let first = c
        .query_selector_all("[role=option]")
        .unwrap()
        .item(0)
        .unwrap();
    assert!(
        first
            .text_content()
            .unwrap()
            .contains("To adapt an already trained model"),
        "{:?}",
        first.text_content()
    );
    mousedown(&first.dyn_into::<Element>().unwrap());
    settle().await;
    // The policy binds ex:, so the pick is written as a compact IRI.
    assert_eq!(
        set_texts(&log),
        vec![(distill.to_string(), "ex:fineTune".to_string())]
    );

    // A policy that does not bind the prefix gets the full IRI.
    let json = r#"{"@type": "Set", "uid": "urn:p", "permission": [{"action": "use"}]}"#;
    let (c, log, _h) = mount_worded(json, Some(research_wording())).await;
    let path = "policy[0].permission[0].action[0]#name";
    type_into(&slot(&c, path), "fi");
    settle().await;
    let first = q(&c, "[role=option]");
    mousedown(&first);
    settle().await;
    assert_eq!(
        set_texts(&log),
        vec![(
            path.to_string(),
            "https://example.org/odrl/profile/research/fineTune".to_string()
        )]
    );
}

#[wasm_bindgen_test]
async fn right_operand_slot_offers_academic_then_indented_clinical() {
    let (c, log, _h) = mount_worded(RESEARCH_POLICY, Some(research_wording())).await;
    let path = "policy[0].permission[0].constraint[0]#rightOperand[1]";
    let el = slot(&c, path);
    type_into(&el, "");
    settle().await;
    let options = shown_options(&c);
    let labels: Vec<&str> = options.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Academic research",
            "Clinical research",
            "Commercial research"
        ],
        "{options:?}"
    );
    assert!(!options[0].1.contains("prose-suggestion-nested"));
    assert!(
        options[1].1.contains("prose-suggestion-nested"),
        "{options:?}"
    );
    assert!(!options[2].1.contains("prose-suggestion-nested"));
    // No group heading: these are the left operand's own values.
    assert!(
        c.query_selector("[role=listbox] [role=group]")
            .unwrap()
            .is_none()
    );
    let third = c
        .query_selector_all("[role=option]")
        .unwrap()
        .item(2)
        .unwrap();
    mousedown(&third.dyn_into::<Element>().unwrap());
    settle().await;
    assert_eq!(
        set_texts(&log),
        vec![(path.to_string(), "ex:commercialResearch".to_string())]
    );

    // A left operand without suggested values gets every profile value,
    // under a heading that says they are not specific to it.
    let json = RESEARCH_POLICY.replace("ex:researchPurpose", "purpose");
    let (c, _log, _h) = mount_worded(&json, Some(research_wording())).await;
    type_into(&slot(&c, path), "");
    settle().await;
    let group = q(&c, "[role=listbox] [role=group]");
    let heading = group.get_attribute("aria-labelledby").unwrap();
    assert_eq!(
        q(&c, &format!("[id=\"{heading}\"]"))
            .text_content()
            .as_deref(),
        Some("Profile values (not specific to this operand)")
    );
    assert_eq!(shown_options(&c).len(), 3);
}

#[wasm_bindgen_test]
async fn switching_wording_rerenders_without_resetting_doc_or_undo() {
    let (c, _log, mut handle) = mount_worded(RESEARCH_POLICY, None).await;
    let action = "policy[0].permission[0].action[0]#name";
    let before = slot(&c, action).get_attribute("title").unwrap();
    assert_ne!(before, "fine-tune");
    // An edit, then a profile is loaded.
    let uid = slot(&c, "policy[0]#uid");
    type_into(&uid, "urn:policy:changed");
    fire_focusout(&uid);
    settle().await;
    let undo = q(&c, "#worded-undo");
    assert!(!undo.has_attribute("disabled"));
    let log: Log = Rc::new(RefCell::new(vec![]));
    handle.update(WordedProps {
        json: RESEARCH_POLICY.into(),
        log: log.clone(),
        wording: Some(WordingRef(research_wording())),
    });
    settle().await;
    assert_eq!(
        slot(&c, action).get_attribute("title").as_deref(),
        Some("fine-tune")
    );
    assert_eq!(
        slot(&c, "policy[0]#uid").text_content().as_deref(),
        Some("urn:policy:changed")
    );
    let undo = q(&c, "#worded-undo");
    assert!(!undo.has_attribute("disabled"), "the undo history is kept");
    // And unloaded again: the built-in wording, the edit still there.
    handle.update(WordedProps {
        json: RESEARCH_POLICY.into(),
        log,
        wording: None,
    });
    settle().await;
    assert_eq!(slot(&c, action).get_attribute("title").unwrap(), before);
    click(&q(&c, "#worded-undo"));
    settle().await;
    assert_eq!(
        slot(&c, "policy[0]#uid").text_content().as_deref(),
        Some("urn:policy:research-1")
    );
}

const UNIT_PROFILE: &str = r#"{
    "@context": {"odrl": "http://www.w3.org/ns/odrl/2/", "ex": "https://example.org/u/",
                 "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
                 "owl": "http://www.w3.org/2002/07/owl#",
                 "prose": "https://ds-labs-org.github.io/ds-prose-rs/profile#"},
    "@graph": [
        {"@id": "ex:size", "@type": "odrl:LeftOperand", "rdfs:label": "Size",
         "prose:unit": ["ex:gigabyte", "ex:terabyte"]},
        {"@id": "ex:tier", "@type": "odrl:LeftOperand", "rdfs:label": "Tier"},
        {"@id": "ex:gold", "@type": "odrl:RightOperand", "rdfs:label": "Gold"},
        {"@id": "ex:tin", "@type": "odrl:RightOperand", "rdfs:label": "Tin",
         "owl:deprecated": true}
    ]
}"#;

const UNIT_POLICY: &str = r#"{
    "@context": {"ex": "https://example.org/u/"},
    "@type": "Set", "uid": "urn:p",
    "permission": [{"action": "use", "constraint": [
        {"leftOperand": "ex:size", "operator": "lteq", "rightOperand": 10, "unit": "ex:gigabyte"},
        {"leftOperand": "ex:tier", "operator": "eq", "rightOperand": {"@id": "ex:gold"}}
    ]}]
}"#;

#[wasm_bindgen_test]
async fn unit_slot_offers_the_left_operands_units() {
    let w = Rc::new(Wording::new().with_profile(ProfileVocab::parse(UNIT_PROFILE).unwrap()));
    let (c, log, _h) = mount_worded(UNIT_POLICY, Some(w)).await;
    let path = "policy[0].permission[0].constraint[0]#unit";
    type_into(&slot(&c, path), "");
    settle().await;
    let options = shown_options(&c);
    assert_eq!(options.len(), 2, "{options:?}");
    let second = c
        .query_selector_all("[role=option]")
        .unwrap()
        .item(1)
        .unwrap();
    mousedown(&second.dyn_into::<Element>().unwrap());
    settle().await;
    // Written with the prefix the policy binds.
    assert_eq!(
        set_texts(&log),
        vec![(path.to_string(), "ex:terabyte".to_string())]
    );
}

#[wasm_bindgen_test]
async fn profile_values_leave_out_deprecated_and_list_each_once() {
    // A second profile declaring the same value: listed once.
    let again = r#"{
        "@context": {"odrl": "http://www.w3.org/ns/odrl/2/", "ex": "https://example.org/u/"},
        "@graph": [{"@id": "ex:gold", "@type": "odrl:RightOperand"}]
    }"#;
    let w = Rc::new(
        Wording::new()
            .with_profile(ProfileVocab::parse(UNIT_PROFILE).unwrap())
            .with_profile(ProfileVocab::parse(again).unwrap()),
    );
    let (c, _log, _h) = mount_worded(UNIT_POLICY, Some(w)).await;
    type_into(
        &slot(&c, "policy[0].permission[0].constraint[1]#rightOperand[0]"),
        "",
    );
    settle().await;
    let labels: Vec<String> = shown_options(&c).into_iter().map(|(l, _)| l).collect();
    assert_eq!(labels, ["Gold"]);
}

fn set_iris(log: &Log) -> Vec<(String, String)> {
    log.borrow()
        .iter()
        .filter_map(|e| match e {
            EditEvent::SetIri { slot, value, .. } => Some((slot.to_string(), value.clone())),
            _ => None,
        })
        .collect()
}

#[wasm_bindgen_test]
async fn a_value_picked_into_a_text_slot_is_written_as_an_iri() {
    // The slot holds a plain string: picking a profile value must turn it
    // into an IRI reference, not the string "ex:academic".
    let json = r#"{"@context": {"ex": "https://example.org/odrl/profile/research/"},
        "@type": "Set", "uid": "urn:p",
        "permission": [{"action": "use", "target": "urn:a",
          "constraint": [{"leftOperand": "ex:researchPurpose", "operator": "eq",
                          "rightOperand": "tbd"}]}]}"#;
    let (c, log, _h) = mount_worded(json, Some(research_wording())).await;
    let path = "policy[0].permission[0].constraint[0]#rightOperand[0]";
    type_into(&slot(&c, path), "");
    settle().await;
    mousedown(&q(&c, "[role=option]"));
    settle().await;
    assert_eq!(set_texts(&log), vec![]);
    assert_eq!(
        set_iris(&log),
        vec![(path.to_string(), "ex:academic".to_string())]
    );
    // The slot now shows the value's label.
    assert_eq!(
        slot(&c, path).get_attribute("title").as_deref(),
        Some("academic research")
    );
}

#[wasm_bindgen_test]
async fn a_typed_slot_is_not_offered_unrelated_profile_values() {
    // ex:dataSubjectCount says prose:datatype xsd:integer and suggests no
    // values: the profile's IRI values do not fit it.
    let (c, _log, _h) = mount_worded(RESEARCH_POLICY, Some(research_wording())).await;
    let path = "policy[0].permission[0].action[0].refinement[0]#rightOperand[0]";
    type_into(&slot(&c, path), "");
    settle().await;
    assert_eq!(shown_options(&c), vec![]);
}

// --- strict validation ------------------------------------------------------------

use prose_core::validate::Options;
use prose_yew::{ProseProblems, StrictEdit, use_prose_editor_with, use_validation};

const CLOSED: &str = include_str!("../../prose-core/tests/profiles/research-closed.jsonld");

/// A Set that conforms to the closed research profile in strict mode.
const STRICT_POLICY: &str = r#"{
  "@context": ["http://www.w3.org/ns/odrl.jsonld",
               {"ex": "https://example.org/odrl/profile/research/",
                "foo": "https://example.net/foo/"}],
  "@type": "Set", "uid": "urn:policy:s",
  "profile": "https://example.org/odrl/profile/research",
  "permission": [{
    "target": "urn:asset:1", "assigner": "urn:party:a", "action": "use",
    "constraint": [{"leftOperand": "ex:researchPurpose", "operator": "isAnyOf",
                    "rightOperand": [{"@id": "ex:academic"}]}]
  }]
}"#;

/// The same with `foo:bar` as the action, which the profile does not allow.
fn violating_policy() -> String {
    STRICT_POLICY.replace(r#""action": "use""#, r#""action": "foo:bar""#)
}

/// `ex:train` is used without the policy declaring the profile.
fn undeclared_policy() -> String {
    STRICT_POLICY
        .replace(r#""action": "use""#, r#""action": "ex:train""#)
        .replace(
            "  \"profile\": \"https://example.org/odrl/profile/research\",\n",
            "",
        )
}

const LEFT: &str = "policy[0].permission[0].constraint[0]#leftOperand";
const OPERATOR: &str = "policy[0].permission[0].constraint[0]#operator";

fn closed_wording() -> Rc<Wording> {
    Rc::new(Wording::new().with_profile(ProfileVocab::parse(CLOSED).unwrap()))
}

#[derive(Properties, PartialEq, Clone)]
struct StrictProps {
    json: AttrValue,
    log: Log,
    strict: bool,
    policy: StrictEdit,
    /// Events the page's buttons emit (`#inject-0`, ...), as a host that
    /// writes events itself would; `#apply-batch` applies them all as one.
    inject: Vec<EditEvent>,
    /// An action added to the vocabulary, which the profile does not allow.
    extra_action: Option<AttrValue>,
}

fn strict_props(json: &str, log: &Log, strict: bool, policy: StrictEdit) -> StrictProps {
    StrictProps {
        json: json.to_string().into(),
        log: log.clone(),
        strict,
        policy,
        inject: vec![],
        extra_action: None,
    }
}

/// A host as the demo's Edit tab builds one: the configuration follows the
/// strict switch, the issues come from the validator, and a refusal and the
/// problems panel are shown.
#[function_component(StrictHost)]
fn strict_host(p: &StrictProps) -> Html {
    let strict = use_state(|| p.strict);
    let config = use_memo((*strict, p.extra_action.clone()), |(strict, extra)| {
        let mut cfg = EditConfig::default().with_wording(closed_wording());
        if let Some(a) = extra {
            cfg.vocab
                .actions
                .push(prose_yew::Suggestion::new(a, a, None));
        }
        if *strict {
            cfg = cfg.with_strict(Options::strict());
        }
        cfg
    });
    let json = p.json.clone();
    let with = use_prose_editor_with(
        move || read_model(&json).expect("a readable fixture"),
        Rc::new(config.rules.clone()),
        config.validator.clone(),
        p.policy,
    );
    let report = use_validation(&with.editor.doc, config.validator.as_ref());
    let issues = Rc::new(report.issues());
    let refusal = with.refused.as_ref().and_then(|(ev, e)| e.issue(ev));
    let onedit = {
        let log = p.log.clone();
        let inner = with.editor.onedit.clone();
        Callback::from(move |ev: EditEvent| {
            log.borrow_mut().push(ev.clone());
            inner.emit(ev);
        })
    };
    let toggle = {
        let strict = strict.clone();
        Callback::from(move |_| strict.set(!*strict))
    };
    let undo = {
        let undo = with.editor.undo.clone();
        Callback::from(move |_| undo.emit(()))
    };
    let batch = {
        let apply = with.apply_all.clone();
        let evs = p.inject.clone();
        Callback::from(move |_| apply.emit(evs.clone()))
    };
    let onfix = with.apply_all.clone();
    html! {
        <>
            <button id="toggle" type="button" onclick={toggle}>{ "Toggle strict" }</button>
            <button id="undo" type="button" onclick={undo}>{ "Undo" }</button>
            <button id="apply-batch" type="button" onclick={batch}>{ "Batch" }</button>
            { for p.inject.iter().enumerate().map(|(i, ev)| {
                let onedit = onedit.clone();
                let ev = ev.clone();
                html! {
                    <button id={format!("inject-{i}")} type="button"
                            onclick={Callback::from(move |_| onedit.emit(ev.clone()))}>
                        { "Inject" }
                    </button>
                }
            }) }
            <p id="state"
               data-refused={with.refused.is_some().to_string()}
               data-can-undo={with.editor.can_undo.to_string()}
               data-verdict={format!("{:?}", report.verdict)} />
            <ProseProblems report={report.clone()} onfix={onfix} />
            <OdrlProseView
                doc={with.editor.doc.clone()}
                mode={EditMode::Edit}
                config={config.clone()}
                onedit={onedit}
                issues={issues}
                refusal={refusal}
            />
        </>
    }
}

async fn mount_strict(props: StrictProps) -> (Element, yew::AppHandle<StrictHost>) {
    let container = document().create_element("div").unwrap();
    document().body().unwrap().append_child(&container).unwrap();
    let handle =
        yew::Renderer::<StrictHost>::with_root_and_props(container.clone(), props).render();
    settle().await;
    (container, handle)
}

fn state_attr(c: &Element, name: &str) -> String {
    q(c, "#state").get_attribute(name).unwrap()
}

fn text_event(json: &str, path: &str, field: &str, value: &str) -> EditEvent {
    let slot: SlotPath = format!("{path}#{field}").parse().unwrap();
    EditEvent::SetText {
        expect: id_of(json, path),
        slot,
        value: value.into(),
    }
}

fn wrap_of(el: &Element) -> Element {
    el.closest(".prose-slot-wrap").unwrap().unwrap()
}

#[wasm_bindgen_test]
async fn non_match_enter_keeps_draft_open_invalid() {
    let log = Log::default();
    let (c, _h) = mount_strict(strict_props(STRICT_POLICY, &log, true, StrictEdit::Flag)).await;
    let el = slot(&c, ACTION);
    type_into(&el, "trian");
    key(&el, "Enter", false, false);
    settle().await;
    assert!(log.borrow().is_empty(), "nothing is committed");
    // The same element, with the draft in it: not remounted.
    let again = slot(&c, ACTION);
    assert_eq!(again.text_content().as_deref(), Some("trian"));
    assert_eq!(again.get_attribute("aria-invalid").as_deref(), Some("true"));
    let err = wrap_of(&again)
        .query_selector(".prose-issue-error")
        .unwrap()
        .expect("a message");
    assert_eq!(
        err.text_content().unwrap(),
        "\"trian\" is not an allowed action in Research Data Profile 0.1.0. Did you mean Train?"
    );
    let described = again.get_attribute("aria-describedby").unwrap();
    assert!(described.split(' ').any(|id| id == err.id()), "{described}");

    // With several entries left, the list stays open and Enter takes none.
    type_into(&slot(&c, ACTION), "t");
    settle().await;
    key(&slot(&c, ACTION), "Enter", false, false);
    settle().await;
    assert!(log.borrow().is_empty());
    assert!(
        c.query_selector("[role=listbox]").unwrap().is_some(),
        "{}",
        c.inner_html()
    );
    assert_eq!(
        slot(&c, ACTION).get_attribute("aria-invalid").as_deref(),
        Some("true")
    );
}

#[wasm_bindgen_test]
async fn typing_again_clears_the_message() {
    let log = Log::default();
    let (c, _h) = mount_strict(strict_props(STRICT_POLICY, &log, true, StrictEdit::Flag)).await;
    type_into(&slot(&c, ACTION), "trian");
    key(&slot(&c, ACTION), "Enter", false, false);
    settle().await;
    assert!(
        c.query_selector(".prose-slot-wrap .prose-issue-error")
            .unwrap()
            .is_some()
    );
    type_into(&slot(&c, ACTION), "tria");
    settle().await;
    assert!(
        c.query_selector(".prose-slot-wrap .prose-issue-error")
            .unwrap()
            .is_none()
    );
    assert_eq!(slot(&c, ACTION).get_attribute("aria-invalid"), None);
}

#[wasm_bindgen_test]
async fn blur_non_match_reverts_and_announces() {
    let log = Log::default();
    let (c, _h) = mount_strict(strict_props(STRICT_POLICY, &log, true, StrictEdit::Flag)).await;
    let el = slot(&c, ACTION);
    type_into(&el, "trian");
    fire_focusout(&el);
    settle().await;
    assert!(log.borrow().is_empty());
    let again = slot(&c, ACTION);
    assert_eq!(
        again.text_content().as_deref(),
        Some("use"),
        "back to the value"
    );
    assert_eq!(again.get_attribute("aria-invalid"), None);
    let live = wrap_of(&again)
        .query_selector("[role=status]")
        .unwrap()
        .expect("a live region");
    assert_eq!(live.get_attribute("aria-live").as_deref(), Some("polite"));
    assert_eq!(
        live.text_content().unwrap(),
        "Kept \"use\"; \"trian\" is not in the profile"
    );
}

#[wasm_bindgen_test]
async fn escape_reverts_a_rejected_draft() {
    let log = Log::default();
    let (c, _h) = mount_strict(strict_props(STRICT_POLICY, &log, true, StrictEdit::Flag)).await;
    type_into(&slot(&c, ACTION), "trian");
    key(&slot(&c, ACTION), "Enter", false, false);
    settle().await;
    key(&slot(&c, ACTION), "Escape", false, false);
    settle().await;
    assert!(log.borrow().is_empty());
    assert_eq!(slot(&c, ACTION).text_content().as_deref(), Some("use"));
    assert!(
        c.query_selector(".prose-slot-wrap .prose-issue-error")
            .unwrap()
            .is_none()
    );
}

#[wasm_bindgen_test]
async fn exact_label_match_commits() {
    for (typed, written) in [
        ("Train", "ex:train"),
        ("TRAIN", "ex:train"),
        ("ex:train", "ex:train"),
        ("Fine-tune", "ex:fineTune"),
        (
            "https://example.org/odrl/profile/research/fineTune",
            "ex:fineTune",
        ),
        ("odrl:read", "read"),
        // In the vocabulary, though not among the offered entries.
        ("extract", "extract"),
    ] {
        let log = Log::default();
        let (c, _h) = mount_strict(strict_props(STRICT_POLICY, &log, true, StrictEdit::Flag)).await;
        let el = slot(&c, ACTION);
        type_into(&el, typed);
        key(&el, "Enter", false, false);
        settle().await;
        assert_eq!(
            set_texts(&log),
            vec![(ACTION.into(), written.into())],
            "typed {typed:?}"
        );
        assert!(
            c.query_selector(".prose-slot-wrap .prose-issue-error")
                .unwrap()
                .is_none()
        );
    }
}

#[wasm_bindgen_test]
async fn enter_takes_the_only_entry_left() {
    let log = Log::default();
    let (c, _h) = mount_strict(strict_props(STRICT_POLICY, &log, true, StrictEdit::Flag)).await;
    let el = slot(&c, ACTION);
    type_into(&el, "fine");
    key(&el, "Enter", false, false);
    settle().await;
    assert_eq!(set_texts(&log), vec![(ACTION.into(), "ex:fineTune".into())]);
}

#[wasm_bindgen_test]
async fn clearing_a_closed_slot_commits_the_empty_text() {
    let log = Log::default();
    let (c, _h) = mount_strict(strict_props(STRICT_POLICY, &log, true, StrictEdit::Flag)).await;
    let el = slot(&c, ACTION);
    type_into(&el, "");
    fire_focusout(&el);
    settle().await;
    assert_eq!(set_texts(&log), vec![(ACTION.into(), String::new())]);
}

#[wasm_bindgen_test]
async fn empty_query_lists_all_allowed() {
    let log = Log::default();
    let mut props = strict_props(STRICT_POLICY, &log, true, StrictEdit::Flag);
    props.extra_action = Some("foo:bar".into());
    let (c, _h) = mount_strict(props).await;
    type_into(&slot(&c, ACTION), "");
    settle().await;
    let labels: Vec<String> = shown_options(&c).into_iter().map(|(l, _)| l).collect();
    // More than the 8 an open list shows, and none the profile refuses.
    assert!(labels.len() > 8, "{labels:?}");
    assert!(labels.iter().any(|l| l == "Train"), "{labels:?}");
    assert!(labels.iter().any(|l| l == "use"), "{labels:?}");
    assert!(!labels.iter().any(|l| l == "foo:bar"), "{labels:?}");

    // Outside strict mode the same list is open: capped, and unfiltered.
    let log = Log::default();
    let mut props = strict_props(STRICT_POLICY, &log, false, StrictEdit::Flag);
    props.extra_action = Some("foo:bar".into());
    let (c, _h) = mount_strict(props).await;
    type_into(&slot(&c, ACTION), "");
    settle().await;
    assert_eq!(shown_options(&c).len(), 8);
    assert!(slot(&c, ACTION).get_attribute("data-closed").is_none());
}

#[wasm_bindgen_test]
async fn an_open_slot_still_commits_anything() {
    let log = Log::default();
    let (c, _h) = mount_strict(strict_props(STRICT_POLICY, &log, false, StrictEdit::Flag)).await;
    let el = slot(&c, ACTION);
    type_into(&el, "foo:bar");
    key(&el, "Enter", false, false);
    settle().await;
    assert_eq!(set_texts(&log), vec![(ACTION.into(), "foo:bar".into())]);
}

#[wasm_bindgen_test]
async fn view_never_triggers_refusal() {
    let log = Log::default();
    let (c, _h) = mount_strict(strict_props(
        STRICT_POLICY,
        &log,
        true,
        StrictEdit::RefuseTerms,
    ))
    .await;
    // Text the profile does not take, on every closed slot: not sent.
    for (path, text) in [
        (ACTION, "foo:bar"),
        (LEFT, "foo:bar"),
        ("policy[0]#kind", "foo:bar"),
    ] {
        let el = slot(&c, path);
        type_into(&el, text);
        key(&el, "Enter", false, false);
        settle().await;
        fire_focusout(&slot(&c, path));
        settle().await;
    }
    // An operator the operand does not allow, picked by script.
    let select = slot(&c, OPERATOR).dyn_into::<HtmlSelectElement>().unwrap();
    select.set_value("gt");
    select
        .dispatch_event(&Event::new_with_event_init_dict("change", &bubbling()).unwrap())
        .unwrap();
    settle().await;
    assert!(log.borrow().is_empty(), "{:?}", log.borrow());
    assert_eq!(state_attr(&c, "data-refused"), "false");
    assert_eq!(
        slot(&c, OPERATOR)
            .dyn_into::<HtmlSelectElement>()
            .unwrap()
            .value(),
        "isAnyOf",
        "the select shows the document's operator"
    );
}

#[wasm_bindgen_test]
async fn flag_commits_and_marks() {
    let log = Log::default();
    let mut props = strict_props(STRICT_POLICY, &log, true, StrictEdit::Flag);
    props.inject = vec![text_event(
        STRICT_POLICY,
        "policy[0].permission[0].action[0]",
        "name",
        "foo:bar",
    )];
    let (c, _h) = mount_strict(props).await;
    click(&q(&c, "#inject-0"));
    settle().await;
    let el = slot(&c, ACTION);
    assert_eq!(el.text_content().as_deref(), Some("foo:bar"), "committed");
    assert_eq!(
        el.get_attribute("aria-invalid").as_deref(),
        Some("true"),
        "and marked"
    );
    assert_eq!(state_attr(&c, "data-refused"), "false");
    assert_eq!(state_attr(&c, "data-verdict"), "Violations");
    assert!(
        c.query_selector(".prose-problems .prose-problem-error")
            .unwrap()
            .is_some()
    );
}

#[wasm_bindgen_test]
async fn refuseterms_host_event_shown_at_slot() {
    let log = Log::default();
    let mut props = strict_props(STRICT_POLICY, &log, true, StrictEdit::RefuseTerms);
    props.inject = vec![
        text_event(
            STRICT_POLICY,
            "policy[0].permission[0].action[0]",
            "name",
            "foo:bar",
        ),
        text_event(
            STRICT_POLICY,
            "policy[0].permission[0].action[0]",
            "name",
            "read",
        ),
    ];
    let (c, _h) = mount_strict(props).await;
    click(&q(&c, "#inject-0"));
    settle().await;
    let el = slot(&c, ACTION);
    assert_eq!(
        el.text_content().as_deref(),
        Some("use"),
        "the document is unchanged"
    );
    assert_eq!(state_attr(&c, "data-refused"), "true");
    assert_eq!(state_attr(&c, "data-can-undo"), "false", "no history entry");
    assert_eq!(el.get_attribute("aria-invalid").as_deref(), Some("true"));
    // The slot is described by the refusal, which the view draws with its
    // other issues.
    let described = el.get_attribute("aria-describedby").expect("a description");
    let err = described
        .split(' ')
        .filter_map(|id| document().get_element_by_id(id))
        .find(|e| e.class_name().contains("prose-issue-error"))
        .expect("the refusal");
    assert!(
        err.text_content().unwrap().contains("foo:bar"),
        "{}",
        err.text_content().unwrap()
    );
    // The next edit that goes through clears it.
    click(&q(&c, "#inject-1"));
    settle().await;
    assert_eq!(slot(&c, ACTION).text_content().as_deref(), Some("read"));
    assert_eq!(state_attr(&c, "data-refused"), "false");
    assert_eq!(slot(&c, ACTION).get_attribute("aria-invalid"), None);
}

#[wasm_bindgen_test]
async fn fix_is_one_undo_step() {
    let json = undeclared_policy();
    let log = Log::default();
    let (c, _h) = mount_strict(strict_props(&json, &log, true, StrictEdit::Flag)).await;
    assert_eq!(state_attr(&c, "data-verdict"), "Violations");
    let fix = q(&c, ".prose-problems .prose-problem-fix");
    assert_eq!(fix.get_attribute("data-safe").as_deref(), Some("true"));
    click(&fix);
    settle().await;
    assert_eq!(
        state_attr(&c, "data-verdict"),
        "Conforms",
        "{}",
        c.inner_html()
    );
    assert!(
        c.query_selector(".prose-problems .prose-problem-fix")
            .unwrap()
            .is_none()
    );
    assert_eq!(state_attr(&c, "data-can-undo"), "true");
    click(&q(&c, "#undo"));
    settle().await;
    assert_eq!(state_attr(&c, "data-verdict"), "Violations");
    assert_eq!(
        state_attr(&c, "data-can-undo"),
        "false",
        "one step undid the fix"
    );
}

#[wasm_bindgen_test]
async fn several_events_are_one_undo_step_or_none() {
    let mut props = strict_props(
        STRICT_POLICY,
        &Log::default(),
        true,
        StrictEdit::RefuseTerms,
    );
    let action = "policy[0].permission[0].action[0]";
    props.inject = vec![
        text_event(STRICT_POLICY, action, "name", "read"),
        text_event(STRICT_POLICY, "policy[0]", "uid", "urn:policy:renamed"),
    ];
    let (c, _h) = mount_strict(props.clone()).await;
    click(&q(&c, "#apply-batch"));
    settle().await;
    assert_eq!(slot(&c, ACTION).text_content().as_deref(), Some("read"));
    assert_eq!(
        slot(&c, "policy[0]#uid").text_content().as_deref(),
        Some("urn:policy:renamed")
    );
    click(&q(&c, "#undo"));
    settle().await;
    assert_eq!(slot(&c, ACTION).text_content().as_deref(), Some("use"));
    assert_eq!(
        slot(&c, "policy[0]#uid").text_content().as_deref(),
        Some("urn:policy:s")
    );
    assert_eq!(state_attr(&c, "data-can-undo"), "false");

    // If the second is refused, the first is not applied either.
    props.inject = vec![
        text_event(STRICT_POLICY, action, "name", "read"),
        text_event(STRICT_POLICY, action, "name", "foo:bar"),
    ];
    let (c, _h) = mount_strict(props).await;
    click(&q(&c, "#apply-batch"));
    settle().await;
    assert_eq!(slot(&c, ACTION).text_content().as_deref(), Some("use"));
    assert_eq!(state_attr(&c, "data-refused"), "true");
    assert_eq!(state_attr(&c, "data-can-undo"), "false");
}

#[wasm_bindgen_test]
async fn undo_never_refused() {
    let json = violating_policy();
    let mut props = strict_props(&json, &Log::default(), true, StrictEdit::RefuseTerms);
    props.inject = vec![text_event(
        &json,
        "policy[0].permission[0].action[0]",
        "name",
        "read",
    )];
    let (c, _h) = mount_strict(props).await;
    // An already violating document loads and can be edited.
    assert_eq!(slot(&c, ACTION).text_content().as_deref(), Some("foo:bar"));
    click(&q(&c, "#inject-0"));
    settle().await;
    assert_eq!(slot(&c, ACTION).text_content().as_deref(), Some("read"));
    assert_eq!(state_attr(&c, "data-verdict"), "Conforms");
    // Undo restores the document the profile does not allow.
    click(&q(&c, "#undo"));
    settle().await;
    assert_eq!(slot(&c, ACTION).text_content().as_deref(), Some("foo:bar"));
    assert_eq!(state_attr(&c, "data-refused"), "false");
    assert_eq!(state_attr(&c, "data-verdict"), "Violations");
}

#[wasm_bindgen_test]
async fn toggle_strict_keeps_doc_and_history() {
    let log = Log::default();
    let (c, _h) = mount_strict(strict_props(STRICT_POLICY, &log, true, StrictEdit::Flag)).await;
    let uid = slot(&c, "policy[0]#uid");
    type_into(&uid, "urn:policy:edited");
    fire_focusout(&uid);
    settle().await;
    assert_eq!(state_attr(&c, "data-can-undo"), "true");
    assert!(slot(&c, ACTION).get_attribute("data-closed").is_some());
    click(&q(&c, "#toggle"));
    settle().await;
    assert!(slot(&c, ACTION).get_attribute("data-closed").is_none());
    assert_eq!(
        slot(&c, "policy[0]#uid").text_content().as_deref(),
        Some("urn:policy:edited")
    );
    assert_eq!(
        state_attr(&c, "data-can-undo"),
        "true",
        "the history is kept"
    );
    click(&q(&c, "#toggle"));
    settle().await;
    assert!(slot(&c, ACTION).get_attribute("data-closed").is_some());
    assert_eq!(
        slot(&c, "policy[0]#uid").text_content().as_deref(),
        Some("urn:policy:edited")
    );
    click(&q(&c, "#undo"));
    settle().await;
    assert_eq!(
        slot(&c, "policy[0]#uid").text_content().as_deref(),
        Some("urn:policy:s")
    );
}

#[wasm_bindgen_test]
async fn a_closed_value_slot_takes_only_the_values_the_profile_lists() {
    let value = "policy[0].permission[0].constraint[0]#rightOperand[0]";
    let log = Log::default();
    let (c, _h) = mount_strict(strict_props(STRICT_POLICY, &log, true, StrictEdit::Flag)).await;
    assert_eq!(
        slot(&c, value).get_attribute("data-closed").as_deref(),
        Some("true")
    );
    // The profile lists ex:academic (with its narrower values) and
    // ex:commercialResearch, and nothing else is taken.
    let el = slot(&c, value);
    type_into(&el, "ex:nothing");
    key(&el, "Enter", false, false);
    settle().await;
    assert!(log.borrow().is_empty(), "{:?}", log.borrow());
    assert_eq!(
        slot(&c, value).get_attribute("aria-invalid").as_deref(),
        Some("true")
    );
    let el = slot(&c, value);
    type_into(&el, "ex:commercialResearch");
    key(&el, "Enter", false, false);
    settle().await;
    assert_eq!(
        set_texts(&log),
        vec![(value.into(), "ex:commercialResearch".into())]
    );
}

#[wasm_bindgen_test]
async fn a_closed_value_typed_over_a_string_is_written_as_an_iri() {
    let value = "policy[0].permission[0].constraint[0]#rightOperand[0]";
    let json = STRICT_POLICY.replace(r#"[{"@id": "ex:academic"}]"#, r#"["ex:academic"]"#);
    let log = Log::default();
    let (c, _h) = mount_strict(strict_props(&json, &log, true, StrictEdit::Flag)).await;
    let el = slot(&c, value);
    type_into(&el, "ex:commercialResearch");
    key(&el, "Enter", false, false);
    settle().await;
    assert_eq!(
        set_iris(&log),
        vec![(value.into(), "ex:commercialResearch".into())]
    );
    assert!(set_texts(&log).is_empty(), "{:?}", log.borrow());
}

#[wasm_bindgen_test]
async fn the_list_of_a_closed_slot_closes_once_its_text_is_taken() {
    let log = Log::default();
    let (c, _h) = mount_strict(strict_props(STRICT_POLICY, &log, true, StrictEdit::Flag)).await;
    let el = slot(&c, ACTION);
    type_into(&el, "t");
    settle().await;
    assert!(c.query_selector("[role=listbox]").unwrap().is_some());
    type_into(&slot(&c, ACTION), "train");
    key(&slot(&c, ACTION), "Enter", false, false);
    settle().await;
    assert_eq!(set_texts(&log), vec![(ACTION.into(), "ex:train".into())]);
    assert!(
        c.query_selector("[role=listbox]").unwrap().is_none(),
        "{}",
        c.inner_html()
    );
}

#[wasm_bindgen_test]
async fn giving_up_a_rejected_draft_on_blur_takes_the_message_away() {
    let log = Log::default();
    let (c, _h) = mount_strict(strict_props(STRICT_POLICY, &log, true, StrictEdit::Flag)).await;
    type_into(&slot(&c, ACTION), "trian");
    key(&slot(&c, ACTION), "Enter", false, false);
    settle().await;
    assert!(
        c.query_selector(".prose-slot-wrap .prose-issue-error")
            .unwrap()
            .is_some()
    );
    fire_focusout(&slot(&c, ACTION));
    settle().await;
    assert!(
        c.query_selector(".prose-slot-wrap .prose-issue-error")
            .unwrap()
            .is_none()
    );
    assert_eq!(slot(&c, ACTION).get_attribute("aria-invalid"), None);
    assert_eq!(slot(&c, ACTION).text_content().as_deref(), Some("use"));
}

/// UX-6: a refusal belongs to the gate that made it. Turning strict off
/// drops it, with the mark on the slot.
#[wasm_bindgen_test]
async fn a_refusal_does_not_outlive_a_change_of_strictness() {
    let log = Log::default();
    let mut props = strict_props(STRICT_POLICY, &log, true, StrictEdit::RefuseTerms);
    props.inject = vec![text_event(
        STRICT_POLICY,
        "policy[0].permission[0].action[0]",
        "name",
        "foo:bar",
    )];
    let (c, _h) = mount_strict(props).await;
    (q(&c, "#inject-0").dyn_into::<HtmlElement>().unwrap()).click();
    settle().await;
    assert_eq!(state_attr(&c, "data-refused"), "true");
    assert_eq!(
        slot(&c, ACTION).get_attribute("aria-invalid").as_deref(),
        Some("true")
    );
    (q(&c, "#toggle").dyn_into::<HtmlElement>().unwrap()).click();
    settle().await;
    assert_eq!(state_attr(&c, "data-refused"), "false");
    assert_eq!(slot(&c, ACTION).get_attribute("aria-invalid"), None);
    // And a refusal made now is kept as before (the effect does not eat it
    // while nothing about the gate changes).
    (q(&c, "#toggle").dyn_into::<HtmlElement>().unwrap()).click();
    settle().await;
    (q(&c, "#inject-0").dyn_into::<HtmlElement>().unwrap()).click();
    settle().await;
    assert_eq!(state_attr(&c, "data-refused"), "true");
}

// --- Hosts that set the editor's own switches (suggestions, validator options)

#[derive(Properties, PartialEq, Clone)]
struct TuneProps {
    json: AttrValue,
    log: Log,
    options: Option<Options>,
    policy: StrictEdit,
    suggestions: bool,
}

/// A strict host without the problems panel, whose suggestions switch and
/// validator options the test sets.
#[function_component(TuneHost)]
fn tune_host(p: &TuneProps) -> Html {
    let config = use_memo((p.options.clone(), p.suggestions), |(o, s)| {
        let mut cfg = EditConfig::default().with_wording(closed_wording());
        cfg.suggestions = *s;
        if let Some(o) = o {
            cfg = cfg.with_strict(o.clone());
        }
        cfg
    });
    let json = p.json.clone();
    let with = use_prose_editor_with(
        move || read_model(&json).expect("a readable fixture"),
        Rc::new(config.rules.clone()),
        config.validator.clone(),
        p.policy,
    );
    let report = use_validation(&with.editor.doc, config.validator.as_ref());
    let issues = Rc::new(report.issues());
    let refusal = with.refused.as_ref().and_then(|(ev, e)| e.issue(ev));
    let onedit = {
        let log = p.log.clone();
        let inner = with.editor.onedit.clone();
        Callback::from(move |ev: EditEvent| {
            log.borrow_mut().push(ev.clone());
            inner.emit(ev);
        })
    };
    html! {
        <>
            <p id="state"
               data-refused={with.refused.is_some().to_string()}
               data-verdict={format!("{:?}", report.verdict)} />
            <OdrlProseView
                doc={with.editor.doc.clone()}
                mode={EditMode::Edit}
                config={config.clone()}
                onedit={onedit}
                issues={issues}
                refusal={refusal}
            />
        </>
    }
}

async fn mount_tune(p: TuneProps) -> Element {
    let container = document().create_element("div").unwrap();
    document().body().unwrap().append_child(&container).unwrap();
    let handle = yew::Renderer::<TuneHost>::with_root_and_props(container.clone(), p).render();
    std::mem::forget(handle);
    settle().await;
    container
}

fn tune(json: &str, log: &Log, options: Option<Options>, suggestions: bool) -> TuneProps {
    TuneProps {
        json: json.to_string().into(),
        log: log.clone(),
        options,
        policy: StrictEdit::RefuseTerms,
        suggestions,
    }
}

/// UX-1 (I7): the stock view never triggers a refusal, with or without the
/// suggestions, and the typed text stays to be corrected.
#[wasm_bindgen_test]
async fn ux1_without_suggestions_a_refusing_host_keeps_the_draft_and_is_not_refused() {
    let log = Log::default();
    let c = mount_tune(tune(STRICT_POLICY, &log, Some(Options::strict()), false)).await;
    let el = slot(&c, ACTION);
    type_into(&el, "foo:bar");
    key(&el, "Enter", false, false);
    settle().await;
    assert!(set_texts(&log).is_empty(), "sent {:?}", set_texts(&log));
    assert_eq!(state_attr(&c, "data-refused"), "false");
    let el = slot(&c, ACTION);
    assert_eq!(el.text_content().as_deref(), Some("foo:bar"));
    assert_eq!(el.get_attribute("aria-invalid").as_deref(), Some("true"));
    assert!(c.query_selector(".prose-slot-message").unwrap().is_some());
    assert!(c.query_selector("[role=listbox]").unwrap().is_none());
    // A term the profile has still commits.
    type_into(&el, "ex:train");
    key(&el, "Enter", false, false);
    settle().await;
    assert_eq!(
        set_texts(&log),
        [(ACTION.to_string(), "ex:train".to_string())]
    );
    assert_eq!(state_attr(&c, "data-refused"), "false");
}

/// UX-1: past the render's check budget a slot is rendered open, but a
/// commit asks the validator once and does not send what it would refuse.
#[wasm_bindgen_test]
async fn ux1_past_the_check_budget_a_refusing_host_is_still_not_refused() {
    // Each policy's kind and action slots take two checks to find out
    // whether they are closed and whether the text in them is outside, so
    // a few hundred policies use the budget up.
    const POLICIES: usize = 700;
    let one = |i: usize| {
        format!(
            r#"{{"@type": "Set", "uid": "urn:policy:{i}",
                "profile": "https://example.org/odrl/profile/research",
                "permission": [{{"target": "urn:asset:1", "action": "use"}}]}}"#
        )
    };
    let all = (0..POLICIES).map(one).collect::<Vec<_>>().join(",");
    let json = format!(
        r#"{{"@context": ["http://www.w3.org/ns/odrl.jsonld",
                          {{"ex": "https://example.org/odrl/profile/research/"}}],
            "@graph": [{all}]}}"#
    );
    let log = Log::default();
    let c = mount_tune(tune(&json, &log, Some(Options::strict()), true)).await;
    // Past the budget some action slots are rendered open (which ones
    // depends on the order the components render in).
    let open_slot = c
        .query_selector("[data-slot$=\"action[0]#name\"]:not([data-closed])")
        .unwrap()
        .expect("a slot past the budget is rendered open")
        .dyn_into::<HtmlElement>()
        .unwrap();
    let last = open_slot.get_attribute("data-slot").unwrap();
    let el = slot(&c, &last);
    type_into(&el, "foo:bar");
    key(&el, "Enter", false, false);
    settle().await;
    assert!(set_texts(&log).is_empty(), "sent {:?}", set_texts(&log));
    assert_eq!(state_attr(&c, "data-refused"), "false");
    assert_eq!(slot(&c, &last).text_content().as_deref(), Some("foo:bar"));
}

/// UX-2: taking the slot's current value from the list after a rejected
/// draft ends the rejection: no event is needed, but the message and
/// aria-invalid must go.
#[wasm_bindgen_test]
async fn ux2_picking_the_current_value_after_a_rejection_clears_the_message() {
    let log = Log::default();
    let (c, _h) = mount_strict(strict_props(STRICT_POLICY, &log, true, StrictEdit::Flag)).await;
    type_into(&slot(&c, ACTION), "us");
    key(&slot(&c, ACTION), "Enter", false, false);
    settle().await;
    assert!(c.query_selector(".prose-slot-message").unwrap().is_some());
    let shown = shown_options(&c);
    let idx = shown
        .iter()
        .position(|(l, _)| l == "use")
        .expect("the current value is offered");
    // The highlight starts on the first entry.
    for _ in 0..idx {
        key(&slot(&c, ACTION), "ArrowDown", false, false);
        settle().await;
    }
    key(&slot(&c, ACTION), "Enter", false, false);
    settle().await;
    let el = slot(&c, ACTION);
    assert_eq!(el.text_content().as_deref(), Some("use"));
    assert!(set_texts(&log).is_empty(), "the value did not change");
    assert!(c.query_selector(".prose-slot-message").unwrap().is_none());
    assert_eq!(el.get_attribute("aria-invalid"), None);
    assert_eq!(el.get_attribute("aria-describedby"), None);
}
