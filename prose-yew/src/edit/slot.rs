//! `SlotEditor`: one piece of free text, edited in place with
//! `contenteditable`.
//!
//! Yew never reads the DOM text back, so the span is remounted (through its
//! `key`) after every commit, revert and refused edit; that is the only safe
//! way to bring the DOM and the virtual tree back in step. Commit happens
//! on blur or Enter, Escape reverts, and Enter never inserts a line break.
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use yew::prelude::*;

use prose_core::edit::{EditEvent, IssueTarget, Severity, Slot, SlotKind};

use super::config::{Suggestion, cls, merge_styles};
use super::suggest::filter_suggestions;
use super::{Env, EnvRef, FocusKind, SlotFocus};

/// The text to store: no CR or LF, U+00A0 as a plain space, no other
/// control characters. Never trimmed: the engine compares text exactly.
pub(crate) fn clean_text(s: &str) -> String {
    s.chars()
        .filter_map(|c| match c {
            '\r' | '\n' => None,
            '\u{a0}' => Some(' '),
            c if (c as u32) < 0x20 => None,
            c => Some(c),
        })
        .collect()
}

/// The text to store after an edit: only the part that changed is cleaned.
/// A tab or a no-break space that was already in `raw` and that the user did
/// not touch stays as it is, because the engine compares text exactly and a
/// one-character fix must not rewrite the rest of the value.
pub(crate) fn clean_edit(text: &str, raw: &str) -> String {
    let t: Vec<char> = text.chars().collect();
    let r: Vec<char> = raw.chars().collect();
    let prefix = t.iter().zip(&r).take_while(|(a, b)| a == b).count();
    let limit = t.len().min(r.len()) - prefix;
    let suffix = t
        .iter()
        .rev()
        .zip(r.iter().rev())
        .take(limit)
        .take_while(|(a, b)| a == b)
        .count();
    let middle: String = t[prefix..t.len() - suffix].iter().collect();
    let mut out: String = t[..prefix].iter().collect();
    out.push_str(&clean_text(&middle));
    out.extend(&t[t.len() - suffix..]);
    out
}

/// Pasted text: line breaks and tabs become spaces.
pub(crate) fn clean_paste(s: &str) -> String {
    s.replace("\r\n", " ").replace(['\n', '\r', '\t'], " ")
}

fn has_odd_whitespace(raw: &str) -> bool {
    raw.starts_with(char::is_whitespace)
        || raw.ends_with(char::is_whitespace)
        || raw.contains('\u{a0}')
}

/// `plaintext-only` where the browser supports it (detected once), else
/// `true`. On the host (server rendering) always `plaintext-only`.
fn editable_value() -> &'static str {
    #[cfg(target_arch = "wasm32")]
    {
        use std::cell::Cell;
        thread_local! {
            static CACHE: Cell<Option<&'static str>> = const { Cell::new(None) };
        }
        if let Some(v) = CACHE.with(|c| c.get()) {
            return v;
        }
        let supported = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.create_element("span").ok())
            .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
            .map(|e| {
                let _ = e.set_attribute("contenteditable", "plaintext-only");
                e.content_editable() == "plaintext-only"
            })
            .unwrap_or(false);
        let v = if supported { "plaintext-only" } else { "true" };
        CACHE.with(|c| c.set(Some(v)));
        v
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        "plaintext-only"
    }
}

/// Focus an element and put the caret at the end of its content.
pub(crate) fn focus_end(el: &web_sys::Element) {
    if let Some(h) = el.dyn_ref::<web_sys::HtmlElement>() {
        let _ = h.focus();
    }
    if el.get_attribute("contenteditable").is_some()
        && let Some(sel) = web_sys::window().and_then(|w| w.get_selection().ok().flatten())
    {
        let _ = sel.select_all_children(el);
        let _ = sel.collapse_to_end();
    }
}

#[derive(Clone, PartialEq)]
struct Rev(u32);

