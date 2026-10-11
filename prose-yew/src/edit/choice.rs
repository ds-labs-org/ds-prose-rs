//! Closed lists as `<select>`: operator, logical operator, conflict
//! strategy, and the kind of a rule.
use yew::prelude::*;

use prose_core::edit::{
    ChoiceKind, ChoiceSlot, EditEvent, Field, LogicalOp, RuleKindSlot, RuleList, Severity, SlotPath,
};
use prose_core::wording;

use prose_core::edit::EditDoc;
use prose_core::validate::{Mode, Validator};

use super::config::{Choice, cls, fill, written_form};
use super::{Env, EnvRef, FocusKind, SlotFocus};

struct Opt {
    value: String,
    label: String,
    /// Among the operators the profile suggests for the left operand.
    suggested: bool,
    /// The current value, which no list offers.
    extra: bool,
    /// Strict validation: the validator would not let this be written.
    blocked: bool,
}

impl Opt {
    fn new(value: String, label: String) -> Opt {
        Opt {
            value,
            label,
            suggested: false,
            extra: false,
            blocked: false,
        }
    }
}

/// The operators the loaded profiles suggest for `left` (`prose:operator`),
/// as expanded IRIs; empty without a profile or a hint.
fn hinted_operators(env: &Env, left: &str) -> Vec<String> {
    let w = &env.cfg.wording;
    if w.is_empty() || left.is_empty() {
        return Vec::new();
    }
    w.lookup_written(&env.prefixes, left)
        .map(|(t, _)| t.hints.operators.clone())
        .unwrap_or_default()
}

/// The validator of a strict configuration.
fn strict(env: &Env) -> Option<&Validator> {
    env.cfg
        .validator
        .as_ref()
        .filter(|v| v.options().mode() == Mode::Strict)
}

/// Whether the left operand is closed by its profile to certain operators:
/// the profile's operators are the only ones strict validation takes.
fn closes_operators(env: &Env, left: &str) -> bool {
    let w = &env.cfg.wording;
    if left.is_empty() || w.is_empty() {
        return false;
    }
    w.lookup_written(&env.prefixes, left)
        .is_some_and(|(t, _)| t.hints.closed && !t.hints.operators.is_empty())
}

/// Whether strict validation would refuse `value` written into the select.
fn is_blocked(v: &Validator, doc: &EditDoc, c: &ChoiceSlot, value: &str) -> bool {
    !value.is_empty()
        && v.admissible(
            doc,
            &EditEvent::SetChoice {
                slot: c.path.clone(),
                expect: c.owner,
                value: value.to_string(),
            },
        )
        .is_err()
}

