//! Closed lists as `<select>`: operator, logical operator, conflict
//! strategy, and the kind of a rule.
use yew::prelude::*;

use prose_core::edit::{
    ChoiceKind, ChoiceSlot, EditEvent, Field, LogicalOp, RuleKindSlot, RuleList, Severity, SlotPath,
};
use prose_core::wording;

use super::config::{Choice, cls, fill};
use super::{Env, EnvRef, FocusKind, SlotFocus};

struct Opt {
    value: String,
    label: String,
}

/// The options of a choice, with the current raw value among them: a raw
/// value that is not in the list becomes an extra first option, selected.
fn options(env: &Env, c: &ChoiceSlot, severity: Option<Severity>) -> (Vec<Opt>, String) {
    let labels = &env.cfg.labels;
    let vocab = &env.cfg.vocab;
    let mut opts: Vec<Opt> = match c.kind {
        ChoiceKind::Operator => vocab
            .operators
            .iter()
            .map(|op| {
                let phrase = wording::operator_for(&c.left_operand, op);
                Opt {
                    value: op.to_string(),
                    label: if env.cfg.show_raw_terms {
                        fill(&labels.operator_option, &[("phrase", &phrase), ("raw", op)])
                    } else {
                        phrase
                    },
                }
            })
            .collect(),
        ChoiceKind::LogicalOp => LogicalOp::ALL
            .iter()
            .map(|o| Opt {
                value: o.as_str().to_string(),
                label: o.phrase().to_string(),
            })
            .collect(),
        ChoiceKind::Conflict => vocab
            .conflicts
            .iter()
            .map(|Choice { value, label }| Opt {
                value: value.to_string(),
                label: label.to_string(),
            })
            .collect(),
        // A choice this version has no vocabulary for offers only its value.
        _ => Vec::new(),
    };
    // A compact IRI or full term for a listed one selects that option, unless
    // the host flagged the value as an error: then it is shown as it is, so
    // that choosing the listed option is a real change the browser reports.
    let selected = opts
        .iter()
        .find(|o| o.value == c.raw)
        .or_else(|| {
            (!c.raw.is_empty() && severity != Some(Severity::Error))
                .then(|| {
                    opts.iter()
                        .find(|o| !o.value.is_empty() && o.value == wording::local_name(&c.raw))
                })
                .flatten()
        })
        .map(|o| o.value.clone());
    // An operator the reducer would refuse is not offered, unless the host
    // translates the switch itself.
    if c.kind == ChoiceKind::Operator && !env.cfg.host_plans_operator_switches {
        let rules = &env.cfg.rules;
        opts.retain(|o| {
            Some(&o.value) == selected.as_ref()
                || rules.operator_switch_allowed(&c.raw, &o.value, c.values)
        });
    }
    let selected = match selected {
        Some(v) => v,
        None => {
            let label = if c.raw.is_empty() {
                match c.kind {
                    ChoiceKind::Operator => labels.fields.operator.to_string(),
                    ChoiceKind::LogicalOp => labels.fields.logical_op.to_string(),
                    ChoiceKind::Conflict => labels.fields.conflict.to_string(),
                    _ => String::new(),
                }
            } else {
                fill(&labels.unknown_option, &[("raw", &c.raw)])
            };
            opts.insert(
                0,
                Opt {
                    value: c.raw.clone(),
                    label,
                },
            );
            c.raw.clone()
        }
    };
    (opts, selected)
}

#[derive(Properties, PartialEq, Clone)]
pub(crate) struct ChoiceSelectProps {
    pub slot: ChoiceSlot,
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
    fn reduce(self: std::rc::Rc<Self>, _: ()) -> std::rc::Rc<Self> {
        std::rc::Rc::new(Rev(self.0.wrapping_add(1)))
    }
}

/// After every render, make the DOM select show `selected`. A browser keeps
/// the user's pick in the element when the host refuses the edit; this puts
/// it back without replacing the element (which would lose focus).
#[hook]
fn use_select_in_step(node_ref: &NodeRef, selected: String) {
    let node_ref = node_ref.clone();
    use_effect(move || {
        if let Some(select) = node_ref.cast::<web_sys::HtmlSelectElement>()
            && select.value() != selected
        {
            select.set_value(&selected);
        }
        || ()
    });
}