impl Reducible for Rev {
    type Action = ();
    fn reduce(self: Rc<Self>, _: ()) -> Rc<Self> {
        Rc::new(Rev(self.0.wrapping_add(1)))
    }
}

pub(crate) fn issue_key(t: &IssueTarget) -> String {
    match t {
        IssueTarget::Doc => "doc".to_string(),
        IssueTarget::Node(p) => format!("node:{p}"),
        IssueTarget::Slot(s) => format!("slot:{s}"),
        IssueTarget::List(l) => format!("list:{l}"),
        // A target this version does not know still gets a distinct key.
        other => format!("other:{other:?}"),
    }
}

#[derive(Properties, PartialEq, Clone)]
pub(crate) struct SlotEditorProps {
    pub slot: Slot,
    pub env: EnvRef,
    #[prop_or_default]
    pub severity: Option<Severity>,
    /// Space-separated ids of the issue elements that describe this slot.
    #[prop_or_default]
    pub described_by: Option<AttrValue>,
}

fn options_for(env: &Env, slot: &Slot) -> Vec<Suggestion> {
    let v = &env.cfg.vocab;
    match slot.kind {
        SlotKind::Action => v.actions.clone(),
        SlotKind::LeftOperand => v.left_operands.clone(),
        SlotKind::PolicyKind => v.policy_kinds.clone(),
        SlotKind::Party => v.parties.clone(),
        SlotKind::Asset => v.assets.clone(),
        SlotKind::PolicyRef => env
            .policy_uids
            .iter()
            .filter(|(i, _)| *i != slot.path.node.policy)
            .map(|(_, uid)| Suggestion {
                value: uid.clone(),
                label: uid.clone(),
                hint: None,
            })
            .collect(),
        _ => vec![],
    }
}

fn placeholder(env: &Env, kind: SlotKind) -> AttrValue {
    let p = &env.cfg.labels.placeholders;
    match kind {
        SlotKind::PolicyKind => &p.kind,
        SlotKind::Uid => &p.uid,
        SlotKind::Party => &p.party,
        SlotKind::Asset => &p.asset,
        SlotKind::Action => &p.action,
        SlotKind::LeftOperand => &p.left_operand,
        SlotKind::Literal => &p.literal,
        SlotKind::Unit => &p.unit,
        SlotKind::OperandReference => &p.operand_reference,
        SlotKind::PartOf => &p.part_of,
        SlotKind::Profile => &p.profile,
        SlotKind::PolicyRef => &p.policy_ref,
        SlotKind::Reference => &p.reference,
        // A kind this version has no placeholder for shows none.
        _ => return AttrValue::from(""),
    }
    .clone()
}

