//! `<OdrlProse>`: renders ODRL JSON-LD as plain-language prose. All reading
//! lives in [`prose_core`]; this crate only lays a [`prose_core::Document`]
//! out as semantic HTML. The host supplies styling through the `prose-*`
//! class names below (or `class` on the root).
//!
//! Classes: `ds-prose` (root), `prose-policy`, `prose-rules`, `prose-rule`
//! plus `prose-permission` / `prose-prohibition` / `prose-obligation`,
//! `prose-rule-sentence`, `prose-label`, `prose-notes`, `prose-warnings`,
//! `prose-error`, and in the glossary of profile terms ([`ProseGlossary`])
//! `prose-glossary`, `prose-note`, `prose-kind-of`, `prose-implies`,
//! `prose-deprecated`, `prose-from`.
//!
//! A profile reaches the components as a prop: [`OdrlProseProps::wording`]
//! (a [`SharedWording`]) for `<OdrlProse>`, [`EditConfig::with_wording`] for
//! [`OdrlProseView`]. Without one, both render exactly as before 0.4.
//!
//! [`OdrlProseView`] is the editing counterpart: the same sentences, with an
//! Edit mode (see [`edit`]).
pub mod edit;
use edit::fill;
pub use edit::{
    BASE_CSS, ButtonContent, Choice, Decoration, DecorationAt, EditConfig, EditMode, EmptyTexts,
    FieldNames, FocusKind, Labels, Nouns, OdrlProseView, OdrlProseViewProps, Placeholders,
    ProseClasses, ProseEditor, ProseEditorWith, ProseProblems, ProseProblemsProps, ProseStyles,
    Reveal, SlotFocus, StrictEdit, Suggestion, Vocabulary, filter_suggestions, use_prose_editor,
    use_prose_editor_with, use_validation,
};
/// The core crate, so hosts can reach `prose_yew::prose_core::edit::*`.
pub use prose_core;
use prose_core::profile::Wording;
use prose_core::validate::{Report, Validator};
use prose_core::{Condition, Document, Policy, PolicyGlossary, ProseError, Rule, RuleKind};
use std::rc::Rc;
use yew::prelude::*;

/// A [`Wording`] shared by components. Compared by identity, so a component
/// renders again only when the host hands it another one.
#[derive(Clone, Debug)]
pub struct SharedWording(pub Rc<Wording>);