#[function_component(ChoiceSelect)]
pub(crate) fn choice_select(props: &ChoiceSelectProps) -> Html {
    let env = props.env.0.clone();
    let cfg = env.cfg.clone();
    let c = &props.slot;
    let rev = use_reducer(|| Rev(0));
    let (opts, selected) = options(&env, c, props.severity);
    let node_ref = use_node_ref();
    use_select_in_step(&node_ref, selected.clone());

    let onchange = {
        let onedit = env.onedit.clone();
        let slot = c.path.clone();
        let owner = c.owner;
        let raw = c.raw.clone();
        let rev = rev.dispatcher();
        Callback::from(move |e: Event| {
            use wasm_bindgen::JsCast;
            let value = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok())
                .map(|s| s.value());
            if let Some(value) = value
                && value != raw
            {
                onedit.emit(EditEvent::SetChoice {
                    slot: slot.clone(),
                    expect: owner,
                    value,
                });
            }
            // Render again so the select shows what the document says,
            // whatever the host decided. The element is not replaced: a
            // keyboard user stepping through the options keeps focus.
            rev.dispatch(());
        })
    };
    let onfocusin = focus_callback(
        &env,
        c.path.clone(),
        FocusKind::Choice(c.kind),
        c.raw.clone(),
    );
    let onfocusout = {
        let on_focus = env.on_focus_slot.clone();
        Callback::from(move |_: FocusEvent| on_focus.emit(None))
    };
    let invalid = matches!(props.severity, Some(Severity::Error)).then_some("true");
    html! {
        <select
            key={format!("{}:{:?}", c.owner.0, c.path.field)}
            ref={node_ref}
            class={cls!(cfg, select, "prose-select")}
            style={cfg.styles.select.clone()}
            data-slot={c.path.to_string()}
            aria-label={cfg.labels.slot_name(&c.path)}
            aria-invalid={invalid}
            aria-describedby={props.described_by.clone()}
            title={c.display.clone()}
            onchange={onchange}
            onfocusin={onfocusin}
            onfocusout={onfocusout}
        >
            { for opts.iter().map(|o| html! {
                <option value={o.value.clone()} selected={o.value == selected}>{ o.label.clone() }</option>
            }) }
        </select>
    }
}

fn focus_callback(env: &Env, slot: SlotPath, kind: FocusKind, raw: String) -> Callback<FocusEvent> {
    let on_focus = env.on_focus_slot.clone();
    Callback::from(move |_: FocusEvent| {
        on_focus.emit(Some(SlotFocus {
            slot: slot.clone(),
            kind,
            raw: raw.clone(),
        }))
    })
}

#[derive(Properties, PartialEq, Clone)]
pub(crate) struct RuleKindSelectProps {
    pub slot: RuleKindSlot,
    pub env: EnvRef,
    /// The kinds to offer; the rule's own is always among them.
    pub kinds: Vec<RuleList>,
}

#[function_component(RuleKindSelect)]
pub(crate) fn rule_kind_select(props: &RuleKindSelectProps) -> Html {
    let env = props.env.0.clone();
    let cfg = env.cfg.clone();
    let k = &props.slot;
    let rev = use_reducer(|| Rev(0));
    let node_ref = use_node_ref();
    let current = match k.current {
        RuleList::Permission | RuleList::Prohibition | RuleList::Obligation => k.current,
        _ => RuleList::Obligation,
    };
    use_select_in_step(&node_ref, current.as_str().to_string());
    let kinds: Vec<(RuleList, AttrValue)> = [
        (RuleList::Permission, cfg.labels.may.clone()),
        (RuleList::Prohibition, cfg.labels.must_not.clone()),
        (RuleList::Obligation, cfg.labels.must.clone()),
    ]
    .into_iter()
    .filter(|(kind, _)| *kind == current || props.kinds.contains(kind))
    .collect();
    let onchange = {
        let onedit = env.onedit.clone();
        let rule = k.rule.clone();
        let owner = k.owner;
        let rev = rev.dispatcher();
        Callback::from(move |e: Event| {
            use wasm_bindgen::JsCast;
            let to = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok())
                .and_then(|s| RuleList::parse(&s.value()));
            if let Some(to) = to
                && to != current
            {
                onedit.emit(EditEvent::ChangeRuleKind {
                    rule: rule.clone(),
                    expect: owner,
                    to,
                });
            }
            rev.dispatch(());
        })
    };
    let slot_path = SlotPath {
        node: k.rule.clone(),
        field: Field::Kind,
    };
    let onfocusin = focus_callback(
        &env,
        slot_path,
        FocusKind::RuleKind,
        current.as_str().to_string(),
    );
    let onfocusout = {
        let on_focus = env.on_focus_slot.clone();
        Callback::from(move |_: FocusEvent| on_focus.emit(None))
    };
    html! {
        <select
            key={format!("{}:kind", k.owner.0)}
            ref={node_ref}
            class={cls!(cfg, select, "prose-select", "prose-rule-kind")}
            style={cfg.styles.select.clone()}
            data-node={k.rule.to_string()}
            aria-label={fill(&cfg.labels.rule_kind_aria, &[("owner", &cfg.labels.describe(&k.rule))])}
            onchange={onchange}
            onfocusin={onfocusin}
            onfocusout={onfocusout}
        >
            { for kinds.iter().map(|(kind, label)| html! {
                <option value={kind.as_str()} selected={*kind == current}>{ label.clone() }</option>
            }) }
        </select>
    }
}