/// The options of a choice, with the current raw value among them: a raw
/// value that is not in the list becomes an extra first option, selected.
fn options(env: &Env, c: &ChoiceSlot, severity: Option<Severity>) -> (Vec<Opt>, String) {
    let labels = &env.cfg.labels;
    let vocab = &env.cfg.vocab;
    let w = &env.cfg.wording;
    let mut opts: Vec<Opt> = match c.kind {
        ChoiceKind::Operator => {
            let hinted = hinted_operators(env, &c.left_operand);
            vocab
                .operators
                .iter()
                .map(|op| {
                    let op = written_form(op, &env.prefixes).to_string();
                    let phrase = if w.is_empty() {
                        wording::operator_for(&c.left_operand, &op)
                    } else {
                        wording::operator_with(w, &env.prefixes, &c.left_operand, &op)
                    };
                    let label = if env.cfg.show_raw_terms {
                        fill(
                            &labels.operator_option,
                            &[("phrase", &phrase), ("raw", &op)],
                        )
                    } else {
                        phrase
                    };
                    let suggested =
                        !hinted.is_empty() && hinted.contains(&w.expand(&env.prefixes, &op));
                    Opt {
                        suggested,
                        ..Opt::new(op, label)
                    }
                })
                .collect()
        }
        ChoiceKind::LogicalOp => LogicalOp::ALL
            .iter()
            .map(|o| Opt::new(o.as_str().to_string(), o.phrase().to_string()))
            .collect(),
        ChoiceKind::Conflict => vocab
            .conflicts
            .iter()
            .map(|Choice { value, label }| {
                Opt::new(
                    written_form(value, &env.prefixes).to_string(),
                    label.to_string(),
                )
            })
            .collect(),
        // A choice this version has no vocabulary for offers only its value.
        _ => Vec::new(),
    };
    // A compact IRI or full term for a listed one selects that option, unless
    // the host flagged the value as an error: then it is shown as it is, so
    // that choosing the listed option is a real change the browser reports.
    // The same IRI written another way (in full, or as a CURIE the policy or
    // the profile resolves) is that option; else a value with the option's
    // local name is.
    let expand = |v: &str| match w.is_empty() {
        true => env.prefixes.expand(v),
        false => w.expand(&env.prefixes, v),
    };
    let selected = opts
        .iter()
        .find(|o| o.value == c.raw)
        .or_else(|| {
            (!c.raw.is_empty() && severity != Some(Severity::Error))
                .then(|| {
                    let raw = expand(&c.raw);
                    opts.iter()
                        .find(|o| !o.value.is_empty() && expand(&o.value) == raw)
                        .or_else(|| {
                            opts.iter().find(|o| {
                                !o.value.is_empty() && o.value == wording::local_name(&c.raw)
                            })
                        })
                })
                .flatten()
        })
        .map(|o| o.value.clone());
    // Under strict validation, what the validator would refuse is listed but
    // cannot be picked.
    if let Some(v) = strict(env)
        && matches!(c.kind, ChoiceKind::Operator | ChoiceKind::Conflict)
    {
        for o in opts.iter_mut().filter(|o| !o.value.is_empty()) {
            o.blocked = is_blocked(v, &env.doc, c, &o.value);
        }
    }
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
                    extra: true,
                    ..Opt::new(c.raw.clone(), label)
                },
            );
            c.raw.clone()
        }
    };
    // The current value stays selected, and says it is not allowed.
    for o in opts
        .iter_mut()
        .filter(|o| o.blocked && !o.extra && o.value == selected)
    {
        o.label = fill(&labels.not_allowed_option, &[("label", &o.label)]);
    }
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
    let closed = c.kind == ChoiceKind::Operator
        && strict(&env).is_some()
        && closes_operators(&env, &c.left_operand);
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
            if closed {
                // The profile closes the operand: what it allows, and, still
                // listed, what it does not.
                { for opts.iter().filter(|o| o.extra).map(|o| option(o, &selected)) }
                <optgroup label={cfg.labels.allowed_group.clone()}>
                    { for opts.iter().filter(|o| !o.extra && !o.blocked).map(|o| option(o, &selected)) }
                </optgroup>
                if opts.iter().any(|o| !o.extra && o.blocked) {
                    <optgroup label={cfg.labels.not_allowed_group.clone()} disabled=true>
                        { for opts.iter().filter(|o| !o.extra && o.blocked).map(|o| option(o, &selected)) }
                    </optgroup>
                }
            } else if opts.iter().any(|o| o.suggested) {
                // The profile's suggestions first, never hiding the rest.
                { for opts.iter().filter(|o| o.extra).map(|o| option(o, &selected)) }
                <optgroup label={cfg.labels.suggested_operators.clone()}>
                    { for opts.iter().filter(|o| o.suggested).map(|o| option(o, &selected)) }
                </optgroup>
                // Only when something is left for it: an empty group is
                // announced as one.
                if opts.iter().any(|o| !o.suggested && !o.extra) {
                    <optgroup label={cfg.labels.other_operators.clone()}>
                        { for opts.iter().filter(|o| !o.suggested && !o.extra).map(|o| option(o, &selected)) }
                    </optgroup>
                }
            } else {
                { for opts.iter().map(|o| option(o, &selected)) }
            }
        </select>
    }
}

fn option(o: &Opt, selected: &str) -> Html {
    // A blocked option is disabled, unless it is the current value: a
    // disabled option that is selected still shows as the select's value.
    html! {
        <option value={o.value.clone()} selected={o.value == selected} disabled={o.blocked && o.value != selected}>
            { o.label.clone() }
        </option>
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
