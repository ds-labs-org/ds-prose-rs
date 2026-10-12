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

use std::collections::{HashMap, HashSet};

use prose_core::edit::{
    ConstraintNode, EditDoc, EditEvent, Field, IssueTarget, Literal, NodeRef, RightOperand,
    Severity, Slot, SlotKind, SlotPath, left_operand_at,
};
use prose_core::profile::{Term, TermKind};
use prose_core::validate::{Level, Mode, RuleId, Validator};
use prose_core::wording;

use super::closed;
use super::config::{Suggestion, cls, merge_styles, written_form};
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

/// A text no profile defines, to ask the validator whether a slot takes
/// anything: if it would refuse this, the slot is closed.
const PROBE: &str = "https://ds-prose.invalid/not-a-term";

/// The validator of a strict configuration.
fn strict(env: &Env) -> Option<&Validator> {
    env.cfg
        .validator
        .as_ref()
        .filter(|v| v.options().mode() == Mode::Strict)
}

fn set_text(slot: &Slot, value: &str) -> EditEvent {
    EditEvent::SetText {
        slot: slot.path.clone(),
        expect: slot.owner,
        value: value.to_string(),
    }
}

/// How strict validation closes a slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Closure {
    /// The validator refuses a term outside the vocabulary.
    Terms,
    /// The left operand is closed to the values the profile lists.
    Values,
}

/// Whether, and how, strict validation closes `slot`: its terms are the
/// vocabulary's and nothing else, so a combobox offers those and takes
/// nothing more. Open while the vocabulary is incomplete (the validator
/// would refuse nothing) and in advisory mode. A term slot is closed with or
/// without suggestions (without, there is no list, but a term the validator
/// refuses is still not sent); the values of an operand are listed, so they
/// need the suggestions.
fn closure(env: &Env, slot: &Slot) -> Option<Closure> {
    let v = strict(env)?;
    let cached = |key: &str| env.closed_cache.borrow().get(key).copied();
    match slot.kind {
        SlotKind::Action | SlotKind::LeftOperand | SlotKind::PolicyKind | SlotKind::Unit => {
            // What a policy's terms are read against is the policy's; a
            // unit's depends on its constraint's left operand.
            let key = match slot.kind {
                SlotKind::Unit => slot.path.to_string(),
                kind => format!("{}:{:?}", slot.path.node.policy, kind),
            };
            let closed = match cached(&key) {
                Some(c) => c,
                // Past the render's checks the slot is left open.
                None if !env.spend() => false,
                None => {
                    let closed = v.admissible(&env.doc, &set_text(slot, PROBE)).is_err();
                    env.closed_cache.borrow_mut().insert(key, closed);
                    closed
                }
            };
            closed.then_some(Closure::Terms)
        }
        SlotKind::Literal
            if env.cfg.suggestions && matches!(slot.path.field, Field::RightOperand(_)) =>
        {
            // Closed as far as the validator makes a value outside the list
            // an error: switched off, pinned out or lowered, it takes it.
            let binding = matches!(
                v.options().level_of(RuleId::ProseValueNotAllowed, true),
                Level::Error
            );
            let closed_values = binding
                && left_operand_term(env, slot)
                    .is_some_and(|t| t.hints.closed && !t.hints.value_tree.is_empty());
            closed_values.then_some(Closure::Values)
        }
        _ => None,
    }
}

/// As [`closure`] for a term slot at the moment its text is committed: one
/// ask, outside the render's budget. A slot rendered after the budget ran
/// out is open for the markers, but a commit must still not send a term the
/// validator refuses.
fn closure_at_commit(env: &Env, slot: &Slot) -> Option<Closure> {
    let v = strict(env)?;
    match slot.kind {
        SlotKind::Action | SlotKind::LeftOperand | SlotKind::PolicyKind | SlotKind::Unit => v
            .admissible(&env.doc, &set_text(slot, PROBE))
            .is_err()
            .then_some(Closure::Terms),
        _ => None,
    }
}

