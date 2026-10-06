//! `OdrlProseView`: the sentences of an `EditDoc`, read or edited.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use yew::prelude::*;

use prose_core::edit::{
    ConditionList, ConditionProse, EditDoc, EditEvent, FocusTarget, Issue, IssueTarget, ListItem,
    ListKind, ListPath, Locked, LogicalOp, NewItem, NodeId, NodePath, PolicyProse, RefinementGroup,
    RuleList, RuleProse, Segment, Sentence, Severity, Slot, SlotList, focus_after, sentences,
};

use super::choice::{ChoiceSelect, RuleKindSelect};
use super::config::{ButtonContent, EditConfig, Reveal, cls, fill, merge_styles};
use super::css::BASE_CSS;
use super::slot::{SlotEditor, focus_end, issue_key};
use super::{Decoration, DecorationAt, EditMode, Env, EnvRef, SlotFocus};

#[derive(Properties, PartialEq, Clone)]
pub struct OdrlProseViewProps {
    /// Controlled: the host owns the document and applies edits.
    pub doc: Rc<EditDoc>,
    #[prop_or_default]
    pub mode: EditMode,
    #[prop_or_default]
    pub config: Rc<EditConfig>,
    /// Every committed edit and structural action. Not applied here. The
    /// host applies it before the user's next input (as `use_prose_editor`
    /// does, synchronously): a structural event whose new document has not
    /// arrived by then is treated as refused, and moves no focus and
    /// announces nothing.
    #[prop_or_default]
    pub onedit: Callback<EditEvent>,
    /// `Some` when a slot or select gets focus, `None` when it loses it.
    #[prop_or_default]
    pub on_focus_slot: Callback<Option<SlotFocus>>,
    /// Shown inline at their targets.
    #[prop_or_default]
    pub issues: Rc<Vec<Issue>>,
    /// Host badges after each policy heading, rule sentence and condition.
    #[prop_or_default]
    pub decorate: Option<Callback<Decoration, Html>>,
    #[prop_or_default]
    pub class: Classes,
}

fn is_structural(ev: &EditEvent) -> bool {
    !matches!(ev, EditEvent::SetText { .. } | EditEvent::SetChoice { .. })
}

/// The structural event waiting for the host's new document: the event, the
/// document it was emitted against, and a serial number.
type Pending = Rc<RefCell<Option<(EditEvent, usize, u64)>>>;

/// How long a structural event may wait for the host's answer when no user
/// input comes first. Far longer than any render, so a slow, multi-task
/// render still lands inside it.
#[cfg(target_arch = "wasm32")]
const PENDING_WINDOW_MS: i32 = 500;

#[cfg(target_arch = "wasm32")]
const EXPIRY_KINDS: [&str; 4] = ["pointerdown", "mousedown", "keydown", "click"];

/// What a registered expiry holds: the function the listeners and the timer
/// call, and the timer. `disarm` removes all of it, once.
#[cfg(target_arch = "wasm32")]
#[derive(Default)]
struct ExpiryShared {
    f: RefCell<Option<web_sys::js_sys::Function>>,
    timer: std::cell::Cell<Option<i32>>,
}

#[cfg(target_arch = "wasm32")]
impl ExpiryShared {
    fn disarm(&self) {
        let Some(f) = self.f.borrow_mut().take() else {
            return;
        };
        if let Some(w) = web_sys::window() {
            if let Some(t) = self.timer.take() {
                w.clear_timeout_with_handle(t);
            }
            if let Some(doc) = w.document() {
                for kind in EXPIRY_KINDS {
                    let _ = doc.remove_event_listener_with_callback_and_bool(kind, &f, true);
                }
            }
        }
    }
}

/// The registration of one pending event's expiry. Dropping it removes every
/// listener and the timer, so a view that is unmounted, or that emits a newer
/// event, leaves nothing behind.
struct ExpiryGuard {
    #[cfg(target_arch = "wasm32")]
    shared: Rc<ExpiryShared>,
    #[cfg(target_arch = "wasm32")]
    _closure: wasm_bindgen::closure::Closure<dyn Fn()>,
}

#[cfg(target_arch = "wasm32")]
impl Drop for ExpiryGuard {
    fn drop(&mut self) {
        self.shared.disarm();
    }
}

