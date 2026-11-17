//! `TermSelect`: a term slot as a `<select>` of the terms the vocabulary
//! and the loaded profiles define, for [`TermInput::Select`].
//!
//! The options are the ones the free-text slot suggests (see
//! `slot::options_for`), every one of them rather than the best few,
//! grouped by profile, and filtered by strict validation when it is on.
//! Deprecated terms are hidden when the validator makes them an error, and
//! otherwise listed last under `Labels::deprecated`. A stored value that is
//! none of the options is kept as an extra first option, selected, with the
//! `not_in_profile` note (`not_in_list` without a profile) under the select: it is never dropped, and the
//! validation report keeps reporting it.
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use yew::prelude::*;

use prose_core::edit::{EditEvent, Field, Severity, Slot, SlotKind};
use prose_core::profile::TermKind;
use prose_core::validate::{Level, RuleId};

use super::choice::use_select_in_step;
use super::config::{Suggestion, TermInput, cls, merge_styles};
use super::slot::{
    Closure, allowed_options, holds_iri, left_operand_hints, options_for, placeholder,
    profile_suggestion, set_text, strict,
};
use super::{Env, EnvRef, FocusKind, SlotFocus};

/// Whether `slot` is rendered as a [`TermSelect`]: the configuration asks
/// for selects, and the slot holds a term. A unit and a right operand only
/// when the profile closes the constraint's left operand (`prose:closed`)
/// and lists units, or values, for it: an open left operand's units and
/// values are suggestions, so the slot stays free text with them. Any other
/// right operand is a literal (a date, a number, an identifier) and stays
/// free text.
pub(crate) fn takes_select(env: &Env, slot: &Slot) -> bool {
    if env.cfg.term_input != TermInput::Select {
        return false;
    }
    match slot.kind {
        SlotKind::Action | SlotKind::LeftOperand | SlotKind::PolicyKind => true,
        SlotKind::Unit => {
            left_operand_hints(env, slot).is_some_and(|h| h.closed && !h.units.is_empty())
        }
        SlotKind::Literal if matches!(slot.path.field, Field::RightOperand(_)) => {
            left_operand_hints(env, slot).is_some_and(|h| h.closed && !h.value_tree.is_empty())
        }
        _ => false,
    }
}

/// Whether the validator reads the slot's text as a term (and so can be
/// asked which terms it takes).
fn is_term(kind: SlotKind) -> bool {
    matches!(
        kind,
        SlotKind::Action | SlotKind::LeftOperand | SlotKind::PolicyKind | SlotKind::Unit
    )
}

/// The profile kind of the terms a slot lists, for its deprecated ones.
fn term_kind(kind: SlotKind) -> Option<TermKind> {
    match kind {
        SlotKind::Action => Some(TermKind::Action),
        SlotKind::LeftOperand => Some(TermKind::LeftOperand),
        SlotKind::PolicyKind => Some(TermKind::PolicyClass),
        _ => None,
    }
}

/// What the select lists: the current terms (with the depth of a nested
/// value), then the deprecated ones.
struct Listing {
    current: Vec<Suggestion>,
    depths: HashMap<AttrValue, usize>,
    deprecated: Vec<Suggestion>,
}

