//! `<ProseProblems>`: the findings of a validation, grouped by rule, each
//! with a button that goes to it and one per fix. A report without findings
//! and without a verdict (advisory mode) renders nothing.
use std::rc::Rc;
use yew::prelude::*;

use prose_core::edit::{EditEvent, IssueTarget};
use prose_core::validate::{Finding, Mode, Report, RuleId, Verdict};

use wasm_bindgen::JsCast;

use super::config::{Labels, fill};

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// The selectors "Go to" tries for a target, best first: the first one that
/// matches an element of the Edit view gets the focus. A target about the
/// document as a whole goes to the first node there is.
pub fn target_selectors(target: &IssueTarget) -> Vec<String> {
    match target {
        IssueTarget::Slot(s) => vec![
            format!("[data-slot=\"{}\"]", escape(&s.to_string())),
            format!("[data-node=\"{}\"]", escape(&s.node.to_string())),
        ],
        IssueTarget::Node(n) => vec![format!("[data-node=\"{}\"]", escape(&n.to_string()))],
        IssueTarget::List(l) => vec![format!("[data-add=\"{}\"]", escape(&l.to_string()))],
        _ => vec!["[data-node]".to_string()],
    }
}

/// Moves the keyboard focus to the first element of `root` that one of the
/// target's selectors finds, and scrolls it into view. A node is not
/// focusable by itself, so it is made so first. `false` when nothing matched.
/// Hosts call it from `<ProseProblems onjump>`.
pub fn go_to(root: &NodeRef, target: &IssueTarget) -> bool {
    let Some(root) = root.cast::<web_sys::Element>() else {
        return false;
    };
    for sel in target_selectors(target) {
        let Ok(Some(el)) = root.query_selector(&sel) else {
            continue;
        };
        let Ok(el) = el.dyn_into::<web_sys::HtmlElement>() else {
            continue;
        };
        if el.tab_index() < 0 && el.get_attribute("tabindex").is_none() {
            let _ = el.set_attribute("tabindex", "-1");
        }
        let _ = el.focus();
        el.scroll_into_view();
        return true;
    }
    false
}

#[derive(Properties, PartialEq, Clone)]
pub struct ProseProblemsProps {
    pub report: Rc<Report>,
    /// A "Go to" button was pressed: the host moves focus to the target
    /// (a `[data-slot]` or `[data-node]` element of the view).
    #[prop_or_default]
    pub onjump: Callback<IssueTarget>,
    /// A fix button was pressed: apply the events as one (see
    /// `ProseEditorWith::apply_all`). They carry `expect` ids, so a fix from
    /// a report the document has moved on from is refused as stale.
    #[prop_or_default]
    pub onfix: Callback<Vec<EditEvent>>,
    #[prop_or_default]
    pub labels: Rc<Labels>,
    #[prop_or_default]
    pub class: Classes,
}

/// The status line: only a strict report can certify anything.
pub(crate) fn status(report: &Report, l: &Labels) -> Option<(&'static str, AttrValue)> {
    if report.mode != Mode::Strict {
        return None;
    }
    Some(match report.verdict {
        Verdict::Conforms => ("prose-conforms", l.status_conforms.clone()),
        Verdict::Incomplete => ("prose-incomplete", l.status_incomplete.clone()),
        // A verdict this version does not know is not a pass.
        _ => ("prose-violations-found", l.status_violations.clone()),
    })
}

fn severity_class(f: &Finding) -> &'static str {
    match f.severity {
        prose_core::edit::Severity::Error => "prose-problem-error",
        prose_core::edit::Severity::Warning => "prose-problem-warning",
        prose_core::edit::Severity::Info => "prose-problem-info",
    }
}

