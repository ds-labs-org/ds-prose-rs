//! `<OdrlProse>`: renders ODRL JSON-LD as plain-language prose. All reading
//! lives in [`prose_core`]; this crate only lays a [`prose_core::Document`]
//! out as semantic HTML. The host supplies styling through the `prose-*`
//! class names below (or `class` on the root).
//!
//! Classes: `ds-prose` (root), `prose-policy`, `prose-rules`, `prose-rule`
//! plus `prose-permission` / `prose-prohibition` / `prose-obligation`,
//! `prose-rule-sentence`, `prose-label`, `prose-notes`, `prose-warnings`,
//! `prose-error`.
use prose_core::{Condition, Document, Policy, ProseError, Rule, RuleKind};
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct OdrlProseProps {
    /// The ODRL policy as JSON-LD text.
    pub json: AttrValue,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(OdrlProse)]
pub fn odrl_prose(props: &OdrlProseProps) -> Html {
    let parsed = use_memo(props.json.clone(), |json| prose_core::render(json));
    let class = classes!("ds-prose", props.class.clone());
    match &*parsed {
        Ok(doc) => html! { <article class={class}>{ document(doc) }</article> },
        Err(e) => html! { <article class={class}>{ error(e) }</article> },
    }
}

fn error(e: &ProseError) -> Html {
    html! { <p class="prose-error" role="alert">{ e.to_string() }</p> }
}

fn document(doc: &Document) -> Html {
    html! {
        <>
            { for doc.policies.iter().map(policy) }
            if !doc.warnings.is_empty() {
                <aside class="prose-warnings">
                    <p class="prose-label">{ "Not rendered" }</p>
                    <ul>{ for doc.warnings.iter().map(|w| html! { <li>{ w }</li> }) }</ul>
                </aside>
            }
        </>
    }
}

fn policy(p: &Policy) -> Html {
    html! {
        <section class="prose-policy">
            <h3>{ &p.heading }</h3>
            <p>{ &p.intro }</p>
            if !p.notes.is_empty() {
                <ul class="prose-notes">{ for p.notes.iter().map(|n| html! { <li>{ n }</li> }) }</ul>
            }
            if !p.rules.is_empty() {
                <ol class="prose-rules">{ for p.rules.iter().map(rule) }</ol>
            }
        </section>
    }
}

fn rule(r: &Rule) -> Html {
    let kind = match r.kind {
        RuleKind::Permission => "prose-permission",
        RuleKind::Prohibition => "prose-prohibition",
        RuleKind::Obligation => "prose-obligation",
    };
    html! {
        <li class={classes!("prose-rule", kind)}>
            <p class="prose-rule-sentence">{ &r.sentence }</p>
            { conditions("Limits on the action", &r.refinements) }
            { conditions("Applies only if", &r.conditions) }
            { for r.follow_ups.iter().map(|f| html! {
                <>
                    <p class="prose-label">{ format!("{}:", f.label) }</p>
                    <ul>{ for f.rules.iter().map(rule) }</ul>
                </>
            }) }
        </li>
    }
}

fn conditions(label: &str, list: &[Condition]) -> Html {
    if list.is_empty() {
        return html! {};
    }
    html! {
        <>
            <p class="prose-label">{ format!("{label}:") }</p>
            { condition_list(list) }
        </>
    }
}

fn condition_list(list: &[Condition]) -> Html {
    html! {
        <ul>
            { for list.iter().map(|c| html! {
                <li>
                    { &c.text }
                    if !c.children.is_empty() { { ":" } { condition_list(&c.children) } }
                </li>
            }) }
        </ul>
    }
}
