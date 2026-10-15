//! `<OdrlProse>`: renders ODRL JSON-LD as plain-language prose. All reading
//! lives in [`prose_core`]; this crate only lays a [`prose_core::Document`]
//! out as semantic HTML. The host supplies styling through the `prose-*`
//! class names below (or `class` on the root).
//!
//! Classes: `ds-prose` (root), `prose-policy`, `prose-rules`, `prose-rule`
//! plus `prose-permission` / `prose-prohibition` / `prose-obligation`,
//! `prose-rule-sentence`, `prose-label`, `prose-notes`, `prose-warnings`,
//! `prose-error`, `prose-reference` (a link to a constraint declared outside
//! the document, see [`OdrlProseProps::links`]), `prose-iri` (a party, asset
//! or value named by an IRI, shown short with the IRI in its title), and in
//! the glossary of profile terms ([`ProseGlossary`])
//! `prose-glossary`, `prose-note`, `prose-kind-of`, `prose-implies`,
//! `prose-deprecated`, `prose-from`.
//!
//! A profile reaches the components as a prop: [`OdrlProseProps::wording`]
//! (a [`SharedWording`]) for `<OdrlProse>`, [`EditConfig::with_wording`] for
//! [`OdrlProseView`]. Without one, both render exactly as before 0.4.
//!
//! Strict validation is opt-in too: [`OdrlProseProps::validator`] with
//! `violations` shows the findings and the verdict under the prose, and
//! [`EditConfig::with_strict`] marks them in the editor (see [`edit`]).
//! Without a validator, nothing is validated or rendered for it.
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
use prose_core::{
    Condition, Document, Mention, Policy, PolicyGlossary, ProseError, Rule, RuleKind,
};
use std::collections::BTreeSet;
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
    /// IRIs the host found to dereference (the catalog: those that answer
    /// HTTP 200). A condition that refers to a constraint declared at one
    /// of them shows the IRI as a link, and so does a party, an asset or a
    /// value named by an `http(s)` IRI among them; any other IRI is not a
    /// link. The component fetches nothing. Empty by default: no link.
    #[prop_or_default]
    pub links: Rc<BTreeSet<String>>,
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
                    { document(&r.document, glossary, &r.mentions, &props.links) }
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

/// The IRIs a reference may link to.
type Links = BTreeSet<String>;

/// What the text of one policy is shown with: the IRIs it names, and the
/// IRIs that may be links.
struct Cx<'a> {
    mentions: &'a [Mention],
    links: &'a Links,
}

fn document(
    doc: &Document,
    glossary: &[PolicyGlossary],
    mentions: &[Vec<Mention>],
    links: &Links,
) -> Html {
    html! {
        <>
            { for doc.policies.iter().enumerate().map(|(i, p)| {
                let cx = Cx { mentions: mentions.get(i).map_or(&[], Vec::as_slice), links };
                policy(p, glossary.get(i), &cx)
            }) }
            if !doc.warnings.is_empty() {
                <aside class="prose-warnings">
                    <p class="prose-label">{ "Not rendered" }</p>
                    <ul>{ for doc.warnings.iter().map(|w| html! { <li>{ w }</li> }) }</ul>
                </aside>
            }
        </>
    }
}

fn policy(p: &Policy, glossary: Option<&PolicyGlossary>, cx: &Cx) -> Html {
    let glossary = glossary.filter(|g| !g.terms.is_empty());
    html! {
        <section class="prose-policy">
            <h3>{ &p.heading }</h3>
            // The policy's parties are the ones its rules inherit, so their
            // references are among the rules'.
            <p>{ linked_text(&p.intro, p.rules.iter().flat_map(|r| &r.references), cx) }</p>
            if !p.notes.is_empty() {
                <ul class="prose-notes">{ for p.notes.iter().map(|n| html! { <li>{ n }</li> }) }</ul>
            }
            if !p.rules.is_empty() {
                <ol class="prose-rules">{ for p.rules.iter().map(|r| rule(r, cx)) }</ol>
            }
            if let Some(g) = glossary {
                <ProseGlossary glossary={g.clone()} />
            }
        </section>
    }
}

fn rule(r: &Rule, cx: &Cx) -> Html {
    let kind = match r.kind {
        RuleKind::Permission => "prose-permission",
        RuleKind::Prohibition => "prose-prohibition",
        RuleKind::Obligation => "prose-obligation",
    };
    html! {
        <li class={classes!("prose-rule", kind)}>
            <p class="prose-rule-sentence">{ linked_text(&r.sentence, &r.references, cx) }</p>
            { conditions("Limits on the action", &r.refinements, cx) }
            { conditions("Applies only if", &r.conditions, cx) }
            { for r.follow_ups.iter().map(|f| html! {
                <>
                    <p class="prose-label">{ format!("{}:", f.label) }</p>
                    <ul>{ for f.rules.iter().map(|r| rule(r, cx)) }</ul>
                </>
            }) }
        </li>
    }
}