/// What a closed slot offers: the entries the validator would take. Asks it
/// about all of them in one call, which reads the document once.
fn allowed_options(
    env: &Env,
    slot: &Slot,
    closure: Closure,
    options: &[Suggestion],
) -> Vec<Suggestion> {
    match (closure, strict(env)) {
        (Closure::Terms, Some(v)) => {
            let values: Vec<&str> = options.iter().map(|o| o.value.as_ref()).collect();
            let verdicts = v.admissible_values(&env.doc, &slot.path, &values);
            options
                .iter()
                .zip(verdicts)
                .filter(|(_, verdict)| verdict.is_ok())
                .map(|(o, _)| o.clone())
                .collect()
        }
        _ => options.to_vec(),
    }
}

/// What typed text means in a closed slot: the written form of the entry it
/// names, a term the vocabulary has that the list does not offer, or, on
/// Enter, the only entry the text leaves. `None`: it names nothing.
fn resolve(
    env: &Env,
    slot: &Slot,
    closure: Closure,
    allowed: &[Suggestion],
    typed: &str,
    enter: bool,
) -> Option<String> {
    let w = &env.cfg.wording;
    let expand = |t: &str| match w.is_empty() {
        true => env.prefixes.expand(t),
        false => w.expand(&env.prefixes, t),
    };
    let same = |a: &str, b: &str| expand(a) == expand(b);
    if let Some(o) = closed::exact(allowed, typed, &same) {
        return Some(o.value.to_string());
    }
    if closure == Closure::Terms
        && strict(env).is_some_and(|v| v.admissible(&env.doc, &set_text(slot, typed)).is_ok())
    {
        return Some(typed.to_string());
    }
    if enter {
        return closed::only(allowed, typed).map(|o| o.value.to_string());
    }
    None
}

/// What commits a slot's text.
#[derive(Clone, Copy, PartialEq, Eq)]
enum How {
    Enter,
    Blur,
    /// An entry of the list was picked.
    Pick,
}

/// Whether the text now in a closed slot is one it does not take: the
/// validator refuses it, or it is not one of the values the profile lists.
fn outside(env: &Env, slot: &Slot, closure: Closure, options: &[Suggestion]) -> bool {
    if slot.raw.is_empty() {
        return false;
    }
    match closure {
        // Each ask reads the document: past the render's checks no note.
        Closure::Terms => strict(env).is_some_and(|v| {
            env.spend() && v.admissible(&env.doc, &set_text(slot, &slot.raw)).is_err()
        }),
        Closure::Values => {
            let w = &env.cfg.wording;
            let same = |a: &str, b: &str| w.expand(&env.prefixes, a) == w.expand(&env.prefixes, b);
            !options.iter().any(|o| same(&o.value, &slot.raw))
        }
    }
}

/// The suggestions for a slot, and the depth of each nested one (a value
/// under a broader suggested value), by value.
fn options_for(env: &Env, slot: &Slot) -> (Vec<Suggestion>, HashMap<AttrValue, usize>) {
    let v = &env.cfg.vocab;
    // A profile term is listed by its full IRI and written as the policy's
    // prefixes allow.
    let written = |list: &[Suggestion]| -> Vec<Suggestion> {
        list.iter()
            .map(|s| Suggestion {
                value: written_form(&s.value, &env.prefixes),
                ..s.clone()
            })
            .collect()
    };
    let flat = |list: Vec<Suggestion>| (list, HashMap::new());
    match slot.kind {
        SlotKind::Action => flat(written(&v.actions)),
        SlotKind::LeftOperand => flat(written(&v.left_operands)),
        SlotKind::PolicyKind => flat(written(&v.policy_kinds)),
        SlotKind::Party => flat(v.parties.clone()),
        SlotKind::Asset => flat(v.assets.clone()),
        SlotKind::PolicyRef => flat(
            env.policy_uids
                .iter()
                .filter(|(i, _)| *i != slot.path.node.policy)
                .map(|(_, uid)| Suggestion {
                    value: uid.clone(),
                    label: uid.clone(),
                    hint: None,
                    group: None,
                })
                .collect(),
        ),
        SlotKind::Literal if matches!(slot.path.field, Field::RightOperand(_)) => {
            profile_values(env, slot)
        }
        SlotKind::Unit => flat(
            left_operand_term(env, slot)
                .map(|t| {
                    t.hints
                        .units
                        .iter()
                        .map(|u| profile_suggestion(env, u, None))
                        .collect()
                })
                .unwrap_or_default(),
        ),
        _ => flat(vec![]),
    }
}