/// Forget the pending event with this serial number at the user's next
/// input, or after `PENDING_WINDOW_MS`. The host's answer (a new document)
/// comes before the next user input however long it takes Yew to render it,
/// whereas a short timer can fire first: Yew yields to the browser with
/// `setTimeout(0)` after 16 ms of work, and a timer queued before that yield
/// runs before the render. When no answer came, the event was refused, and a
/// later, unrelated change of the document must not run its focus move and
/// announcement. The first of the four input kinds or the timer clears the
/// event and removes all the others.
fn expire_pending_on_next_input(pending: Pending, serial: u64) -> Option<ExpiryGuard> {
    #[cfg(target_arch = "wasm32")]
    {
        let w = web_sys::window()?;
        let doc = w.document()?;
        let shared = Rc::new(ExpiryShared::default());
        let closure = {
            let shared = shared.clone();
            wasm_bindgen::closure::Closure::<dyn Fn()>::new(move || {
                {
                    let mut p = pending.borrow_mut();
                    if p.as_ref().is_some_and(|(_, _, s)| *s == serial) {
                        *p = None;
                    }
                }
                shared.disarm();
            })
        };
        let f: web_sys::js_sys::Function = closure
            .as_ref()
            .unchecked_ref::<web_sys::js_sys::Function>()
            .clone();
        for kind in EXPIRY_KINDS {
            let _ = doc.add_event_listener_with_callback_and_bool(kind, &f, true);
        }
        let timer = w
            .set_timeout_with_callback_and_timeout_and_arguments_0(&f, PENDING_WINDOW_MS)
            .ok();
        shared.timer.set(timer);
        *shared.f.borrow_mut() = Some(f);
        Some(ExpiryGuard {
            shared,
            _closure: closure,
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (pending, serial);
        None
    }
}

fn css_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// `rule_select`: the event changed a rule's kind, so focus goes to the
/// kind select of the moved rule (it shares its `data-node` with the rule's
/// `<li>`, which would otherwise win) and a keyboard user can keep going.
fn focus_target(root: &web_sys::Element, target: &FocusTarget, rule_select: bool) -> bool {
    let (primary, fallback) = match target {
        FocusTarget::Node(p) if rule_select => (
            format!("select[data-node=\"{}\"]", css_escape(&p.to_string())),
            Some(format!("[data-node=\"{}\"]", css_escape(&p.to_string()))),
        ),
        FocusTarget::Slot(s) => (
            format!("[data-slot=\"{}\"]", css_escape(&s.to_string())),
            Some(format!(
                "[data-node=\"{}\"]",
                css_escape(&s.node.to_string())
            )),
        ),
        FocusTarget::AddButton(l) => (
            format!("[data-add=\"{}\"]", css_escape(&l.to_string())),
            None,
        ),
        FocusTarget::Node(p) => (
            format!("[data-node=\"{}\"]", css_escape(&p.to_string())),
            None,
        ),
        // A target this version cannot locate leaves focus where it is.
        _ => return false,
    };
    for sel in std::iter::once(primary).chain(fallback) {
        if let Ok(Some(el)) = root.query_selector(&sel) {
            focus_end(&el);
            return true;
        }
    }
    false
}

#[function_component(OdrlProseView)]
pub fn odrl_prose_view(props: &OdrlProseViewProps) -> Html {
    let cfg = props.config.clone();
    let edit = props.mode == EditMode::Edit;
    let prose = use_memo(props.doc.clone(), |d| sentences(d));
    let root = use_node_ref();
    let live = use_state(AttrValue::default);
    let pending: Pending = use_mut_ref(|| None);
    let serial = use_mut_ref(|| 0u64);
    // The expiry of the pending event; emptied on unmount.
    let expiry: Rc<RefCell<Option<ExpiryGuard>>> = use_mut_ref(|| None);
    {
        let expiry = expiry.clone();
        use_effect_with((), move |_| {
            move || {
                expiry.borrow_mut().take();
            }
        });
    }
    let ptr = Rc::as_ptr(&props.doc) as usize;

    let onedit = {
        let pending = pending.clone();
        let serial = serial.clone();
        let expiry = expiry.clone();
        let live = live.clone();
        let onedit = props.onedit.clone();
        Callback::from(move |ev: EditEvent| {
            if is_structural(&ev) {
                // Empty the live region first: the same message twice in a
                // row only reaches assistive technology if the text changes
                // in between (Yew leaves an unchanged text node alone).
                live.set(AttrValue::default());
                let id = {
                    let mut n = serial.borrow_mut();
                    *n += 1;
                    *n
                };
                *pending.borrow_mut() = Some((ev.clone(), ptr, id));
                let guard = expire_pending_on_next_input(pending.clone(), id);
                // Replacing the old guard unregisters its listeners.
                *expiry.borrow_mut() = guard;
            }
            onedit.emit(ev);
        })
    };
    let on_focus_slot = {
        let pending = pending.clone();
        let cb = props.on_focus_slot.clone();
        Callback::from(move |f: Option<SlotFocus>| {
            if f.is_some() {
                *pending.borrow_mut() = None;
            }
            cb.emit(f);
        })
    };

    {
        let pending = pending.clone();
        let expiry = expiry.clone();
        let doc = props.doc.clone();
        let root = root.clone();
        let live = live.clone();
        let cfg = cfg.clone();
        use_effect_with(ptr, move |ptr| {
            let taken = {
                let mut p = pending.borrow_mut();
                match p.as_ref() {
                    Some((_, old, _)) if old != ptr => p.take(),
                    _ => None,
                }
            };
            if let Some((ev, _, _)) = taken {
                // Answered: nothing is left to expire.
                expiry.borrow_mut().take();
                let mut done = false;
                if let Some(el) = root.cast::<web_sys::Element>() {
                    if let Some(t) = focus_after(&ev, &doc) {
                        done =
                            focus_target(&el, &t, matches!(ev, EditEvent::ChangeRuleKind { .. }));
                    }
                    if !done && let Some(h) = el.dyn_ref::<web_sys::HtmlElement>() {
                        let _ = h.focus();
                    }
                }
                let labels = &cfg.labels;
                match &ev {
                    EditEvent::Add { list, .. } => live.set(AttrValue::from(fill(
                        &labels.announce_added,
                        &[("noun", labels.nouns.list(list.kind))],
                    ))),
                    EditEvent::Remove { list, .. } => live.set(AttrValue::from(fill(
                        &labels.announce_removed,
                        &[("noun", labels.nouns.list(list.kind))],
                    ))),
                    _ => {}
                }
            }
            || ()
        });
    }

    let env = Rc::new(Env {
        cfg: cfg.clone(),
        onedit,
        on_focus_slot,
        policy_uids: props
            .doc
            .policies
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                p.uid
                    .as_ref()
                    .filter(|u| !u.is_empty())
                    .map(|u| (i, AttrValue::from(u.clone())))
            })
            .collect(),
    });
    let mut index: HashMap<String, Vec<&Issue>> = HashMap::new();
    for i in props.issues.iter() {
        index.entry(issue_key(&i.target)).or_default().push(i);
    }
    let r = Rend {
        env,
        edit,
        doc: &props.doc,
        issues: index,
        decorate: &props.decorate,
    };

    let mut class = classes!("ds-prose", "prose-view", cfg.classes.root.clone());
    if edit {
        class.push("prose-edit");
        if cfg.reveal == Reveal::OnHoverOrFocus {
            class.push("prose-reveal-hover");
        }
    }
    class.extend(props.class.clone());

    html! {
        <article ref={root} class={class} style={cfg.styles.root.clone()} tabindex="-1">
            if edit && cfg.base_css {
                <style>{ BASE_CSS }</style>
            }
            { for prose.iter().map(|p| r.policy(p)) }
            if prose.is_empty() {
                <p class={cls!(cfg, empty, "prose-empty")} style={cfg.styles.empty.clone()}>
                    { cfg.labels.no_policies.clone() }
                </p>
            }
            if edit {
                <div class={cls!(cfg, add_bar, "prose-add-bar")} style={cfg.styles.add_bar.clone()}>
                    { r.add_button(&ListPath::policies(), props.doc.policies.len(), NewItem::Default, None) }
                </div>
            }
            { r.doc_notes() }
            if edit {
                <div class={cls!(cfg, live, "prose-live")} style={cfg.styles.live.clone()} role="status" aria-live="polite">
                    { (*live).clone() }
                </div>
            }
        </article>
    }
}