#[function_component(ProseProblems)]
pub fn prose_problems(props: &ProseProblemsProps) -> Html {
    let ProseProblemsProps {
        report,
        onjump,
        onfix,
        labels,
        class,
    } = props;
    // In order of first appearance: document order for the groups too.
    let mut groups: Vec<(RuleId, Vec<&Finding>)> = vec![];
    for f in &report.findings {
        match groups.iter_mut().find(|(r, _)| *r == f.rule) {
            Some((_, list)) => list.push(f),
            None => groups.push((f.rule, vec![f])),
        }
    }
    let counts = report.counts();
    let status = status(report, labels);
    // A fix removes the finding, and with it the button that has the focus:
    // the browser would send the focus to the page. The button that was
    // pressed is kept until the panel has rendered the new report; if it is
    // gone and the focus fell, the heading takes it.
    let heading = use_node_ref();
    // Whether the panel has been shown: when its last finding goes (an
    // advisory report has no verdict to keep it), an empty target stays for
    // the focus, so it does not fall to the page.
    let was_shown = use_mut_ref(|| false);
    let pressed = use_mut_ref(|| None::<web_sys::Element>);
    {
        let heading = heading.clone();
        let pressed = pressed.clone();
        use_effect(move || {
            let gone = pressed
                .borrow()
                .as_ref()
                .is_some_and(|button| !button.is_connected());
            if gone {
                pressed.borrow_mut().take();
                let fell = web_sys::window()
                    .and_then(|w| w.document())
                    .and_then(|d| d.active_element())
                    .is_none_or(|a| a.tag_name() == "BODY");
                if fell && let Some(h) = heading.cast::<web_sys::HtmlElement>() {
                    let _ = h.set_attribute("tabindex", "-1");
                    let _ = h.focus();
                }
            }
            || ()
        });
    }
    let onfix = {
        let onfix = onfix.clone();
        let pressed = pressed.clone();
        Callback::from(
            move |(events, button): (Vec<EditEvent>, Option<web_sys::Element>)| {
                *pressed.borrow_mut() = button;
                onfix.emit(events);
            },
        )
    };
    // Nothing to say: no findings, and an advisory report has no verdict.
    if report.findings.is_empty() && status.is_none() {
        if *was_shown.borrow() {
            return html! { <div class="prose-problems-clear" ref={heading}></div> };
        }
        return Html::default();
    }
    *was_shown.borrow_mut() = true;
    html! {
        <section class={classes!("prose-problems", class.clone())}>
            <h3 class="prose-problems-heading" ref={heading}>{ labels.problems_heading.clone() }</h3>
            if let Some((kind, text)) = status {
                <p class={classes!("prose-problems-status", kind)} role="status">{ text }</p>
            }
            if counts.total() > 0 {
                <p class="prose-problems-counts">
                    { fill(&labels.counts, &[
                        ("errors", &counts.error.to_string()),
                        ("warnings", &counts.warning.to_string()),
                        ("notes", &counts.info.to_string()),
                    ]) }
                </p>
                <ol class="prose-problem-groups">
                    { for groups.iter().map(|(rule, list)| html! {
                        <li class="prose-problem-group">
                            <h4><code>{ rule.as_str() }</code>{ " " }{ rule.info().title }</h4>
                            <ol class="prose-problem-list">
                                { for list.iter().map(|f| problem(f, labels, onjump, &onfix)) }
                            </ol>
                        </li>
                    }) }
                </ol>
            }
        </section>
    }
}

fn problem(
    f: &Finding,
    labels: &Labels,
    onjump: &Callback<IssueTarget>,
    onfix: &Callback<(Vec<EditEvent>, Option<web_sys::Element>)>,
) -> Html {
    let go = {
        let target = f.target.clone();
        let onjump = onjump.clone();
        Callback::from(move |_: MouseEvent| onjump.emit(target.clone()))
    };
    let mut class = classes!("prose-problem", severity_class(f));
    if f.gap {
        class.push("prose-problem-gap");
    }
    html! {
        <li class={class}>
            <span class="prose-problem-message">{ f.message.clone() }</span>
            { " " }
            <button type="button" class="prose-problem-go" onclick={go}>
                { labels.go_to.clone() }
            </button>
            { for f.fixes.iter().map(|fix| {
                let events = fix.events.clone();
                let onfix = onfix.clone();
                let click = Callback::from(move |_: MouseEvent| {
                    // The button that has the focus, if it is in a panel: it is
                    // the one to go.
                    let button = web_sys::window()
                        .and_then(|w| w.document())
                        .and_then(|d| d.active_element())
                        .filter(|a| a.closest(".prose-problems").ok().flatten().is_some());
                    onfix.emit((events.clone(), button));
                });
                html! {
                    <>
                        { " " }
                        <button
                            type="button"
                            class="prose-problem-fix"
                            data-safe={fix.safe.to_string()}
                            onclick={click}
                        >
                            { fix.label.clone() }
                        </button>
                    </>
                }
            }) }
        </li>
    }
}
