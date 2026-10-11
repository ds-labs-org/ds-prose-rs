//! What the demo's strict mode shows, as pure functions the browser tests
//! and the host tests both reach: where "Go to" lands, whether the export
//! is open, and what the status badge says.
use std::rc::Rc;

use prose_yew::prose_core::edit::IssueTarget;
use prose_yew::prose_core::validate::{Finding, Mode, Report, Verdict};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use yew::prelude::*;

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// The selectors "Go to" tries for a finding, best first: the first one that
/// matches an element of the Edit view gets the focus. A finding about the
/// document as a whole goes to the first node there is.
pub fn jump_selectors(f: &Finding) -> Vec<String> {
    target_selectors(&f.target)
}

/// As [`jump_selectors`], for the target alone (what the problems panel
/// reports when "Go to" is pressed).
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

/// The first of [`jump_selectors`].
pub fn jump_selector(f: &Finding) -> String {
    jump_selectors(f).into_iter().next().unwrap_or_default()
}

/// Whether "Copy conformant JSON" is open: only a strict report that
/// conforms certifies anything.
pub fn export_enabled(r: &Report) -> bool {
    r.mode == Mode::Strict && r.verdict == Verdict::Conforms
}

/// What a gap says, as the end of "Cannot certify: ...": the profile that
/// is not loaded, else the finding's own words.
fn gap_reason(r: &Report) -> Option<String> {
    let gap = r.gaps.first()?;
    Some(match &gap.profile {
        Some(iri) => format!("profile {iri} is not loaded"),
        None => gap.message.clone(),
    })
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// "2 errors, 1 warning": the severities that have a finding.
fn tally(r: &Report) -> String {
    let c = r.counts();
    let parts: Vec<String> = [
        (c.error, "error", "errors"),
        (c.warning, "warning", "warnings"),
        (c.info, "note", "notes"),
    ]
    .into_iter()
    .filter(|(n, ..)| *n > 0)
    .map(|(n, one, many)| plural(n, one, many))
    .collect();
    parts.join(", ")
}

/// Why the export is shut, when it is.
pub fn export_note(r: &Report) -> Option<String> {
    if export_enabled(r) {
        return None;
    }
    if r.mode != Mode::Strict {
        return Some("Switch strict validation on to certify a policy.".to_string());
    }
    Some(match r.verdict {
        Verdict::Incomplete => match r.gaps.first().and_then(|g| g.profile.as_ref()) {
            Some(iri) => format!("Load profile {iri} to certify."),
            None => format!("Cannot certify: {}.", gap_reason(r).unwrap_or_default()),
        },
        // A verdict this version does not know is not a pass.
        _ => format!("The policy does not conform: {}.", tally(r)),
    })
}

/// The status badge: a class and the sentence. `profile` is the name of the
/// loaded profile, when there is one.
pub fn badge(r: &Report, profile: Option<&str>) -> (&'static str, String) {
    if r.mode != Mode::Strict {
        return ("prose-advisory", tally(r));
    }
    match r.verdict {
        Verdict::Conforms => (
            "prose-conforms",
            match profile {
                Some(p) => format!("Conforms to {p} (strict)"),
                None => "Conforms (strict)".to_string(),
            },
        ),
        Verdict::Incomplete => (
            "prose-incomplete",
            format!("Cannot certify: {}", gap_reason(r).unwrap_or_default()),
        ),
        _ => ("prose-violations-found", tally(r)),
    }
}

/// The report as the `ds-prose/validation-report@1` JSON the download holds.
pub fn report_text(r: &Report) -> String {
    serde_json::to_string_pretty(&r.to_json()).unwrap_or_default()
}

/// Moves the keyboard focus to the first element of `root` that one of the
/// target's selectors finds, and scrolls it into view. A node is not
/// focusable by itself, so it is made so first. `false` when nothing matched.
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

/// Downloads `text` as `name`, through a Blob URL that lives for the click.
fn download(name: &str, text: &str) {
    let parts = js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(text));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("application/json");
    let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts) else {
        return;
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };
    if let Some(a) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.create_element("a").ok())
        .and_then(|a| a.dyn_into::<web_sys::HtmlElement>().ok())
    {
        let _ = a.set_attribute("href", &url);
        let _ = a.set_attribute("download", name);
        a.click();
    }
    let _ = web_sys::Url::revoke_object_url(&url);
}