impl PartialEq for SharedWording {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl From<Wording> for SharedWording {
    fn from(w: Wording) -> Self {
        SharedWording(Rc::new(w))
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct OdrlProseProps {
    /// The ODRL policy as JSON-LD text.
    pub json: AttrValue,
    #[prop_or_default]
    pub class: Classes,
    /// The loaded profiles. `None` reads as before.
    #[prop_or_default]
    pub wording: Option<SharedWording>,
    /// A glossary of the profile terms after each policy. Only shown with
    /// a profile that explains a term the policy uses.
    #[prop_or(true)]
    pub glossary: bool,
    /// Validates the policy against `validator`'s profiles. Nothing is shown
    /// for it unless `violations` is set.
    #[prop_or_default]
    pub validator: Option<Validator>,
    /// Add a section after the policies with the validator's findings and
    /// its verdict. Needs a `validator`. Without it, the rendering is as
    /// before.
    #[prop_or(false)]
    pub violations: bool,
}

#[derive(Properties, PartialEq, Clone)]
pub struct GlossaryProps {
    pub glossary: PolicyGlossary,
    #[prop_or_default]
    pub labels: Rc<Labels>,
}

/// The profile terms one policy uses, as a `<details class="prose-glossary">`
/// with a `<dl>`. Nothing when the glossary is empty.
#[function_component(ProseGlossary)]
pub fn prose_glossary(props: &GlossaryProps) -> Html {
    let terms = &props.glossary.terms;
    if terms.is_empty() {
        return Html::default();
    }
    let l = &props.labels;
    let line = |class: &'static str, template: &str, key: &str, value: &str| {
        html! { <dd class={class}>{ fill(template, &[(key, value)]) }</dd> }
    };
    let labels = |list: &[(String, String)]| {
        list.iter()
            .map(|(_, label)| label.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    html! {
        <details class="prose-glossary" open=true>
            <summary>{ l.glossary_heading.clone() }</summary>
            <dl>
                { for terms.iter().take(GLOSSARY_MAX).map(|g| html! {
                    <>
                        <dt><span>{ g.label.clone() }</span>{ " " }<code>{ g.written.clone() }</code></dt>
                        if let Some(d) = &g.definition { <dd>{ d.clone() }</dd> }
                        if let Some(n) = &g.note { { line("prose-note", &l.usage_note, "note", n) } }
                        if !g.broader.is_empty() {
                            { line("prose-kind-of", &l.kind_of, "terms", &labels(&g.broader)) }
                        }
                        if !g.implies.is_empty() {
                            { line("prose-implies", &l.implies, "terms", &labels(&g.implies)) }
                        }
                        if g.deprecated {
                            <dd class="prose-deprecated">{ l.deprecated.clone() }</dd>
                        }
                        if let Some(p) = &g.profile {
                            { line("prose-from", &l.from_profile, "profile", p) }
                        }
                    </>
                }) }
            </dl>
        </details>
    }
}

/// At most this many glossary entries per policy.
const GLOSSARY_MAX: usize = 30;

#[function_component(OdrlProse)]
pub fn odrl_prose(props: &OdrlProseProps) -> Html {
    let parsed = use_memo(
        (props.json.clone(), props.wording.clone()),
        |(json, w)| match w {
            Some(w) => prose_core::render_with(json, &w.0),
            None => prose_core::render_with(json, &Wording::default()),
        },
    );
    let report = use_memo(
        (
            props.json.clone(),
            props.validator.clone(),
            props.violations,
        ),
        |(json, validator, violations)| match validator {
            Some(v) if *violations => Some(v.validate_json(json)),
            _ => None,
        },
    );
    let class = classes!("ds-prose", props.class.clone());
    match &*parsed {
        Ok(r) => {
            let glossary = if props.glossary { &r.glossary[..] } else { &[] };
            html! {
                <article class={class}>
                    { document(&r.document, glossary) }
                    if let Some(report) = &*report { { violations_section(report) } }
                </article>
            }
        }
        Err(e) => html! { <article class={class}>{ error(e) }</article> },
    }
}

/// The findings of a validation, after the policies. Plain markup: a
/// status line and, when there is something to say, a list.
fn violations_section(report: &Report) -> Html {
    let labels = Labels::default();
    let status = edit::status(report, &labels);
    let findings = &report.findings;
    html! {
        <section class="prose-violations">
            if let Some((kind, text)) = status {
                <p class={classes!("prose-violations-status", kind)} role="status">{ text }</p>
            }
            if !findings.is_empty() {
                <ul class="prose-violations-list">
                    { for findings.iter().map(|f| {
                        let severity = match f.severity {
                            prose_core::edit::Severity::Error => "prose-violation-error",
                            prose_core::edit::Severity::Warning => "prose-violation-warning",
                            prose_core::edit::Severity::Info => "prose-violation-info",
                        };
                        html! {
                            <li class={classes!("prose-violation", severity)}>
                                <code class="prose-rule-id">{ f.rule.as_str() }</code>{ " " }
                                { f.message.clone() }
                            </li>
                        }
                    }) }
                </ul>
            }
        </section>
    }
}

fn error(e: &ProseError) -> Html {
    html! { <p class="prose-error" role="alert">{ e.to_string() }</p> }
}

fn document(doc: &Document, glossary: &[PolicyGlossary]) -> Html {
    html! {
        <>
            { for doc.policies.iter().enumerate().map(|(i, p)| policy(p, glossary.get(i))) }
            if !doc.warnings.is_empty() {
                <aside class="prose-warnings">
                    <p class="prose-label">{ "Not rendered" }</p>
                    <ul>{ for doc.warnings.iter().map(|w| html! { <li>{ w }</li> }) }</ul>
                </aside>
            }
        </>
    }
}

fn policy(p: &Policy, glossary: Option<&PolicyGlossary>) -> Html {
    let glossary = glossary.filter(|g| !g.terms.is_empty());
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
            if let Some(g) = glossary {
                <ProseGlossary glossary={g.clone()} />
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

#[cfg(doctest)]
mod non_exhaustive_pins;