fn listing(env: &Env, slot: &Slot) -> Listing {
    let w = &env.cfg.wording;
    let (mut list, tree) = options_for(env, slot);
    // A unit or a value may itself be deprecated in its profile.
    let is_deprecated = |value: &str| {
        !w.is_empty()
            && w.lookup_written(&env.prefixes, value)
                .is_some_and(|(t, _)| t.deprecated)
    };
    let mut deprecated: Vec<Suggestion> = vec![];
    // A value nested under one that moves to the deprecated group is
    // indented by its listed ancestors only, so none hangs under a parent
    // that is not shown above it.
    let mut depths = HashMap::new();
    let mut ancestors: Vec<(usize, bool)> = vec![];
    list.retain(|s| {
        let dep = is_deprecated(&s.value);
        let d = tree.get(&s.value).copied().unwrap_or(0);
        while ancestors.last().is_some_and(|(a, _)| *a >= d) {
            ancestors.pop();
        }
        let shown = ancestors.iter().filter(|(_, kept)| *kept).count();
        ancestors.push((d, !dep));
        if dep {
            deprecated.push(s.clone());
        } else if shown > 0 {
            depths.insert(s.value.clone(), shown);
        }
        !dep
    });
    // The vocabulary leaves the deprecated actions, left operands and
    // policy classes out: they come from the profiles.
    if let Some(kind) = term_kind(slot.kind) {
        for p in w.profiles() {
            for t in p.terms_of(kind).filter(|t| t.deprecated) {
                deprecated.push(profile_suggestion(env, &t.iri, None));
            }
        }
    }
    let mut seen: HashSet<AttrValue> = list.iter().map(|s| s.value.clone()).collect();
    deprecated.retain(|s| seen.insert(s.value.clone()));
    let hide_deprecated = strict(env).is_some_and(|v| {
        v.options().level_of(RuleId::ProfileDeprecatedTerm, false) == Level::Error
    });
    if hide_deprecated {
        deprecated.clear();
    }
    // Under strict validation, only what the validator takes: one ask for
    // both lists, which reads the document once, and within the render's
    // checks. Past them the list is left unfiltered; what a pick sends is
    // still checked.
    if is_term(slot.kind) && strict(env).is_some() && env.spend() {
        let both: Vec<Suggestion> = list.iter().chain(&deprecated).cloned().collect();
        let taken: HashSet<AttrValue> = allowed_options(env, slot, Closure::Terms, &both)
            .into_iter()
            .map(|s| s.value)
            .collect();
        list.retain(|s| taken.contains(&s.value));
        deprecated.retain(|s| taken.contains(&s.value));
    }
    Listing {
        current: list,
        depths,
        deprecated,
    }
}

/// The option whose value is the slot's text: the same text, else the same
/// IRI written another way (a CURIE, a bare ODRL term, in full).
fn selected_value(env: &Env, slot: &Slot, options: &[&Suggestion]) -> Option<AttrValue> {
    if let Some(o) = options.iter().find(|o| o.value == slot.raw) {
        return Some(o.value.clone());
    }
    if slot.raw.is_empty() {
        return None;
    }
    let w = &env.cfg.wording;
    let expand = |v: &str| match w.is_empty() {
        true => env.prefixes.expand(v),
        false => w.expand(&env.prefixes, v),
    };
    let raw = expand(&slot.raw);
    options
        .iter()
        .find(|o| expand(&o.value) == raw)
        .map(|o| o.value.clone())
}

#[derive(Properties, PartialEq, Clone)]
pub(crate) struct TermSelectProps {
    pub slot: Slot,
    pub env: EnvRef,
    #[prop_or_default]
    pub severity: Option<Severity>,
    #[prop_or_default]
    pub described_by: Option<AttrValue>,
}

#[derive(Clone, PartialEq)]
struct Rev(u32);

impl Reducible for Rev {
    type Action = ();
    fn reduce(self: Rc<Self>, _: ()) -> Rc<Self> {
        Rc::new(Rev(self.0.wrapping_add(1)))
    }
}