struct Rend<'a> {
    env: Rc<Env>,
    edit: bool,
    doc: &'a EditDoc,
    issues: HashMap<String, Vec<&'a Issue>>,
    decorate: &'a Option<Callback<Decoration, Html>>,
}

fn join_html(items: Vec<Html>, conj: &str) -> Html {
    let n = items.len();
    let mut out: Vec<Html> = Vec::with_capacity(n * 2);
    for (i, h) in items.into_iter().enumerate() {
        if i > 0 {
            out.push(Html::from(if i == n - 1 {
                format!(" {conj} ")
            } else {
                ", ".to_string()
            }));
        }
        out.push(h);
    }
    html! { <>{ for out }</> }
}

fn collect_keys(s: &Sentence, keys: &mut Vec<String>) {
    for seg in &s.segments {
        collect_segment_keys(seg, keys);
    }
}

fn collect_segment_keys(seg: &Segment, keys: &mut Vec<String>) {
    match seg {
        Segment::Slot(s) => keys.push(issue_key(&IssueTarget::Slot(s.path.clone()))),
        Segment::Choice(c) => keys.push(issue_key(&IssueTarget::Slot(c.path.clone()))),
        Segment::List(l) => {
            keys.push(issue_key(&IssueTarget::List(l.path.clone())));
            for it in &l.items {
                keys.push(issue_key(&IssueTarget::Slot(it.slot.path.clone())));
                for e in &it.extra {
                    collect_segment_keys(e, keys);
                }
            }
        }
        // Text and rule-kind segments have no issue key, and neither does a
        // segment kind this version does not know.
        _ => {}
    }
}

