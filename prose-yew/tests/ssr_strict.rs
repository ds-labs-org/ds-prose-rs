//! Strict validation in the components, rendered on the server: the hooks,
//! the problems panel, the violations section of the read-only component,
//! and the markers on the editing view. Without a validator nothing here
//! changes what was rendered before (see `ssr.rs` and its snapshots).
#![cfg(not(target_arch = "wasm32"))]
use prose_core::edit::{EditDoc, EditRules, read_model};
use prose_core::profile::{ProfileVocab, Wording};
use prose_core::validate::{Options, Validator};
use prose_yew::{StrictEdit, use_prose_editor_with, use_validation};
use std::rc::Rc;
use std::sync::Arc;
use yew::prelude::*;

async fn ssr<C: BaseComponent>(props: C::Properties) -> String {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(yew::LocalServerRenderer::<C>::with_props(props).render())
        .await
}

fn strict_file(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../prose-core/tests/strict/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn profile_file(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../prose-core/tests/profiles/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn closed_wording() -> Arc<Wording> {
    let p = ProfileVocab::parse(&profile_file("research-closed.jsonld")).unwrap();
    Arc::new(Wording::new().with_profile(p))
}

fn strict() -> Validator {
    Validator::new(closed_wording(), Options::strict())
}

fn doc_of(file: &str) -> Rc<EditDoc> {
    Rc::new(read_model(&strict_file(file)).unwrap())
}

// --- the hooks -------------------------------------------------------------------

#[derive(Properties, PartialEq, Clone)]
struct ProbeProps {
    doc: Rc<EditDoc>,
    validator: Option<Validator>,
}

#[function_component(ValidationProbe)]
fn validation_probe(p: &ProbeProps) -> Html {
    let report = use_validation(&p.doc, p.validator.as_ref());
    html! {
        <p id="v" data-mode={format!("{:?}", report.mode)}
           data-verdict={format!("{:?}", report.verdict)}
           data-findings={report.findings.len().to_string()} />
    }
}

fn probe(doc: Rc<EditDoc>, validator: Option<Validator>) -> ProbeProps {
    yew::props!(ProbeProps { doc, validator })
}

#[tokio::test]
async fn use_validation_without_a_validator_finds_nothing() {
    let html = ssr::<ValidationProbe>(probe(doc_of("violating.json"), None)).await;
    assert!(html.contains("data-mode=\"Advisory\""), "{html}");
    assert!(html.contains("data-findings=\"0\""), "{html}");
}

#[tokio::test]
async fn use_validation_reports_what_the_validator_finds() {
    let html = ssr::<ValidationProbe>(probe(doc_of("violating.json"), Some(strict()))).await;
    assert!(html.contains("data-mode=\"Strict\""), "{html}");
    assert!(html.contains("data-verdict=\"Violations\""), "{html}");
    assert!(!html.contains("data-findings=\"0\""), "{html}");
    let html = ssr::<ValidationProbe>(probe(doc_of("clean.json"), Some(strict()))).await;
    assert!(html.contains("data-verdict=\"Conforms\""), "{html}");
    assert!(html.contains("data-findings=\"0\""), "{html}");
}

#[function_component(EditorProbe)]
fn editor_probe(p: &ProbeProps) -> Html {
    let doc = p.doc.clone();
    let with = use_prose_editor_with(
        move || (*doc).clone(),
        Rc::new(EditRules::default()),
        p.validator.clone(),
        StrictEdit::RefuseTerms,
    );
    html! {
        <p id="e" data-policies={with.editor.doc.policies.len().to_string()}
           data-refused={with.refused.is_some().to_string()}
           data-undo={with.editor.can_undo.to_string()} />
    }
}

#[tokio::test]
async fn the_editor_hook_starts_with_the_document_and_no_refusal() {
    let html = ssr::<EditorProbe>(probe(doc_of("violating.json"), Some(strict()))).await;
    assert!(html.contains("data-policies=\"1\""), "{html}");
    assert!(html.contains("data-refused=\"false\""), "{html}");
    assert!(html.contains("data-undo=\"false\""), "{html}");
}

// --- helpers for the markup ------------------------------------------------------

/// Compares with, or (with WRITE_SNAPSHOTS set and the file missing) writes,
/// `tests/snapshots/{name}.html`.
fn snapshot(name: &str, html: &str) {
    let path = format!("{}/tests/snapshots/{name}.html", env!("CARGO_MANIFEST_DIR"));
    if std::env::var("WRITE_SNAPSHOTS").is_ok() && !std::path::Path::new(&path).exists() {
        std::fs::write(&path, html).unwrap();
    }
    let want = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("no snapshot {path}; got:\n{html}"));
    assert_eq!(html, want, "snapshot {name}");
}

/// The `<select>` that carries `data-slot="{slot}"`, to its end.
fn select_of(html: &str, slot: &str) -> String {
    let needle = format!("data-slot=\"{slot}\"");
    let at = html
        .match_indices("<select")
        .map(|(i, _)| i)
        .find(|&i| {
            html[i..]
                .split('>')
                .next()
                .is_some_and(|t| t.contains(&needle))
        })
        .unwrap_or_else(|| panic!("no select for {slot}:\n{html}"));
    let end = html[at..].find("</select>").unwrap();
    html[at..at + end].to_string()
}

/// The opening tag of the element with `data-slot="{slot}"`.
fn tag_of(html: &str, slot: &str) -> String {
    let needle = format!("data-slot=\"{slot}\"");
    let at = html
        .find(&needle)
        .unwrap_or_else(|| panic!("no {slot}:\n{html}"));
    let start = html[..at].rfind('<').unwrap();
    let end = at + html[at..].find('>').unwrap();
    html[start..=end].to_string()
}

const PURPOSE_OP: &str = "policy[0].permission[0].constraint[0]#operator";
const DISTIL: &str = "policy[0].permission[1].action[0]#name";
const FOREIGN: &str = "policy[0].permission[2].action[0]#name";
const FINE_TUNE: &str = "policy[0].permission[0].action[0]#name";

// --- the problems panel ----------------------------------------------------------

use prose_core::edit::{Issue, IssueTarget, Severity};
use prose_core::validate::Report;
use prose_yew::{EditConfig, EditMode, OdrlProse, OdrlProseView, ProseProblems, SharedWording};

fn report_of(file: &str, v: &Validator) -> Rc<Report> {
    Rc::new(v.validate(&doc_of(file)))
}

fn panel(report: Rc<Report>) -> ProseProblemsProps {
    yew::props!(ProseProblemsProps { report })
}

use prose_yew::ProseProblemsProps;

#[tokio::test]
async fn problems_panel_snapshot() {
    let html = ssr::<ProseProblems>(panel(report_of("violating.json", &strict()))).await;
    snapshot("strict_problems", &html);
}

#[tokio::test]
async fn the_panel_groups_findings_by_rule_in_document_order() {
    let html = ssr::<ProseProblems>(panel(report_of("violating.json", &strict()))).await;
    assert!(
        html.contains("<section class=\"prose-problems\">"),
        "{html}"
    );
    assert!(html.contains("Does not conform"), "{html}");
    assert!(html.contains("errors: 4, warnings: 0, notes: 0"), "{html}");
    let order: Vec<usize> = [
        "prose.datatype-lexical",
        "prose.operator-not-allowed",
        "profile.undeclared-in-namespace",
        "profile.unknown-term",
    ]
    .iter()
    .map(|rule| {
        assert_eq!(
            html.matches(rule).count(),
            1,
            "{rule} is a heading once:\n{html}"
        );
        html.find(rule).unwrap()
    })
    .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
    // Every finding can be jumped to.
    assert_eq!(html.matches(">Go to<").count(), 4, "{html}");
    assert!(
        html.contains("\"foo:bar\" is not defined") || html.contains("foo:bar"),
        "{html}"
    );
}

#[tokio::test]
async fn the_panel_offers_each_fix_as_a_button() {
    let html = ssr::<ProseProblems>(panel(report_of("not-declared.json", &strict()))).await;
    assert!(
        html.contains("Declare the profile &lt;https://example.org/odrl/profile/research&gt;"),
        "{html}"
    );
    assert!(html.contains("data-safe=\"true\""), "{html}");
}

#[tokio::test]
async fn the_panel_says_conforms_and_lists_nothing() {
    let html = ssr::<ProseProblems>(panel(report_of("clean.json", &strict()))).await;
    assert!(html.contains("Conforms"), "{html}");
    assert!(!html.contains("<ol"), "{html}");
    assert!(!html.contains("Go to"), "{html}");
}

#[tokio::test]
async fn the_panel_says_when_it_cannot_certify() {
    let html = ssr::<ProseProblems>(panel(report_of("not-loaded.json", &strict()))).await;
    assert!(html.contains("Cannot be certified"), "{html}");
    assert!(html.contains("profile.not-loaded"), "{html}");
}

#[tokio::test]
async fn an_advisory_report_has_no_verdict_line() {
    let v = Validator::new(closed_wording(), Options::advisory());
    let html = ssr::<ProseProblems>(panel(report_of("clean.json", &v))).await;
    for said in ["Conforms", "Does not conform", "Cannot be certified"] {
        assert!(!html.contains(said), "{said}:\n{html}");
    }
}

// --- the read-only component -----------------------------------------------------

fn read_props(
    file: &str,
    validator: Option<Validator>,
    violations: bool,
) -> prose_yew::OdrlProseProps {
    let json = AttrValue::from(strict_file(file));
    let wording = Some(SharedWording(Rc::new((*closed_wording()).clone())));
    yew::props!(prose_yew::OdrlProseProps {
        json,
        wording,
        validator,
        violations
    })
}

#[tokio::test]
async fn read_violations_section_snapshot() {
    let html = ssr::<OdrlProse>(read_props("violating.json", Some(strict()), true)).await;
    snapshot("strict_read_violations", &html);
}

#[tokio::test]
async fn the_violations_section_lists_the_findings_after_the_policies() {
    let html = ssr::<OdrlProse>(read_props("violating.json", Some(strict()), true)).await;
    let section = html
        .find("<section class=\"prose-violations\"")
        .expect(&html);
    assert!(html[..section].contains("prose-policy"), "{html}");
    assert!(html.contains("Does not conform"), "{html}");
    assert_eq!(
        html.matches("class=\"prose-violation prose-violation-error\"")
            .count(),
        4,
        "{html}"
    );
    assert!(html.contains("profile.unknown-term"), "{html}");
}

#[tokio::test]
async fn a_conforming_policy_says_so_in_the_section() {
    let html = ssr::<OdrlProse>(read_props("clean.json", Some(strict()), true)).await;
    assert!(
        html.contains("<section class=\"prose-violations\""),
        "{html}"
    );
    assert!(html.contains("Conforms"), "{html}");
    assert!(!html.contains("prose-violation-"), "{html}");
}

#[tokio::test]
async fn without_the_prop_or_a_validator_the_rendering_is_as_before() {
    let plain = ssr::<OdrlProse>(read_props("violating.json", None, false)).await;
    for (validator, violations) in [(Some(strict()), false), (None, true)] {
        let html = ssr::<OdrlProse>(read_props("violating.json", validator, violations)).await;
        assert_eq!(html, plain);
        assert!(!html.contains("prose-violations"), "{html}");
    }
}

// --- the editing view ------------------------------------------------------------

fn strict_config() -> EditConfig {
    EditConfig::default()
        .with_wording(Rc::new((*closed_wording()).clone()))
        .with_strict(Options::strict())
}

async fn edit_view(file: &str, config: EditConfig, issues: Vec<Issue>) -> String {
    let doc = doc_of(file);
    let config = Rc::new(config);
    let issues = Rc::new(issues);
    ssr::<OdrlProseView>(yew::props!(prose_yew::OdrlProseViewProps {
        doc,
        mode: EditMode::Edit,
        config,
        issues
    }))
    .await
}

#[tokio::test]
async fn strict_slot_aria_invalid() {
    let cfg = strict_config();
    let issues = cfg
        .validator
        .as_ref()
        .unwrap()
        .validate(&doc_of("violating.json"))
        .issues();
    let html = edit_view("violating.json", cfg, issues).await;
    for slot in [DISTIL, FOREIGN] {
        let tag = tag_of(&html, slot);
        assert!(tag.contains("aria-invalid=\"true\""), "{tag}");
        assert!(tag.contains("prose-slot-invalid"), "{tag}");
        assert!(tag.contains("aria-describedby=\"prose-issue-"), "{tag}");
    }
    assert!(!tag_of(&html, FINE_TUNE).contains("aria-invalid"), "{html}");
    assert!(html.contains("prose-issue-error"), "{html}");
}

#[tokio::test]
async fn a_refusal_is_shown_at_its_slot() {
    let slot: prose_core::edit::SlotPath = DISTIL.parse().unwrap();
    let refusal = Issue {
        target: IssueTarget::Slot(slot),
        severity: Severity::Error,
        message: "refused: not in the profile".into(),
    };
    let doc = doc_of("clean.json");
    let config = Rc::new(strict_config());
    let html = ssr::<OdrlProseView>(yew::props!(prose_yew::OdrlProseViewProps {
        doc,
        mode: EditMode::Edit,
        config,
        refusal: Some(refusal),
    }))
    .await;
    // clean.json has no second permission: the refusal's slot is not there,
    // and nothing is drawn for it.
    assert!(!html.contains("refused: not in the profile"), "{html}");
    let slot: prose_core::edit::SlotPath = FINE_TUNE.parse().unwrap();
    let refusal = Issue {
        target: IssueTarget::Slot(slot),
        severity: Severity::Error,
        message: "refused: not in the profile".into(),
    };
    let doc = doc_of("clean.json");
    let config = Rc::new(strict_config());
    let html = ssr::<OdrlProseView>(yew::props!(prose_yew::OdrlProseViewProps {
        doc,
        mode: EditMode::Edit,
        config,
        refusal: Some(refusal),
    }))
    .await;
    assert!(html.contains("refused: not in the profile"), "{html}");
    assert!(
        tag_of(&html, FINE_TUNE).contains("aria-invalid=\"true\""),
        "{html}"
    );
}

#[tokio::test]
async fn strict_operator_select_disabled_group() {
    let html = edit_view("violating.json", strict_config(), vec![]).await;
    let sel = select_of(&html, PURPOSE_OP);
    let allowed = sel
        .find("<optgroup label=\"Allowed by the profile\">")
        .expect(&sel);
    let blocked = sel
        .find("<optgroup label=\"Not allowed by the profile\" disabled")
        .expect(&sel);
    assert!(allowed < blocked, "{sel}");
    let (allowed_part, blocked_part) = (&sel[allowed..blocked], &sel[blocked..]);
    for op in ["eq", "isAnyOf", "isNoneOf"] {
        assert!(
            allowed_part.contains(&format!("<option value=\"{op}\"")),
            "{op}:\n{sel}"
        );
    }
    // Not hidden: the operators the profile does not allow are listed, in
    // the disabled group.
    for op in ["neq", "gt", "gteq", "lt", "isA"] {
        assert!(
            blocked_part.contains(&format!("<option value=\"{op}\"")),
            "{op}:\n{sel}"
        );
        assert!(
            !allowed_part.contains(&format!("<option value=\"{op}\"")),
            "{op}:\n{sel}"
        );
    }
}

#[tokio::test]
async fn strict_current_operator_kept_marked() {
    // The policy uses lteq, which the closed operand does not allow.
    let html = edit_view("violating.json", strict_config(), vec![]).await;
    let sel = select_of(&html, PURPOSE_OP);
    assert!(
        sel.contains("<option value=\"lteq\" selected=\"selected\""),
        "{sel}"
    );
    assert!(sel.contains("(not allowed)"), "{sel}");
    // Only the current one is marked.
    assert_eq!(sel.matches("(not allowed)").count(), 1, "{sel}");
}

#[tokio::test]
async fn an_operator_the_profile_allows_is_not_marked() {
    let html = edit_view("clean.json", strict_config(), vec![]).await;
    let sel = select_of(&html, PURPOSE_OP);
    assert!(!sel.contains("(not allowed)"), "{sel}");
    assert!(
        sel.contains("<option value=\"isAnyOf\" selected=\"selected\""),
        "{sel}"
    );
}

#[tokio::test]
async fn an_open_operand_keeps_the_suggested_and_other_groups() {
    // ex:retentionPeriod is not closed: the profile suggests, never forbids.
    let html = edit_view("clean.json", strict_config(), vec![]).await;
    let sel = select_of(&html, "policy[0].permission[0].constraint[1]#operator");
    assert!(!sel.contains("Not allowed by the profile"), "{sel}");
    assert!(!sel.contains("Allowed by the profile"), "{sel}");
    assert!(!sel.contains("disabled"), "{sel}");
}

#[tokio::test]
async fn without_strict_nothing_is_disabled_or_marked() {
    let cfg = EditConfig::default().with_wording(Rc::new((*closed_wording()).clone()));
    let html = edit_view("violating.json", cfg, vec![]).await;
    let sel = select_of(&html, PURPOSE_OP);
    for said in [
        "Not allowed by the profile",
        "Allowed by the profile",
        "disabled",
        "(not allowed)",
    ] {
        assert!(!sel.contains(said), "{said}:\n{sel}");
    }
    assert!(!html.contains("data-closed"), "{html}");
    assert!(!html.contains("(not in the profile)"), "{html}");
}

#[tokio::test]
async fn a_closed_slot_with_a_value_outside_the_profile_says_so() {
    let html = edit_view("violating.json", strict_config(), vec![]).await;
    for slot in [DISTIL, FOREIGN] {
        let tag = tag_of(&html, slot);
        assert!(tag.contains("data-closed=\"true\""), "{tag}");
    }
    assert_eq!(html.matches("(not in the profile)").count(), 2, "{html}");
    // A value the profile allows has no note.
    assert!(tag_of(&html, FINE_TUNE).contains("data-closed=\"true\""));
}
#[tokio::test]
async fn strict_operator_select_snapshot() {
    let html = edit_view("violating.json", strict_config(), vec![]).await;
    snapshot("strict_operator_select", &select_of(&html, PURPOSE_OP));
}