#[function_component(TermSelect)]
pub(crate) fn term_select(props: &TermSelectProps) -> Html {
    let env: Rc<Env> = props.env.0.clone();
    let cfg = env.cfg.clone();
    let slot = &props.slot;
    let rev = use_reducer(|| Rev(0));
    let node_ref = use_node_ref();

    let Listing {
        current,
        depths,
        deprecated,
    } = listing(&env, slot);
    let all: Vec<&Suggestion> = current.iter().chain(&deprecated).collect();
    let found = selected_value(&env, slot, &all);
    // The slot's text when no option is it: an extra first option, so the
    // value is shown and kept, never replaced by the first entry.
    let outside = found.is_none() && !slot.raw.is_empty();
    let selected: AttrValue = found.unwrap_or_else(|| slot.raw.clone().into());
    use_select_in_step(&node_ref, selected.to_string());
    let extra = (!all.iter().any(|o| o.value == selected)).then(|| {
        let label = if slot.raw.is_empty() {
            placeholder(&env, slot).to_string()
        } else {
            // The note under the select says why; the option is the value.
            slot.raw.to_string()
        };
        (selected.clone(), label)
    });

    // A right operand's values are IRIs: into a slot that holds a string or
    // a typed value, a pick is written as an IRI (as the suggestion list of
    // the free-text slot does).
    let pick_as_iri = slot.kind == SlotKind::Literal
        && matches!(slot.path.field, Field::RightOperand(_))
        && !holds_iri(&env.doc, &slot.path);

    // Every value an option offers, for what a change may send.
    let listed: HashSet<AttrValue> = all.iter().map(|o| o.value.clone()).collect();
    let onchange = {
        let onedit = env.onedit.clone();
        let rev = rev.dispatcher();
        let env = env.clone();
        let slot = slot.clone();
        Callback::from(move |e: Event| {
            use wasm_bindgen::JsCast;
            let value = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok())
                .map(|s| s.value());
            // Only what is listed can be picked, but a script can set any
            // value: it must be an option, and one the validator takes,
            // before anything is sent.
            let refused = |v: &str| {
                !listed.contains(v)
                    || (is_term(slot.kind)
                        && strict(&env).is_some_and(|val| {
                            val.admissible(&env.doc, &set_text(&slot, v)).is_err()
                        }))
            };
            if let Some(value) = value
                && value != slot.raw
                && !value.is_empty()
                && !refused(&value)
            {
                onedit.emit(if pick_as_iri {
                    EditEvent::SetIri {
                        slot: slot.path.clone(),
                        expect: slot.owner,
                        value,
                    }
                } else {
                    EditEvent::SetText {
                        slot: slot.path.clone(),
                        expect: slot.owner,
                        value,
                    }
                });
            }
            // Render again so the select shows what the document says,
            // whatever the host decided.
            rev.dispatch(());
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
        Callback::from(move |_: FocusEvent| on_focus.emit(None))
    };

    let note_id = format!("prose-note-{}", slot.path);
    let described = {
        let mut ids: Vec<String> = props.described_by.iter().map(|d| d.to_string()).collect();
        if outside {
            ids.push(note_id.clone());
        }
        (!ids.is_empty()).then(|| AttrValue::from(ids.join(" ")))
    };
    let invalid = matches!(props.severity, Some(Severity::Error)).then_some("true");
    // The state classes and styles of a slot, so an error and a warning
    // show on a select as they do on free text.
    let mut class = cls!(cfg, select, "prose-select", "prose-term-select");
    let mut styles = vec![&cfg.styles.select];
    match props.severity {
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
    let style = merge_styles(&styles);
    let note = if cfg.wording.is_empty() {
        cfg.labels.not_in_list.clone()
    } else {
        cfg.labels.not_in_profile.clone()
    };

    let option = |s: &Suggestion| {
        // A value narrower than a listed one is indented under it.
        let depth = depths.get(&s.value).copied().unwrap_or(0);
        let label = format!("{}{}", "\u{a0}\u{a0}\u{a0}".repeat(depth), s.label);
        html! {
            <option value={s.value.clone()} selected={s.value == selected} title={s.hint.clone()}>
                { label }
            </option>
        }
    };
    // Runs of options under one heading, as the suggestion list groups
    // them. Without any profile group the list is flat.
    let grouped = current.iter().any(|s| s.group.is_some());
    let mut runs: Vec<(AttrValue, std::ops::Range<usize>)> = vec![];
    if grouped {
        for (i, s) in current.iter().enumerate() {
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
            <select
                key={format!("{}:{:?}", slot.owner.0, slot.path.field)}
                ref={node_ref}
                class={class}
                style={style}
                data-slot={slot.path.to_string()}
                data-outside={outside.then_some("true")}
                aria-label={cfg.labels.slot_name(&slot.path)}
                aria-invalid={invalid}
                aria-describedby={described}
                title={super::view::slot_title(&env, &slot.display, &slot.raw)}
                onchange={onchange}
                onfocusin={onfocusin}
                onfocusout={onfocusout}
            >
                if let Some((value, label)) = &extra {
                    <option value={value.clone()} selected=true>{ label.clone() }</option>
                }
                if grouped {
                    { for runs.iter().map(|(heading, range)| html! {
                        <optgroup label={heading.clone()}>
                            { for current[range.clone()].iter().map(option) }
                        </optgroup>
                    }) }
                } else {
                    { for current.iter().map(option) }
                }
                if !deprecated.is_empty() {
                    <optgroup label={cfg.labels.deprecated.clone()}>
                        { for deprecated.iter().map(option) }
                    </optgroup>
                }
            </select>
            if outside {
                <small id={note_id} class="prose-slot-note">{ note }</small>
            }
        </span>
    }
}