/// The loaded term the slot's constraint names as its left operand.
fn left_operand_term<'e>(env: &'e Env, slot: &Slot) -> Option<&'e Term> {
    let w = &env.cfg.wording;
    if w.is_empty() {
        return None;
    }
    let left = left_operand_at(&env.doc, &slot.path)?;
    w.lookup_written(&env.prefixes, left).map(|(t, _)| t)
}

/// A suggestion for the IRI `iri`: the loaded term's label and definition
/// when a profile declares it.
fn profile_suggestion(env: &Env, iri: &str, group: Option<&AttrValue>) -> Suggestion {
    let w = &env.cfg.wording;
    let term = w.lookup(iri).map(|(t, _)| t);
    let label = match term {
        Some(t) => t.display_label(w.languages()),
        None => wording::humanise(wording::local_name(iri)),
    };
    Suggestion {
        value: written_form(iri, &env.prefixes),
        label: label.into(),
        hint: term
            .and_then(|t| t.definition.pick(w.languages()))
            .map(|d| AttrValue::from(d.to_string())),
        group: group.cloned(),
    }
}

/// A right operand's suggestions: the values the profile suggests for the
/// left operand with their narrower values nested under them; else every
/// value the loaded profiles declare, under a heading that says so, unless
/// the left operand expects a literal datatype those IRIs cannot fit.
fn profile_values(env: &Env, slot: &Slot) -> (Vec<Suggestion>, HashMap<AttrValue, usize>) {
    const ANY_URI: &str = "http://www.w3.org/2001/XMLSchema#anyURI";
    let w = &env.cfg.wording;
    if w.is_empty() {
        return (vec![], HashMap::new());
    }
    let left = left_operand_term(env, slot);
    if let Some(t) = left
        && !t.hints.value_tree.is_empty()
    {
        let mut depth = HashMap::new();
        let list = t
            .hints
            .value_tree
            .iter()
            .map(|(iri, d)| {
                let s = profile_suggestion(env, iri, None);
                if *d > 0 {
                    depth.insert(s.value.clone(), *d);
                }
                s
            })
            .collect();
        return (list, depth);
    }
    if left
        .and_then(|t| t.hints.datatype.as_deref())
        .is_some_and(|d| d != ANY_URI)
    {
        return (vec![], HashMap::new());
    }
    let group = env.cfg.labels.profile_values.clone();
    let mut seen = HashSet::new();
    let list = w
        .profiles()
        .flat_map(|p| p.terms_of(TermKind::RightOperand))
        .filter(|t| !t.deprecated && seen.insert(t.iri.as_str()))
        .map(|t| profile_suggestion(env, &t.iri, Some(&group)))
        .collect();
    (list, HashMap::new())
}

/// Whether the right operand value at `path` is an IRI reference.
fn holds_iri(doc: &EditDoc, path: &SlotPath) -> bool {
    let Field::RightOperand(k) = path.field else {
        return false;
    };
    match doc.node(&path.node) {
        Some(NodeRef::Constraint(ConstraintNode::Atomic(a))) => {
            matches!(&a.right, RightOperand::Values(v) if matches!(v.get(k), Some(Literal::Iri(_))))
        }
        _ => false,
    }
}

fn placeholder(env: &Env, slot: &Slot) -> AttrValue {
    if slot.kind == SlotKind::Literal
        && let Some(dt) = left_operand_term(env, slot).and_then(|t| t.hints.datatype.as_deref())
        && let Some((_, p)) = env
            .cfg
            .labels
            .datatype_placeholders
            .iter()
            .find(|(k, _)| k == dt)
    {
        return p.clone();
    }
    placeholder_of(env, slot.kind)
}