#[function_component(SlotEditor)]
pub(crate) fn slot_editor(props: &SlotEditorProps) -> Html {
    let SlotEditorProps {
        slot,
        env,
        severity,
        described_by,
    } = props;
    let env: Rc<Env> = env.0.clone();
    let cfg = env.cfg.clone();
    let node_ref = use_node_ref();
    let rev = use_reducer(|| Rev(0));
    let dirty = use_mut_ref(|| false);
    // Whether the user moved the highlight in the suggestion list. Enter
    // takes a suggestion only then; otherwise it commits the typed text, the
    // same as leaving the slot does.
    let navigated = use_mut_ref(|| false);
    let reverting = use_mut_ref(|| false);
    let refocus = use_mut_ref(|| false);
    // The raw text of the previous render, to notice the host changing the
    // value under a slot that is being typed in; the text typed so far is
    // kept in `restore` across the remount that follows, and `epoch` is the
    // part of the key that remounts it.
    let last_raw = use_mut_ref(|| slot.raw.clone());
    let restore = use_mut_ref(|| None::<String>);
    let epoch = use_mut_ref(|| 0u32);
    if *dirty.borrow() && *last_raw.borrow() != slot.raw {
        // Yew has not touched the DOM yet: what is in the span is what the
        // user typed. Without a remount Yew would overwrite that text node
        // with the host's new value, and the edit would be lost silently.
        let typed = node_ref
            .cast::<web_sys::Element>()
            .and_then(|e| e.text_content());
        if let Some(t) = typed {
            *restore.borrow_mut() = Some(t);
            *epoch.borrow_mut() += 1;
        }
    }
    *last_raw.borrow_mut() = slot.raw.clone();
    let query = use_state(String::new);
    let open = use_state(|| false);
    let active = use_state(|| 0usize);

    let use_suggestions = cfg.suggestions;
    let options = if use_suggestions {
        options_for(&env, slot)
    } else {
        vec![]
    };
    let shown: Vec<Suggestion> = if *open {
        filter_suggestions(&options, &query, cfg.max_suggestions)
            .into_iter()
            .cloned()
            .collect()
    } else {
        vec![]
    };
    let list_open = *open && !shown.is_empty();
    let list_id = format!("prose-sugg-{}", slot.path);
    let active_idx = if shown.is_empty() {
        0
    } else {
        (*active).min(shown.len() - 1)
    };

    let commit: Callback<()> = {
        let node_ref = node_ref.clone();
        let dirty = dirty.clone();
        let rev = rev.dispatcher();
        let onedit = env.onedit.clone();
        let path = slot.path.clone();
        let owner = slot.owner;
        let raw = slot.raw.clone();
        Callback::from(move |_| {
            if !*dirty.borrow() {
                return;
            }
            let text = node_ref
                .cast::<web_sys::Element>()
                .and_then(|e| e.text_content())
                .unwrap_or_default();
            let value = clean_edit(&text, &raw);
            *dirty.borrow_mut() = false;
            if value != raw {
                onedit.emit(EditEvent::SetText {
                    slot: path.clone(),
                    expect: owner,
                    value,
                });
            }
            rev.dispatch(());
        })
    };

    let accept: Callback<AttrValue> = {
        let node_ref = node_ref.clone();
        let dirty = dirty.clone();
        let refocus = refocus.clone();
        let commit = commit.clone();
        let open = open.clone();
        Callback::from(move |value: AttrValue| {
            if let Some(el) = node_ref.cast::<web_sys::Element>() {
                el.set_text_content(Some(&value));
            }
            *dirty.borrow_mut() = true;
            *refocus.borrow_mut() = true;
            open.set(false);
            commit.emit(());
        })
    };

    let oninput = {
        let dirty = dirty.clone();
        let navigated = navigated.clone();
        let node_ref = node_ref.clone();
        let query = query.clone();
        let open = open.clone();
        let active = active.clone();
        Callback::from(move |e: InputEvent| {
            *dirty.borrow_mut() = true;
            *navigated.borrow_mut() = false;
            if use_suggestions && !e.is_composing() {
                let text = node_ref
                    .cast::<web_sys::Element>()
                    .and_then(|e| e.text_content())
                    .unwrap_or_default();
                query.set(clean_text(&text));
                active.set(0);
                open.set(true);
            }
        })
    };

    let onkeydown = {
        let dirty = dirty.clone();
        let reverting = reverting.clone();
        let refocus = refocus.clone();
        let rev = rev.dispatcher();
        let commit = commit.clone();
        let accept = accept.clone();
        let open = open.clone();
        let active = active.clone();
        let query = query.clone();
        let node_ref = node_ref.clone();
        let shown = shown.clone();
        let navigated = navigated.clone();
        Callback::from(move |e: KeyboardEvent| {
            // keyCode 229: Safari sends the Enter that confirms an IME
            // candidate after `compositionend`, with isComposing already false.
            if e.is_composing() || e.key_code() == 229 {
                return;
            }
            let key = e.key();
            match key.as_str() {
                "Enter" => {
                    e.prevent_default();
                    if list_open
                        && *navigated.borrow()
                        && let Some(s) = shown.get(active_idx)
                    {
                        accept.emit(s.value.clone());
                        return;
                    }
                    if *dirty.borrow() {
                        *refocus.borrow_mut() = true;
                    }
                    open.set(false);
                    commit.emit(());
                }
                "Escape" => {
                    e.prevent_default();
                    if list_open {
                        open.set(false);
                    } else {
                        *reverting.borrow_mut() = true;
                        *dirty.borrow_mut() = false;
                        *refocus.borrow_mut() = true;
                        rev.dispatch(());
                    }
                }
                "ArrowDown" | "ArrowUp" if list_open => {
                    e.prevent_default();
                    let n = shown.len();
                    let next = if key == "ArrowDown" {
                        (active_idx + 1) % n
                    } else {
                        (active_idx + n - 1) % n
                    };
                    active.set(next);
                    *navigated.borrow_mut() = true;
                }
                "ArrowDown" if e.alt_key() && use_suggestions => {
                    e.prevent_default();
                    let text = node_ref
                        .cast::<web_sys::Element>()
                        .and_then(|e| e.text_content())
                        .unwrap_or_default();
                    query.set(clean_text(&text));
                    active.set(0);
                    *navigated.borrow_mut() = false;
                    open.set(true);
                }
                _ => {}
            }
        })
    };

    let onfocusin = {
        let on_focus = env.on_focus_slot.clone();
        let focus = SlotFocus {
            slot: slot.path.clone(),
            kind: FocusKind::Slot(slot.kind),
            raw: slot.raw.clone(),
        };
        Callback::from(move |_: FocusEvent| on_focus.emit(Some(focus.clone())))
    };

    let onfocusout = {
        let on_focus = env.on_focus_slot.clone();
        let reverting = reverting.clone();
        let commit = commit.clone();
        let open = open.clone();
        Callback::from(move |_: FocusEvent| {
            if *reverting.borrow() {
                *reverting.borrow_mut() = false;
                return;
            }
            commit.emit(());
            open.set(false);
            on_focus.emit(None);
        })
    };

    let onpaste = {
        let dirty = dirty.clone();
        let navigated = navigated.clone();
        let query = query.clone();
        let open = open.clone();
        let active = active.clone();
        let node_ref = node_ref.clone();
        Callback::from(move |e: Event| {
            e.prevent_default();
            let Some(text) = e
                .dyn_ref::<web_sys::ClipboardEvent>()
                .and_then(|c| c.clipboard_data())
                .and_then(|d| d.get_data("text/plain").ok())
            else {
                return;
            };
            let text = clean_paste(&text);
            let Some(el) = node_ref.cast::<web_sys::Element>() else {
                return;
            };
            if insert_at_caret(&el, &text).is_some() {
                *RefCell::borrow_mut(&dirty) = true;
                // Inserting text through the DOM fires no `input` event: refresh
                // the suggestions from what the slot now holds, or the next
                // Enter would act on a list for the old text.
                if use_suggestions {
                    query.set(clean_text(&el.text_content().unwrap_or_default()));
                    active.set(0);
                    *RefCell::borrow_mut(&navigated) = false;
                    open.set(true);
                }
            }
        })
    };
    let prevent = Callback::from(|e: DragEvent| e.prevent_default());

    {
        let refocus = refocus.clone();
        let reverting = reverting.clone();
        let node_ref = node_ref.clone();
        let restore = restore.clone();
        use_effect(move || {
            let mut want = std::mem::take(&mut *refocus.borrow_mut());
            *reverting.borrow_mut() = false;
            let typed = restore.borrow_mut().take();
            if let Some(el) = node_ref.cast::<web_sys::Element>() {
                if let Some(t) = typed {
                    // The slot was remounted under the user's typing: put the
                    // typed text back; it is committed on blur as usual.
                    el.set_text_content(Some(&t));
                    want = true;
                }
                if want {
                    focus_end(&el);
                }
            }
            || ()
        });
    }

    let mut class = cls!(cfg, slot, "prose-slot");
    // Inline styles are layered in the same order as the classes, so a
    // host's state style comes after (and beats) its plain `slot` style.
    let mut styles = vec![&cfg.styles.slot];
    if slot.raw.is_empty() {
        class.extend(cls!(cfg, slot_empty, "prose-slot-empty"));
        styles.push(&cfg.styles.slot_empty);
    }
    match severity {
        Some(Severity::Error) => {
            class.extend(cls!(cfg, slot_invalid, "prose-slot-invalid"));
            styles.push(&cfg.styles.slot_invalid);
        }
        Some(Severity::Warning) => {
            class.extend(cls!(cfg, slot_warning, "prose-slot-warning"));
            styles.push(&cfg.styles.slot_warning);
        }
        _ => {}
    }
    if has_odd_whitespace(&slot.raw) {
        class.extend(cls!(cfg, slot_whitespace, "prose-slot-whitespace"));
        styles.push(&cfg.styles.slot_whitespace);
    }
    let style = merge_styles(&styles);
    let key = format!(
        "{}:{:?}:{}:{}",
        slot.owner.0,
        slot.path.field,
        rev.0,
        *epoch.borrow()
    );
    let label = cfg.labels.slot_name(&slot.path);
    let invalid = matches!(severity, Some(Severity::Error)).then_some("true");
    // aria-expanded and the popup relationship belong to a combobox, not a
    // plain textbox.
    let role = if use_suggestions {
        "combobox"
    } else {
        "textbox"
    };
    let (expanded, controls, activedesc) = if use_suggestions {
        (
            Some(if list_open { "true" } else { "false" }),
            Some(list_id.clone()),
            list_open.then(|| format!("{list_id}-{active_idx}")),
        )
    } else {
        (None, None, None)
    };

    html! {
        <span class="prose-slot-wrap">
            <span class="prose-slot-edit">
                <span
                    key={key}
                    ref={node_ref}
                    class={class}
                    style={style}
                    role={role}
                    contenteditable={editable_value()}
                    aria-multiline={(!use_suggestions).then_some("false")}
                    aria-placeholder={placeholder(&env, slot.kind)}
                    aria-haspopup={use_suggestions.then_some("listbox")}
                    aria-label={label}
                    aria-invalid={invalid}
                    aria-describedby={described_by.clone()}
                    aria-autocomplete={use_suggestions.then_some("list")}
                    aria-expanded={expanded}
                    aria-controls={controls}
                    aria-activedescendant={activedesc}
                    data-slot={slot.path.to_string()}
                    data-placeholder={placeholder(&env, slot.kind)}
                    title={slot.display.clone()}
                    spellcheck="false"
                    autocapitalize="off"
                    autocorrect="off"
                    oninput={oninput}
                    onkeydown={onkeydown}
                    onfocusin={onfocusin}
                    onfocusout={onfocusout}
                    onpaste={onpaste}
                    ondrop={prevent.clone()}
                    ondragover={prevent}
                >
                    { if slot.raw.is_empty() { Html::default() } else { html! { { slot.raw.clone() } } } }
                </span>
            </span>
            if list_open {
                <ul
                    id={list_id.clone()}
                    role="listbox"
                    class={cls!(cfg, suggestions, "prose-suggestions")}
                    style={cfg.styles.suggestions.clone()}
                >
                    { for shown.iter().enumerate().map(|(i, s)| {
                        let accept = accept.clone();
                        let value = s.value.clone();
                        let onmousedown = Callback::from(move |e: MouseEvent| {
                            e.prevent_default();
                            accept.emit(value.clone());
                        });
                        // The pointer highlights an option (the stylesheet does
                        // it for `:hover`); the active descendant follows, so
                        // what is shown and what is announced agree. Enter
                        // still commits the typed text unless an arrow key was
                        // used.
                        let onmousemove = {
                            let active = active.clone();
                            Callback::from(move |_: MouseEvent| {
                                if *active != i {
                                    active.set(i);
                                }
                            })
                        };
                        let mut class = cls!(cfg, suggestion, "prose-suggestion");
                        let mut styles = vec![&cfg.styles.suggestion];
                        if i == active_idx {
                            class.extend(cls!(cfg, suggestion_active, "prose-suggestion-active"));
                            styles.push(&cfg.styles.suggestion_active);
                        }
                        let style = merge_styles(&styles);
                        html! {
                            <li
                                id={format!("{list_id}-{i}")}
                                role="option"
                                class={class}
                                style={style}
                                aria-selected={(i == active_idx).to_string()}
                                onmousedown={onmousedown}
                                onmousemove={onmousemove}
                            >
                                { s.label.clone() }
                                if let Some(h) = &s.hint {
                                    <small>{ h.clone() }</small>
                                }
                            </li>
                        }
                    }) }
                </ul>
            }
        </span>
    }
}