#[derive(Properties, PartialEq, Clone)]
pub struct StrictBarProps {
    pub report: Rc<Report>,
    /// The loaded profile's name, for the badge.
    #[prop_or_default]
    pub profile: Option<AttrValue>,
    /// The policy as JSON-LD, which "Copy conformant JSON" copies.
    pub json: AttrValue,
}

/// The verdict as a status line, the export button it gates and the
/// report download. Shown while strict is on, in both tabs.
#[function_component(StrictBar)]
pub fn strict_bar(props: &StrictBarProps) -> Html {
    let r = &props.report;
    let (class, text) = badge(r, props.profile.as_deref());
    let open = export_enabled(r);
    let note = export_note(r);
    let copied = use_state(|| None::<AttrValue>);
    let copy = {
        let json = props.json.clone();
        let copied = copied.clone();
        Callback::from(move |_: MouseEvent| {
            let Some(w) = web_sys::window() else { return };
            let promise = w.navigator().clipboard().write_text(&json);
            let copied = copied.clone();
            spawn_local(async move {
                copied.set(Some(match JsFuture::from(promise).await {
                    Ok(_) => "Copied.".into(),
                    Err(_) => {
                        "The browser would not copy; select the text in the JSON box instead."
                            .into()
                    }
                }));
            });
        })
    };
    let report = r.clone();
    let save = Callback::from(move |_: MouseEvent| {
        download("validation-report.json", &report_text(&report))
    });
    html! {
        <div class="strict-bar">
            <p id="conformance" class={classes!("conformance", class)} role="status">{ text }</p>
            <div class="strict-actions">
                <button type="button" id="copy-conformant" disabled={!open} onclick={copy}>
                    { "Copy conformant JSON" }
                </button>
                <button type="button" id="report-json" onclick={save}>{ "Download the report" }</button>
            </div>
            if let Some(n) = note { <p id="export-note" class="hint">{ n }</p> }
            if open { if let Some(c) = &*copied { <p id="copy-status" class="hint" role="status">{ c.clone() }</p> } }
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::profile_field::{PRESETS, ProfileMsg, ProfileState, reduce};
    use prose_yew::prose_core::edit::read_model;
    use prose_yew::prose_core::validate::{Options, Validator, Verdict};
    use std::sync::Arc;

    fn closed() -> ProfileState {
        let p = PRESETS
            .iter()
            .find(|p| p.id == "profile-sample-closed")
            .expect("the closed sample is a preset");
        reduce(&ProfileState::default(), ProfileMsg::Use(p.text.into()))
    }

    fn report(json: &str, options: Options, with_profile: bool) -> Report {
        let w = match (&closed().wording, with_profile) {
            (Some(w), true) => w.0.clone(),
            _ => Default::default(),
        };
        Validator::new(Arc::new((*w).clone()), options).validate(&read_model(json).unwrap())
    }

    fn preset(label: &str) -> &'static str {
        fixtures::ALL
            .iter()
            .find(|p| p.label == label)
            .unwrap_or_else(|| panic!("no preset {label}"))
            .json
    }

    const VIOLATES: &str = "Research agreement (violates the profile)";

    #[test]
    fn the_strict_presets_are_the_shared_fixtures() {
        assert_eq!(
            preset(VIOLATES),
            include_str!("../../prose-core/tests/strict/violating.json")
        );
        assert_eq!(
            preset("Strict clean"),
            include_str!("../../prose-core/tests/strict/clean.json")
        );
        // The one that has a fix to offer.
        assert_eq!(
            preset("Profile not declared (has a fix)"),
            include_str!("../../prose-core/tests/strict/not-declared.json")
        );
        // The first four are as they were.
        assert_eq!(fixtures::ALL[3].label, "Research agreement");
    }

    #[test]
    fn closed_sample_is_the_closed_profile_file() {
        let s = closed();
        assert_eq!(
            s.text,
            include_str!("../../prose-core/tests/profiles/research-closed.jsonld")
        );
        assert!(s.wording.is_some(), "{:?}", s.error);
    }

    #[test]
    fn a_slot_finding_jumps_to_its_slot_then_its_node() {
        let r = report(preset(VIOLATES), Options::strict(), true);
        let f = r
            .findings
            .iter()
            .find(|f| f.rule.as_str() == "profile.unknown-term" && f.message.contains("foo:bar"))
            .unwrap_or_else(|| panic!("{:#?}", r.findings));
        let sels = jump_selectors(f);
        assert_eq!(
            sels,
            vec![
                "[data-slot=\"policy[0].permission[2].action[0]#name\"]".to_string(),
                "[data-node=\"policy[0].permission[2].action[0]\"]".to_string(),
            ]
        );
        assert_eq!(jump_selector(f), sels[0]);
    }

    #[test]
    fn a_doc_finding_jumps_to_the_first_node() {
        // What no node holds: the text that is not a policy.
        let v = Validator::new(Arc::new(Default::default()), Options::strict());
        let r = v.validate_json("{");
        let f = r
            .findings
            .iter()
            .find(|f| matches!(f.target, prose_yew::prose_core::edit::IssueTarget::Doc))
            .unwrap_or_else(|| panic!("{:#?}", r.findings));
        assert_eq!(jump_selectors(f), vec!["[data-node]".to_string()]);
    }

    #[test]
    fn the_export_is_open_only_for_a_strict_report_that_conforms() {
        let clean = report(preset("Strict clean"), Options::strict(), true);
        assert_eq!(clean.verdict, Verdict::Conforms, "{:#?}", clean.findings);
        assert!(export_enabled(&clean));
        assert_eq!(export_note(&clean), None);

        let bad = report(preset(VIOLATES), Options::strict(), true);
        assert!(!export_enabled(&bad));
        let note = export_note(&bad).expect("a reason");
        assert!(note.contains("error"), "{note}");

        // Nothing certifies without the profile the policy declares.
        let open = report(preset("Strict clean"), Options::strict(), false);
        assert_eq!(open.verdict, Verdict::Incomplete);
        assert!(!export_enabled(&open));
        assert_eq!(
            export_note(&open).as_deref(),
            Some("Load profile https://example.org/odrl/profile/research to certify.")
        );

        // An advisory report certifies nothing either.
        let adv = report(preset("Strict clean"), Options::advisory(), true);
        assert!(!export_enabled(&adv));
    }

    #[test]
    fn the_badge_says_the_verdict() {
        let clean = report(preset("Strict clean"), Options::strict(), true);
        assert_eq!(
            badge(&clean, Some("Research Data Profile 0.1.0")),
            (
                "prose-conforms",
                "Conforms to Research Data Profile 0.1.0 (strict)".to_string()
            )
        );
        assert_eq!(
            badge(&clean, None).1,
            "Conforms (strict)",
            "no profile named"
        );

        let bad = report(preset(VIOLATES), Options::strict(), true);
        let (class, text) = badge(&bad, Some("x"));
        assert_eq!(class, "prose-violations-found");
        let c = bad.counts();
        let part =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        assert!(c.error > 1, "{c:?}");
        assert!(
            text.starts_with(&part(c.error, "error", "errors")),
            "{text}"
        );

        let open = report(preset("Strict clean"), Options::strict(), false);
        assert_eq!(
            badge(&open, None),
            (
                "prose-incomplete",
                "Cannot certify: profile https://example.org/odrl/profile/research is not loaded"
                    .to_string()
            )
        );
    }

    #[test]
    fn the_report_text_is_the_schema() {
        let bad = report(preset(VIOLATES), Options::strict(), true);
        let v: serde_json::Value = serde_json::from_str(&report_text(&bad)).unwrap();
        assert_eq!(v["schema"], "ds-prose/validation-report@1");
        assert_eq!(v["verdict"], "violations");
        assert!(report_text(&bad).contains('\n'), "pretty-printed");
    }
}
