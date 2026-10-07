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