fn placeholder_of(env: &Env, kind: SlotKind) -> AttrValue {
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
    // The list was asked for (Alt+ArrowDown) or kept after a rejection, not
    // narrowed by typing: a closed slot then lists what it takes even when
    // the text matches none of it.
    let browse = use_state(|| false);
    let active = use_state(|| 0usize);
    // A closed slot: why the text typed into it was not taken (it is still
    // there, to correct), and what a polite live region says about a draft
    // that was given up.
    let invalid = use_state(|| None::<AttrValue>);
    let announce = use_state(AttrValue::default);

    let use_suggestions = cfg.suggestions;
    let (options, depths) = if use_suggestions {
        options_for(&env, slot)
    } else {
        (vec![], HashMap::new())
    };
    let closed = closure(&env, slot);
    let not_in_profile = closed.is_some_and(|c| outside(&env, slot, c, &options));
    let note_id = format!("prose-note-{}", slot.path);
    let message_id = format!("prose-message-{}", slot.path);
    // The issues that describe the slot, then the note under it, then why
    // the text typed into it was not taken.
    let described = {
        let mut ids: Vec<String> = described_by.iter().map(|d| d.to_string()).collect();
        if not_in_profile {
            ids.push(note_id.clone());
        }
        if invalid.is_some() {
            ids.push(message_id.clone());
        }
        (!ids.is_empty()).then(|| AttrValue::from(ids.join(" ")))
    };
    // A closed slot lists every entry it takes, scrolling; an open one the
    // best few.
    let allowed;
    let (source, most): (&[Suggestion], usize) = match closed {
        Some(c) if *open => {
            allowed = allowed_options(&env, slot, c, &options);
            (&allowed, usize::MAX)
        }
        _ => (&options, cfg.max_suggestions),
    };
    let shown: Vec<Suggestion> = if *open {
        let mut found = filter_suggestions(source, &query, most);
        if found.is_empty() && *browse && closed.is_some() {
            found = filter_suggestions(source, "", most);
        }
        found.into_iter().cloned().collect()
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

    // A right operand's suggestions are profile values, which are IRIs. A
    // slot that already holds an IRI keeps it through SetText; into any
    // other (a string, a typed or new value) a pick is sent as SetIri.
    let pick_as_iri = slot.kind == SlotKind::Literal
        && matches!(slot.path.field, Field::RightOperand(_))
        && !holds_iri(&env.doc, &slot.path);

    let commit: Callback<How> = {
        let node_ref = node_ref.clone();
        let dirty = dirty.clone();
        let rev = rev.dispatcher();
        let onedit = env.onedit.clone();
        let path = slot.path.clone();
        let owner = slot.owner;
        let raw = slot.raw.clone();
        let env = env.clone();
        let slot = slot.clone();
        let options = closed.map(|_| options.clone());
        let invalid = invalid.clone();
        let announce = announce.clone();
        let open = open.clone();
        let browse = browse.clone();
        Callback::from(move |how: How| {
            if !*dirty.borrow() {
                return;
            }
            let text = node_ref
                .cast::<web_sys::Element>()
                .and_then(|e| e.text_content())
                .unwrap_or_default();
            let mut value = clean_edit(&text, &raw);
            let mut as_iri = false;
            let late: Vec<Suggestion>;
            let guard = match (closed, &options) {
                (Some(c), Some(o)) => Some((c, o)),
                _ => match closure_at_commit(&env, &slot) {
                    Some(c) => {
                        late = match use_suggestions {
                            true => options_for(&env, &slot).0,
                            false => vec![],
                        };
                        Some((c, &late))
                    }
                    None => None,
                },
            };
            if let Some((closure, options)) = guard
                && value != raw
                && !value.is_empty()
            {
                let allowed = allowed_options(&env, &slot, closure, options);
                match resolve(&env, &slot, closure, &allowed, &value, how == How::Enter) {
                    Some(v) => {
                        value = v;
                        as_iri = closure == Closure::Values && pick_as_iri;
                    }
                    None if how == How::Blur => {
                        // Given up: back to the value, and say so.
                        *dirty.borrow_mut() = false;
                        let labels = &env.cfg.labels;
                        announce.set(closed::kept_message(labels, &raw, &value).into());
                        invalid.set(None);
                        rev.dispatch(());
                        return;
                    }
                    None => {
                        // Not taken: the text stays, with the reason.
                        let labels = &env.cfg.labels;
                        let field = match closure {
                            Closure::Values => labels.fields.right_operand.to_string(),
                            Closure::Terms => labels.field_name(slot.path.field),
                        };
                        let near = closed::closest(&allowed, &value);
                        invalid.set(Some(
                            closed::not_allowed_message(
                                labels,
                                &value,
                                &field,
                                &closed::profile_names(&env.cfg.wording),
                                near,
                            )
                            .into(),
                        ));
                        browse.set(true);
                        open.set(use_suggestions);
                        return;
                    }
                }
            }
            *dirty.borrow_mut() = false;
            // The text taken is valid, whether or not it changes the value
            // (the slot's own value picked after a rejection changes nothing).
            if invalid.is_some() {
                invalid.set(None);
            }
            if value != raw {
                onedit.emit(if as_iri {
                    EditEvent::SetIri {
                        slot: path.clone(),
                        expect: owner,
                        value,
                    }
                } else {
                    EditEvent::SetText {
                        slot: path.clone(),
                        expect: owner,
                        value,
                    }
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
        let rev = rev.dispatcher();
        let onedit = env.onedit.clone();
        let path = slot.path.clone();
        let owner = slot.owner;
        Callback::from(move |value: AttrValue| {
            if let Some(el) = node_ref.cast::<web_sys::Element>() {
                el.set_text_content(Some(&value));
            }
            *refocus.borrow_mut() = true;
            open.set(false);
            if pick_as_iri {
                *dirty.borrow_mut() = false;
                onedit.emit(EditEvent::SetIri {
                    slot: path.clone(),
                    expect: owner,
                    value: value.to_string(),
                });
                rev.dispatch(());
                return;
            }
            *dirty.borrow_mut() = true;
            commit.emit(How::Pick);
        })
    };

    let oninput = {
        let dirty = dirty.clone();
        let navigated = navigated.clone();
        let node_ref = node_ref.clone();
        let query = query.clone();
        let open = open.clone();
        let active = active.clone();
        let invalid = invalid.clone();
        let browse = browse.clone();
        Callback::from(move |e: InputEvent| {
            *dirty.borrow_mut() = true;
            if *browse {
                browse.set(false);
            }
            *navigated.borrow_mut() = false;
            if invalid.is_some() {
                invalid.set(None);
            }
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
        let invalid = invalid.clone();
        let browse = browse.clone();
        let rejected = invalid.is_some();
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
                    // A closed slot that does not take the text opens the list
                    // again from the commit.
                    open.set(false);
                    commit.emit(How::Enter);
                }
                "Escape" => {
                    e.prevent_default();
                    if list_open && !rejected {
                        open.set(false);
                    } else {
                        if rejected {
                            invalid.set(None);
                            open.set(false);
                        }
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
                    browse.set(true);
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
        let announce = announce.clone();
        Callback::from(move |_: FocusEvent| {
            // The same message must be able to be announced again.
            if !announce.is_empty() {
                announce.set(AttrValue::default());
            }
            on_focus.emit(Some(focus.clone()))
        })
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
            commit.emit(How::Blur);
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
        let browse = browse.clone();
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
                    browse.set(false);
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

    // The highlight moves with the arrow keys through a list taller than its
    // box: keep it where it can be seen.
    {
        let list_id = list_id.clone();
        use_effect_with(
            (list_open, active_idx, shown.len()),
            move |(open, idx, _)| {
                if *open {
                    keep_in_view(&list_id, *idx);
                }
            },
        );
    }

    let mut class = cls!(cfg, slot, "prose-slot");
    // Inline styles are layered in the same order as the classes, so a
    // host's state style comes after (and beats) its plain `slot` style.
    let mut styles = vec![&cfg.styles.slot];
    if slot.raw.is_empty() {
        class.extend(cls!(cfg, slot_empty, "prose-slot-empty"));
        styles.push(&cfg.styles.slot_empty);
    }
    let severity = if invalid.is_some() {
        Some(Severity::Error)
    } else {
        *severity
    };
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
    let aria_invalid = matches!(severity, Some(Severity::Error)).then_some("true");
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

    let option = |i: usize| {
        let s = &shown[i];
        let accept = accept.clone();
        let value = s.value.clone();
        let onmousedown = Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            accept.emit(value.clone());
        });
        // The pointer highlights an option (the stylesheet does it for
        // `:hover`); the active descendant follows, so what is shown and
        // what is announced agree. Enter still commits the typed text
        // unless an arrow key was used.
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
        // A value narrower than a suggested one is indented by the
        // stylesheet, by depth.
        let depth = depths.get(&s.value).copied().unwrap_or(0);
        let depth_style = (depth > 0).then(|| AttrValue::from(format!("--prose-depth: {depth}")));
        if depth > 0 {
            class.push("prose-suggestion-nested");
            styles.push(&depth_style);
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
    };
    // Runs of options under one heading. Without any profile group the list
    // is flat, as it always was.
    let grouped = shown.iter().any(|s| s.group.is_some());
    let mut runs: Vec<(AttrValue, std::ops::Range<usize>)> = vec![];
    if grouped {
        for (i, s) in shown.iter().enumerate() {
            let heading = s
                .group
                .clone()
                .unwrap_or_else(|| cfg.labels.group_core.clone());
            match runs.last_mut() {
                Some((h, r)) if *h == heading => r.end = i + 1,
                _ => runs.push((heading, i..i + 1)),
            }
        }
    }

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
                    aria-placeholder={placeholder(&env, slot)}
                    aria-haspopup={use_suggestions.then_some("listbox")}
                    aria-label={label}
                    aria-invalid={aria_invalid}
                    aria-describedby={described}
                    aria-autocomplete={use_suggestions.then_some("list")}
                    aria-expanded={expanded}
                    aria-controls={controls}
                    aria-activedescendant={activedesc}
                    data-slot={slot.path.to_string()}
                    data-closed={closed.map(|_| "true")}
                    data-placeholder={placeholder(&env, slot)}
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
            if not_in_profile {
                <small id={note_id} class="prose-slot-note">{ cfg.labels.not_in_profile.clone() }</small>
            }
            if let Some(message) = &*invalid {
                <span
                    id={message_id}
                    class={classes!(
                        cls!(cfg, issue, "prose-issue"),
                        cls!(cfg, issue_error, "prose-issue-error"),
                        "prose-slot-message"
                    )}
                    style={merge_styles(&[&cfg.styles.issue, &cfg.styles.issue_error])}
                >
                    { message.clone() }
                </span>
            }
            if closed.is_some() {
                <span class={cls!(cfg, live, "prose-live")} role="status" aria-live="polite">
                    { (*announce).clone() }
                </span>
            }
            if list_open {
                <ul
                    id={list_id.clone()}
                    role="listbox"
                    class={cls!(cfg, suggestions, "prose-suggestions")}
                    style={cfg.styles.suggestions.clone()}
                >
                    if grouped {
                        { for runs.iter().enumerate().map(|(k, (heading, range))| html! {
                            <li role="group" class="prose-suggestion-group" aria-labelledby={format!("{list_id}-g{k}")}>
                                <span id={format!("{list_id}-g{k}")} class="prose-suggestion-group-label">{ heading.clone() }</span>
                                <ul role="none">{ for range.clone().map(&option) }</ul>
                            </li>
                        }) }
                    } else {
                        { for (0..shown.len()).map(&option) }
                    }
                </ul>
            }
        </span>
    }
}

/// Scroll the list `list_id` so that its option `index` is inside it. Only
/// the list scrolls, never the page.
fn keep_in_view(list_id: &str, index: usize) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let (Some(list), Some(option)) = (
        document.get_element_by_id(list_id),
        document.get_element_by_id(&format!("{list_id}-{index}")),
    ) else {
        return;
    };
    let (l, o) = (
        list.get_bounding_client_rect(),
        option.get_bounding_client_rect(),
    );
    if o.top() < l.top() {
        list.set_scroll_top(list.scroll_top() - (l.top() - o.top()).ceil() as i32);
    } else if o.bottom() > l.bottom() {
        list.set_scroll_top(list.scroll_top() + (o.bottom() - l.bottom()).ceil() as i32);
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