impl Rend<'_> {
    fn cfg(&self) -> &EditConfig {
        &self.env.cfg
    }

    fn emit(&self, ev: EditEvent) -> Callback<MouseEvent> {
        let cb = self.env.onedit.clone();
        Callback::from(move |_| cb.emit(ev.clone()))
    }

    // --- issues and decorations ---------------------------------------------------

    fn issue_ids(&self, key: &str) -> Option<AttrValue> {
        let n = self.issues.get(key)?.len();
        (n > 0).then(|| {
            AttrValue::from(
                (0..n)
                    .map(|i| format!("prose-issue-{key}-{i}"))
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        })
    }

    fn severity(&self, key: &str) -> Option<Severity> {
        self.issues
            .get(key)
            .and_then(|v| v.iter().map(|i| i.severity).min())
    }

    /// The issues at the given keys, as blocks after the sentence.
    fn issue_block(&self, keys: &[String]) -> Html {
        html! { <>{ for self.issue_items(keys) }</> }
    }

    /// One element per issue at the given keys.
    fn issue_items(&self, keys: &[String]) -> Vec<Html> {
        let cfg = self.cfg();
        let mut out = vec![];
        for key in keys {
            let Some(list) = self.issues.get(key) else {
                continue;
            };
            for (i, issue) in list.iter().enumerate() {
                let (sev, sev_style) = match issue.severity {
                    Severity::Error => (
                        cls!(cfg, issue_error, "prose-issue-error"),
                        &cfg.styles.issue_error,
                    ),
                    Severity::Warning => (
                        cls!(cfg, issue_warning, "prose-issue-warning"),
                        &cfg.styles.issue_warning,
                    ),
                    Severity::Info => (
                        cls!(cfg, issue_info, "prose-issue-info"),
                        &cfg.styles.issue_info,
                    ),
                };
                let style = merge_styles(&[&cfg.styles.issue, sev_style]);
                out.push(html! {
                    <span
                        id={format!("prose-issue-{key}-{i}")}
                        class={classes!(cls!(cfg, issue, "prose-issue"), sev)}
                        style={style}
                    >
                        { issue.message.clone() }
                    </span>
                });
            }
        }
        out
    }

    fn node_key(path: &NodePath) -> String {
        issue_key(&IssueTarget::Node(path.clone()))
    }

    /// Node issues plus the issues of every slot and list in the sentences.
    fn issues_for(&self, node: &NodePath, sentences: &[&Sentence]) -> Html {
        if self.issues.is_empty() {
            return Html::default();
        }
        let mut keys = vec![Self::node_key(node)];
        for s in sentences {
            collect_keys(s, &mut keys);
        }
        self.issue_block(&keys)
    }

    fn decoration(&self, at: DecorationAt, path: &NodePath, node: NodeId) -> Html {
        match self.decorate {
            Some(cb) => {
                let cfg = self.cfg();
                html! {
                    <span class={cls!(cfg, decoration, "prose-decoration")} style={cfg.styles.decoration.clone()}>
                        { cb.emit(Decoration { at, path: path.clone(), node }) }
                    </span>
                }
            }
            None => Html::default(),
        }
    }

    // --- buttons --------------------------------------------------------------------

    fn add_button(&self, list: &ListPath, index: usize, item: NewItem, noun: Option<&str>) -> Html {
        if !self.edit || !self.cfg().rules.can_add(self.doc, list) {
            return Html::default();
        }
        let cfg = self.cfg();
        let l = &cfg.labels;
        let noun = noun.map(str::to_string).unwrap_or_else(|| match item {
            NewItem::Logical(_) => l.nouns.group.to_string(),
            _ => l.nouns.list(list.kind).to_string(),
        });
        let owner = l.describe_list_owner(list);
        let name = l.button_name(&l.add, &noun, None, &owner);
        let content = match cfg.add_button {
            ButtonContent::Symbol => l.add_symbol.to_string(),
            ButtonContent::Text => fill(&l.add, &[("noun", &noun)]),
            ButtonContent::SymbolAndText => format!("{} {}", l.add_symbol, noun),
        };
        let ev = EditEvent::Add {
            expect: self.doc.list_owner_id(list),
            list: list.clone(),
            index,
            item,
        };
        html! {
            <button
                type="button"
                class={cls!(cfg, add, "prose-add")}
                style={cfg.styles.add.clone()}
                data-add={list.to_string()}
                data-item={matches!(item, NewItem::Logical(_)).then_some("group")}
                aria-label={name}
                onmousedown={focus_on_press()}
                onclick={self.emit(ev)}
            >
                { content }
            </button>
        }
    }

    fn remove_button(
        &self,
        list: &ListPath,
        index: usize,
        expect: NodeId,
        noun: Option<&str>,
    ) -> Html {
        if !self.edit || !self.cfg().rules.can_remove(self.doc, list) {
            return Html::default();
        }
        let cfg = self.cfg();
        let l = &cfg.labels;
        let noun = noun
            .map(str::to_string)
            .unwrap_or_else(|| l.nouns.list(list.kind).to_string());
        let owner = l.describe_list_owner(list);
        let name = l.button_name(&l.remove, &noun, Some(index + 1), &owner);
        let content = match cfg.remove_button {
            ButtonContent::Symbol => l.remove_symbol.to_string(),
            ButtonContent::Text => fill(
                &l.remove,
                &[("noun", &noun), ("n", &(index + 1).to_string())],
            ),
            ButtonContent::SymbolAndText => format!("{} {}", l.remove_symbol, noun),
        };
        let ev = EditEvent::Remove {
            list: list.clone(),
            index,
            expect,
        };
        html! {
            <button
                type="button"
                class={cls!(cfg, remove, "prose-remove")}
                style={cfg.styles.remove.clone()}
                aria-label={name}
                onmousedown={focus_on_press()}
                onclick={self.emit(ev)}
            >
                { content }
            </button>
        }
    }

    fn move_buttons(
        &self,
        list: &ListPath,
        index: usize,
        has_next: bool,
        expect: NodeId,
        noun: Option<&str>,
    ) -> Html {
        if !self.edit || !self.cfg().rules.allow_reorder {
            return Html::default();
        }
        let cfg = self.cfg();
        let l = &cfg.labels;
        let noun = noun
            .map(str::to_string)
            .unwrap_or_else(|| l.nouns.list(list.kind).to_string());
        let owner = l.describe_list_owner(list);
        let button = |up: bool| {
            let template = if up { &l.move_up } else { &l.move_down };
            let name = l.button_name(template, &noun, Some(index + 1), &owner);
            let to = if up { index.wrapping_sub(1) } else { index + 1 };
            let ev = EditEvent::Move {
                list: list.clone(),
                from: index,
                to,
                expect,
            };
            html! {
                <button
                    type="button"
                    class={cls!(cfg, reorder, "prose-reorder")}
                    style={cfg.styles.reorder.clone()}
                    aria-label={name}
                    onmousedown={focus_on_press()}
                    onclick={self.emit(ev)}
                >
                    { if up { l.move_up_symbol.clone() } else { l.move_down_symbol.clone() } }
                </button>
            }
        };
        html! {
            <>
                if index > 0 { { button(true) } }
                if has_next { { button(false) } }
            </>
        }
    }

    fn has_node_at(&self, list: &ListPath, index: usize) -> bool {
        list.item_path(index)
            .and_then(|p| self.doc.node(&p))
            .is_some()
    }

    fn toolbar(&self, inner: Html) -> Html {
        if !self.edit {
            return Html::default();
        }
        let cfg = self.cfg();
        html! {
            <span class={cls!(cfg, toolbar, "prose-toolbar")} style={cfg.styles.toolbar.clone()}>{ inner }</span>
        }
    }

    // --- sentences ------------------------------------------------------------------

    fn term(&self, display: &str, raw: &str) -> Html {
        let cfg = self.cfg();
        html! {
            <span class={cls!(cfg, term, "prose-term")} style={cfg.styles.term.clone()} title={raw.to_string()}>
                { display.to_string() }
            </span>
        }
    }

    fn slot(&self, s: &Slot) -> Html {
        if !self.edit {
            return if s.display.is_empty() {
                Html::default()
            } else {
                self.term(&s.display, &s.raw)
            };
        }
        let key = issue_key(&IssueTarget::Slot(s.path.clone()));
        // The collection form reads "any asset in the collection X"; the
        // words before the editable part stay as plain text.
        let prefix = (s.kind == prose_core::edit::SlotKind::PartOf)
            .then(|| s.display.strip_suffix(s.raw.as_str()))
            .flatten()
            .filter(|p| !p.is_empty());
        html! {
            <>
                if let Some(p) = prefix { { p.to_string() } }
                <SlotEditor
                    slot={s.clone()}
                    env={EnvRef(self.env.clone())}
                    severity={self.severity(&key)}
                    described_by={self.issue_ids(&key)}
                />
            </>
        }
    }

    fn segment(&self, seg: &Segment) -> Html {
        let cfg = self.cfg();
        match seg {
            // Wrapped only when the host styles plain text, so the default
            // markup stays as it was.
            Segment::Text(t) if cfg.classes.text.is_empty() && cfg.styles.text.is_none() => {
                html! { { t.clone() } }
            }
            Segment::Text(t) => html! {
                <span class={cls!(cfg, text, "prose-text")} style={cfg.styles.text.clone()}>{ t.clone() }</span>
            },
            Segment::Slot(s) => self.slot(s),
            Segment::Choice(c) => {
                if self.edit {
                    let key = issue_key(&IssueTarget::Slot(c.path.clone()));
                    html! {
                        <ChoiceSelect
                            slot={c.clone()}
                            env={EnvRef(self.env.clone())}
                            severity={self.severity(&key)}
                            described_by={self.issue_ids(&key)}
                        />
                    }
                } else if c.display.is_empty() {
                    Html::default()
                } else {
                    self.term(&c.display, &c.raw)
                }
            }
            Segment::RuleKind(k) => {
                if self.edit && k.changeable && cfg.rules.allow_rule_kind_change {
                    html! {
                        <RuleKindSelect
                            slot={k.clone()}
                            env={EnvRef(self.env.clone())}
                            kinds={self.offered_kinds(k)}
                        />
                    }
                } else {
                    let l = &cfg.labels;
                    html! {
                        { match k.current {
                            RuleList::Permission => l.may.clone(),
                            RuleList::Prohibition => l.must_not.clone(),
                            _ => l.must.clone(),
                        } }
                    }
                }
            }
            Segment::List(l) => self.slot_list(l),
            // A segment kind this version does not know renders nothing.
            _ => Html::default(),
        }
    }

    /// The kinds a rule can change to: its own, and every other top-level
    /// kind whose list the rules let the reducer move it into (the same
    /// checks as the + and - buttons, so an option is never offered that the
    /// reducer would refuse).
    fn offered_kinds(&self, k: &prose_core::edit::RuleKindSlot) -> Vec<RuleList> {
        let owner = k.rule.parent().unwrap_or_else(|| k.rule.clone());
        let rules = &self.cfg().rules;
        let can_leave =
            rules.can_remove(self.doc, &ListPath::of(&owner, ListKind::Rules(k.current)));
        [
            RuleList::Permission,
            RuleList::Prohibition,
            RuleList::Obligation,
        ]
        .into_iter()
        .filter(|to| {
            *to == k.current
                || (can_leave
                    && rules.can_add(self.doc, &ListPath::of(&owner, ListKind::Rules(*to))))
        })
        .collect()
    }

    fn sentence(&self, s: &Sentence) -> Html {
        html! { <>{ for s.segments.iter().map(|seg| self.segment(seg)) }</> }
    }

    fn locked(&self, l: &Locked) -> Html {
        let cfg = self.cfg();
        html! {
            <span class={cls!(cfg, locked, "prose-locked-note")}>
                <code class={cls!(cfg, locked, "prose-locked")} style={cfg.styles.locked.clone()}>{ l.json.clone() }</code>
                { " " }
                { format!("{} ({})", cfg.labels.locked, l.reason) }
            </span>
        }
    }

    fn list_item(&self, l: &SlotList, it: &ListItem) -> Html {
        let cfg = self.cfg();
        let expect = it.node.unwrap_or(l.owner);
        let has_next = it.index + 1 < l.items.len();
        let controls = self.toolbar(html! {
            <>
                { self.move_buttons(&l.path, it.index, has_next, expect, None) }
                { self.remove_button(&l.path, it.index, expect, None) }
            </>
        });
        let data_node = it.node.map(|_| it.slot.path.node.to_string());
        // Issues the host put on the item's node (an action or an entity).
        let node_issues = if it.node.is_some() && !self.issues.is_empty() {
            self.issue_block(&[Self::node_key(&it.slot.path.node)])
        } else {
            Html::default()
        };
        let body = match &it.locked {
            Some(locked) => self.locked(locked),
            None => html! {
                <>
                    { self.slot(&it.slot) }
                    { for it.extra.iter().map(|s| self.segment(s)) }
                </>
            },
        };
        html! {
            <span
                key={it.node.map_or_else(|| format!("i{}", it.index), |n| n.0.to_string())}
                class={cls!(cfg, item, "prose-item")}
                style={cfg.styles.item.clone()}
                data-node={data_node.clone()}
                tabindex={data_node.map(|_| "-1")}
            >
                { body }
                { controls }
                { node_issues }
            </span>
        }
    }

    fn slot_list(&self, l: &SlotList) -> Html {
        let cfg = self.cfg();
        let conj = match l.conj {
            prose_core::edit::Conj::And => "and",
            prose_core::edit::Conj::Or => "or",
        };
        let lead = l.lead.clone().unwrap_or_default();
        let body = if !l.items.is_empty() {
            let items: Vec<Html> = l.items.iter().map(|it| self.list_item(l, it)).collect();
            html! { <>{ lead }{ join_html(items, conj) }</> }
        } else if !l.inherited.is_empty() {
            let items: Vec<Html> = l
                .inherited
                .iter()
                .map(|s| {
                    html! {
                        <span
                            class={cls!(cfg, inherited, "prose-inherited")}
                            style={cfg.styles.inherited.clone()}
                            title={cfg.labels.inherited.clone()}
                        >
                            { s.display.clone() }
                        </span>
                    }
                })
                .collect();
            html! { <>{ lead }{ join_html(items, conj) }</> }
        } else {
            let text = cfg.labels.empty.get(l.empty);
            if text.is_empty() {
                Html::default()
            } else {
                html! {
                    <span class={cls!(cfg, empty, "prose-empty")} style={cfg.styles.empty.clone()}>
                        { text }
                    </span>
                }
            }
        };
        let add = self.add_button(&l.path, l.items.len(), NewItem::Default, None);
        html! { <>{ body }{ add }</> }
    }

    // --- conditions -----------------------------------------------------------------

    fn condition_list(&self, cl: &ConditionList, label: Option<String>) -> Html {
        if !self.edit && cl.items.is_empty() {
            return Html::default();
        }
        let cfg = self.cfg();
        let add = if self.edit {
            let logical = if cfg.rules.allow_logical {
                self.add_button(
                    &cl.list,
                    cl.items.len(),
                    NewItem::Logical(LogicalOp::And),
                    None,
                )
            } else {
                Html::default()
            };
            html! {
                <div class={cls!(cfg, add_bar, "prose-add-bar")} style={cfg.styles.add_bar.clone()}>
                    { self.add_button(&cl.list, cl.items.len(), NewItem::Default, None) }
                    { logical }
                    { self.issue_block(&[issue_key(&IssueTarget::List(cl.list.clone()))]) }
                </div>
            }
        } else {
            Html::default()
        };
        html! {
            <div class={cls!(cfg, conditions, "prose-conditions")} style={cfg.styles.conditions.clone()}>
                if let Some(label) = label {
                    <p class={cls!(cfg, label, "prose-label")} style={cfg.styles.label.clone()}>
                        { format!("{label}:") }
                    </p>
                }
                if !cl.items.is_empty() {
                    <ul class={cls!(cfg, list, "prose-list")} style={cfg.styles.list.clone()}>
                        { for cl.items.iter().map(|c| self.condition(cl, c)) }
                    </ul>
                }
                { add }
            </div>
        }
    }

    fn condition(&self, cl: &ConditionList, c: &ConditionProse) -> Html {
        let cfg = self.cfg();
        let mut class = cls!(cfg, condition, "prose-condition");
        let mut styles = vec![&cfg.styles.condition];
        if c.children.is_some() {
            class.extend(cls!(cfg, logical, "prose-logical"));
            styles.push(&cfg.styles.logical);
        }
        let condition_style = merge_styles(&styles);
        let expect = c.node;
        let has_next = c.index + 1 < cl.items.len();
        let wrap = if self.edit && cfg.rules.allow_logical && c.locked.is_none() {
            let l = &cfg.labels;
            let owner = l.describe(&c.path.parent().unwrap_or_else(|| c.path.clone()));
            let noun = l.nouns.condition.to_string();
            let name = |t: &str| l.button_name(t, &noun, Some(c.index + 1), &owner);
            let wrap_btn = |label: &str, text: &str, ev: EditEvent| {
                html! {
                    <button
                        type="button"
                        class={cls!(cfg, wrap, "prose-wrap")}
                        style={cfg.styles.wrap.clone()}
                        aria-label={label.to_string()}
                        onmousedown={focus_on_press()}
                        onclick={self.emit(ev)}
                    >
                        { text.to_string() }
                    </button>
                }
            };
            let can_unwrap = c.children.as_ref().is_some_and(|k| k.items.len() == 1);
            html! {
                <>
                    { wrap_btn(&name(&l.wrap), &l.wrap_text, EditEvent::Wrap {
                        constraint: c.path.clone(), expect, op: LogicalOp::And,
                    }) }
                    if can_unwrap {
                        { wrap_btn(&name(&l.unwrap), &l.unwrap_text, EditEvent::Unwrap {
                            constraint: c.path.clone(), expect,
                        }) }
                    }
                </>
            }
        } else {
            Html::default()
        };
        let controls = self.toolbar(html! {
            <>
                { self.move_buttons(&cl.list, c.index, has_next, expect, None) }
                { wrap }
                { self.remove_button(&cl.list, c.index, expect, None) }
            </>
        });
        let issues = self.issues_for(&c.path, &[&c.sentence]);
        let body = match &c.locked {
            Some(l) => self.locked(l),
            None => self.sentence(&c.sentence),
        };
        html! {
            <li
                key={c.node.0.to_string()}
                class={class}
                style={condition_style}
                data-node={c.path.to_string()}
                tabindex="-1"
            >
                { body }
                if c.children.is_some() { { ":" } }
                { self.decoration(DecorationAt::Condition, &c.path, c.node) }
                { controls }
                { issues }
                if let Some(children) = &c.children {
                    { self.condition_list(children, None) }
                }
            </li>
        }
    }

    fn refinement_groups(&self, groups: &[RefinementGroup]) -> Html {
        let l = &self.cfg().labels;
        html! {
            <>
                { for groups.iter().map(|g| {
                    let label = fill(&l.refinements, &[("action", &g.action_name)]);
                    self.condition_list(&g.conditions, Some(label))
                }) }
            </>
        }
    }

    // --- rules ----------------------------------------------------------------------

    fn rule(&self, r: &RuleProse, list: &ListPath, has_next: bool) -> Html {
        let cfg = self.cfg();
        let (kind, kind_style) = match r.list {
            RuleList::Permission => (
                cls!(cfg, permission, "prose-permission"),
                &cfg.styles.permission,
            ),
            RuleList::Prohibition => (
                cls!(cfg, prohibition, "prose-prohibition"),
                &cfg.styles.prohibition,
            ),
            _ => (
                cls!(cfg, obligation, "prose-obligation"),
                &cfg.styles.obligation,
            ),
        };
        let rule_style = merge_styles(&[&cfg.styles.rule, kind_style]);
        let mut class = classes!(cls!(cfg, rule, "prose-rule"), kind);
        class.push(format!("prose-{}", r.list.as_str()));
        let controls = self.toolbar(html! {
            <>
                { self.move_buttons(list, r.index, has_next, r.node, None) }
                { self.remove_button(list, r.index, r.node, None) }
            </>
        });
        let li = |inner: Html| {
            html! {
                <li
                    key={r.node.0.to_string()}
                    class={class.clone()}
                    style={rule_style.clone()}
                    data-node={r.path.to_string()}
                    tabindex="-1"
                >
                    { inner }
                </li>
            }
        };
        if let Some(locked) = &r.locked {
            return li(html! {
                <>
                    <p class={cls!(cfg, sentence, "prose-rule-sentence")} style={cfg.styles.sentence.clone()}>
                        { self.locked(locked) }
                        { controls }
                    </p>
                    { self.issues_for(&r.path, &[]) }
                </>
            });
        }
        let cond_label = if r.conditions.in_duty {
            &cfg.labels.conditions_in_duty
        } else {
            &cfg.labels.conditions
        };
        li(html! {
            <>
                <p class={cls!(cfg, sentence, "prose-rule-sentence")} style={cfg.styles.sentence.clone()}>
                    { self.sentence(&r.sentence) }
                    { self.decoration(DecorationAt::Rule, &r.path, r.node) }
                    { controls }
                </p>
                { self.issues_for(&r.path, &[&r.sentence]) }
                { self.refinement_groups(&r.refinements) }
                { self.condition_list(&r.conditions, Some(cond_label.to_string())) }
                { for r.follow_ups.iter().map(|f| self.follow_up(f)) }
            </>
        })
    }

    fn follow_up(&self, f: &prose_core::edit::FollowUpProse) -> Html {
        let cfg = self.cfg();
        let l = &cfg.labels;
        if !self.edit && f.rules.is_empty() {
            return Html::default();
        }
        let mut label = match f.kind {
            RuleList::Duty if f.in_odrl_position => l.follow_up_duty.to_string(),
            RuleList::Duty => l.follow_up_duty_elsewhere.to_string(),
            RuleList::Remedy if f.in_odrl_position => l.follow_up_remedy.to_string(),
            RuleList::Remedy => l.follow_up_remedy_elsewhere.to_string(),
            _ if f.in_odrl_position => l.follow_up_consequence.to_string(),
            _ => l.follow_up_consequence_elsewhere.to_string(),
        };
        if !f.in_odrl_position && !l.carried_note.is_empty() {
            label = format!("{label} {}", l.carried_note);
        }
        let n = f.rules.len();
        let add = if self.edit {
            html! {
                <div class={cls!(cfg, add_bar, "prose-add-bar")} style={cfg.styles.add_bar.clone()}>
                    { self.add_button(&f.list, n, NewItem::Default, None) }
                    { self.issue_block(&[issue_key(&IssueTarget::List(f.list.clone()))]) }
                </div>
            }
        } else {
            Html::default()
        };
        let mut class = cls!(cfg, follow_up, "prose-follow-up");
        let mut styles = vec![&cfg.styles.follow_up];
        if !f.in_odrl_position {
            class.extend(cls!(cfg, carried, "prose-carried"));
            styles.push(&cfg.styles.carried);
        }
        let follow_style = merge_styles(&styles);
        html! {
            <div class={class} style={follow_style}>
                <p class={cls!(cfg, label, "prose-label")} style={cfg.styles.label.clone()}>{ format!("{label}:") }</p>
                if n > 0 {
                    <ul class={cls!(cfg, list, "prose-list")} style={cfg.styles.list.clone()}>
                        { for f.rules.iter().enumerate().map(|(i, r)| self.rule(r, &f.list, i + 1 < n)) }
                    </ul>
                }
                { add }
            </div>
        }
    }

    // --- policies -------------------------------------------------------------------

    fn note_is_empty(s: &Sentence) -> bool {
        s.segments.iter().any(|seg| match seg {
            Segment::List(l) => l.items.is_empty(),
            Segment::Choice(c) => c.raw.is_empty(),
            _ => false,
        })
    }

    /// An empty note whose list the host's rules do not let anyone fill
    /// ("Applies to ." where the host cannot carry a policy-level target)
    /// is dead text in the editor: no content, and no + to add any.
    fn note_is_unreachable(&self, s: &Sentence) -> bool {
        s.segments.iter().any(|seg| match seg {
            Segment::List(l) => l.items.is_empty() && !self.cfg().rules.can_add(self.doc, &l.path),
            _ => false,
        })
    }

    /// The policy heading. The toolbar sits beside the `<h3>`, not in it: a
    /// heading's accessible name is made from its content, and the buttons'
    /// names would otherwise be read in every list of headings. In Read mode
    /// there is no toolbar and the bare `<h3>` is kept.
    fn heading_row(&self, inner: Html, controls: Html) -> Html {
        let cfg = self.cfg();
        let h3 = html! {
            <h3 class={cls!(cfg, heading, "prose-heading")} style={cfg.styles.heading.clone()}>
                { inner }
            </h3>
        };
        if !self.edit {
            return h3;
        }
        html! { <div class="prose-heading-row">{ h3 }{ controls }</div> }
    }

    fn policy(&self, p: &PolicyProse) -> Html {
        let cfg = self.cfg();
        let list = ListPath::policies();
        let index = p.path.policy;
        let has_next = index + 1 < self.doc.policies.len();
        let controls = self.toolbar(html! {
            <>
                { self.move_buttons(&list, index, has_next, p.node, None) }
                { self.remove_button(&list, index, p.node, None) }
            </>
        });
        let section = |inner: Html| {
            html! {
                <section
                    key={p.node.0.to_string()}
                    class={cls!(cfg, policy, "prose-policy")}
                    style={cfg.styles.policy.clone()}
                    data-node={p.path.to_string()}
                    tabindex="-1"
                >
                    { inner }
                </section>
            }
        };
        if let Some(locked) = &p.locked {
            return section(html! {
                <>
                    { self.heading_row(self.locked(locked), controls) }
                    { self.issues_for(&p.path, &[]) }
                </>
            });
        }
        let notes: Vec<&Sentence> = p
            .notes
            .iter()
            .filter(|n| {
                if self.edit {
                    !self.note_is_unreachable(n)
                } else {
                    !Self::note_is_empty(n)
                }
            })
            .collect();
        let n_rules = p.rules.len();
        let rule_lists: Vec<ListPath> = p.add_rule_lists.clone();
        let rules = p.rules.iter().map(|r| {
            let owner = r.path.parent().unwrap_or_else(|| p.path.clone());
            let list = ListPath::of(&owner, ListKind::Rules(r.list));
            let has_next = self.has_node_at(&list, r.index + 1);
            self.rule(r, &list, has_next)
        });
        section(html! {
            <>
                { self.heading_row(
                    html! {
                        <>
                            { self.sentence(&p.heading) }
                            { self.decoration(DecorationAt::Policy, &p.path, p.node) }
                        </>
                    },
                    controls,
                ) }
                { self.issues_for(&p.path, &[&p.heading, &p.intro]) }
                <p class={cls!(cfg, intro, "prose-intro")} style={cfg.styles.intro.clone()}>
                    { self.sentence(&p.intro) }
                </p>
                if !notes.is_empty() {
                    <ul class={cls!(cfg, notes, "prose-notes")} style={cfg.styles.notes.clone()}>
                        { for notes.iter().map(|n| html! {
                            <li>{ self.sentence(n) }</li>
                        }) }
                    </ul>
                }
                { self.issue_notes(p) }
                { self.refinement_groups(&p.refinements) }
                if n_rules > 0 {
                    <ol class={cls!(cfg, rules, "prose-rules")} style={cfg.styles.rules.clone()}>
                        { for rules }
                    </ol>
                } else if self.edit {
                    <p class={cls!(cfg, empty, "prose-empty")} style={cfg.styles.empty.clone()}>
                        { cfg.labels.no_rules.clone() }
                    </p>
                }
                if self.edit {
                    <div class={cls!(cfg, add_bar, "prose-add-bar")} style={cfg.styles.add_bar.clone()}>
                        { for rule_lists.iter().map(|l| {
                            let at = self.rule_list_len(l);
                            self.add_button(l, at, NewItem::Default, None)
                        }) }
                    </div>
                }
            </>
        })
    }

    /// Slot and list issues of the notes (they are not shown by the note
    /// itself, to keep notes one line).
    fn issue_notes(&self, p: &PolicyProse) -> Html {
        if self.issues.is_empty() {
            return Html::default();
        }
        let mut keys = vec![];
        for n in &p.notes {
            collect_keys(n, &mut keys);
        }
        self.issue_block(&keys)
    }

    fn rule_list_len(&self, list: &ListPath) -> usize {
        (0..).take_while(|i| self.has_node_at(list, *i)).count()
    }

    fn doc_notes(&self) -> Html {
        let cfg = self.cfg();
        let doc_issues = self.issues.get("doc");
        if self.doc.warnings.is_empty() && doc_issues.is_none_or(|v| v.is_empty()) {
            return Html::default();
        }
        html! {
            <aside class={cls!(cfg, warnings, "prose-warnings")} style={cfg.styles.warnings.clone()}>
                <p class={cls!(cfg, label, "prose-label")} style={cfg.styles.label.clone()}>
                    { cfg.labels.notes_heading.clone() }
                </p>
                <ul>
                    { for self.doc.warnings.iter().map(|w| html! { <li>{ w.clone() }</li> }) }
                    { for self.issue_items(&["doc".to_string()]).into_iter().map(|i| html! { <li>{ i }</li> }) }
                </ul>
            </aside>
        }
    }
}

/// Buttons take focus on press in every browser (Safari does not give
/// them focus on click), so a slot being edited commits first.
fn focus_on_press() -> Callback<MouseEvent> {
    Callback::from(|e: MouseEvent| {
        if let Some(el) = e
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = el.focus();
        }
    })
}