fn conditions(label: &str, list: &[Condition], cx: &Cx) -> Html {
    if list.is_empty() {
        return html! {};
    }
    html! {
        <>
            <p class="prose-label">{ format!("{label}:") }</p>
            { condition_list(list, cx) }
        </>
    }
}

fn condition_list(list: &[Condition], cx: &Cx) -> Html {
    html! {
        <ul>
            { for list.iter().map(|c| html! {
                <li>
                    { condition_text(c, cx) }
                    if !c.children.is_empty() { { ":" } { condition_list(&c.children, cx) } }
                </li>
            }) }
        </ul>
    }
}

/// A condition's text, with the IRI of the constraint it refers to as a
/// link when the host found that IRI dereferences.
fn condition_text(c: &Condition, cx: &Cx) -> Html {
    linked_text(&c.text, &c.reference, cx)
}

/// `text`, in which each "the constraint defined at {IRI} holds" whose IRI
/// is one of `iris` keeps the IRI in full, as a link when it is one of the
/// host's links, and the IRIs the policy names elsewhere are shown short
/// ([`named_text`]). Only that clause is a reference: the same IRI
/// elsewhere in the text (an asset named by it) is a name.
fn linked_text<'i>(text: &str, iris: impl IntoIterator<Item = &'i String>, cx: &Cx) -> Html {
    const BEFORE: &str = "the constraint defined at ";
    let iris: Vec<&str> = iris.into_iter().map(String::as_str).collect();
    let mut parts: Vec<Html> = Vec::new();
    let mut rest = text;
    loop {
        let next = iris
            .iter()
            .filter_map(|iri| {
                let at = rest.find(&format!("{BEFORE}{iri} holds"))?;
                Some((at, *iri))
            })
            .min();
        let Some((at, iri)) = next else { break };
        let start = at + BEFORE.len();
        named_text(&rest[..start], cx, &mut parts);
        match cx.links.contains(iri) {
            true => {
                parts.push(html! { <a class="prose-reference" href={iri.to_string()}>{ iri }</a> })
            }
            false => parts.push(html! { { iri } }),
        }
        rest = &rest[start + iri.len()..];
    }
    if rest.len() == text.len() && next_mention(text, cx.mentions).is_none() {
        return html! { { text } };
    }
    named_text(rest, cx, &mut parts);
    html! { <>{ for parts }</> }
}

/// `text` into `parts`, each IRI the policy names ([`Mention::text`],
/// standing as a word of its own) as an element that shows it short and
/// keeps the IRI in its title: a link when it is an `http(s)` IRI among
/// the host's links. A stretch with no IRI is one text node.
fn named_text(text: &str, cx: &Cx, parts: &mut Vec<Html>) {
    let mut rest = text;
    while let Some((at, m)) = next_mention(rest, cx.mentions) {
        if at > 0 {
            parts.push(html! { { &rest[..at] } });
        }
        parts.push(iri(m, cx));
        rest = &rest[at + m.text().len()..];
    }
    if !rest.is_empty() {
        parts.push(html! { { rest } });
    }
}

/// The first mention in `text` that stands as a word of its own, the
/// longest where several start there.
fn next_mention<'m>(text: &str, mentions: &'m [Mention]) -> Option<(usize, &'m Mention)> {
    mentions
        .iter()
        .filter(|m| !m.text().is_empty())
        .filter_map(|m| {
            let at = text
                .match_indices(m.text())
                .map(|(at, _)| at)
                .find(|&at| stands_alone(text, at, m.text().len()))?;
            Some((at, m))
        })
        .min_by_key(|(at, m)| (*at, std::cmp::Reverse(m.text().len())))
}

/// Whether `text[at..at + len]` is not part of a longer word or IRI: after
/// the start, a space or an opening bracket or quote; before the end, a
/// space, a closing bracket or quote, or punctuation that ends the clause.
fn stands_alone(text: &str, at: usize, len: usize) -> bool {
    let before = text[..at].chars().next_back();
    let mut after = text[at + len..].chars();
    let opens = before.is_none_or(|c| c.is_whitespace() || matches!(c, '(' | '"'));
    let closes = match after.next() {
        None => true,
        Some(c) if c.is_whitespace() || matches!(c, ',' | ';' | ')' | '"') => true,
        Some('.' | ':') => after.next().is_none_or(char::is_whitespace),
        Some(_) => false,
    };
    opens && closes
}

/// One IRI the prose names, short, with the full IRI in its title. Where
/// another IRI of the policy has the same short label, it is not short.
fn iri(m: &Mention, cx: &Cx) -> Html {
    let short = m.short_label();
    let shared = cx
        .mentions
        .iter()
        .any(|o| o.iri != m.iri && o.short_label() == short);
    let shown = if shared { m.text() } else { short };
    let links = cx.links;
    let title = m.iri.clone();
    let web = m.iri.starts_with("http://") || m.iri.starts_with("https://");
    if web && links.contains(&m.iri) {
        html! { <a class="prose-iri" href={m.iri.clone()} {title}>{ shown }</a> }
    } else {
        html! { <span class="prose-iri" {title}>{ shown }</span> }
    }
}

#[cfg(doctest)]
mod non_exhaustive_pins;