/// Insert plain text at the caret inside `el`, or at its end when the
/// selection is elsewhere. `None` when the browser would not tell.
fn insert_at_caret(el: &web_sys::Element, text: &str) -> Option<()> {
    let window = web_sys::window()?;
    let doc = window.document()?;
    let node = doc.create_text_node(text);
    let sel = window.get_selection().ok().flatten();
    let range = sel
        .as_ref()
        .filter(|s| s.range_count() > 0)
        .and_then(|s| s.get_range_at(0).ok())
        .filter(|r| {
            r.common_ancestor_container()
                .map(|c| el.contains(Some(&c)))
                .unwrap_or(false)
        });
    match (sel, range) {
        (Some(sel), Some(range)) => {
            range.delete_contents().ok()?;
            range.insert_node(&node).ok()?;
            range.set_start_after(&node).ok()?;
            range.collapse_with_to_start(true);
            sel.remove_all_ranges().ok()?;
            sel.add_range(&range).ok()?;
        }
        _ => {
            el.append_child(&node).ok()?;
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_text_never_trims() {
        assert_eq!(clean_text("  a\u{a0}b \n"), "  a b ");
        assert_eq!(clean_text("a\r\nb\tc"), "abc");
    }

    #[test]
    fn clean_edit_only_touches_what_changed() {
        // An untouched interior tab and no-break space survive a one-letter fix.
        assert_eq!(clean_edit("Acme\tCorpx", "Acme\tCorp"), "Acme\tCorpx");
        assert_eq!(
            clean_edit("research\u{a0}lab", "reserch\u{a0}lab"),
            "research\u{a0}lab"
        );
        // What was typed or pasted is cleaned.
        assert_eq!(clean_edit("a\u{a0}b", "ab"), "a b");
        assert_eq!(clean_edit("ab\t", "ab"), "ab");
        // Unchanged, grown, shrunk and empty values.
        assert_eq!(clean_edit("a\tb", "a\tb"), "a\tb");
        assert_eq!(clean_edit("", "a\tb"), "");
        assert_eq!(clean_edit("x", ""), "x");
        // Prefix and suffix may not overlap on repeated characters.
        assert_eq!(clean_edit("aa", "a"), "aa");
        assert_eq!(clean_edit("a", "aa"), "a");
    }

    #[test]
    fn clean_paste_turns_breaks_into_spaces() {
        assert_eq!(clean_paste("<b>x</b>\nY"), "<b>x</b> Y");
        assert_eq!(clean_paste("a\r\nb\tc"), "a b c");
    }

    #[test]
    fn odd_whitespace_is_flagged() {
        assert!(has_odd_whitespace(" a"));
        assert!(has_odd_whitespace("a\u{a0}b"));
        assert!(!has_odd_whitespace("a b"));
    }
}
