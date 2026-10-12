//! The code shown on the documentation site (site/content/yew/*.md), compiled
//! and server-rendered here. The site embeds each `region` of this file with
//! `{{source prose-yew/tests/site_examples.rs#name}}`, so a snippet that stops
//! compiling fails `cargo test`, and a docs page cannot show other code.
//!
//! The same file pins the policy and the sentences on the home page: the
//! renderer's output for the sample is stored in `site/content/examples/` and
//! compared here. After a deliberate change of the renderer, regenerate with
//! `WRITE_SITE_EXAMPLES=1 cargo test -p prose-yew --test site_examples`.
#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::field_reassign_with_default)]

use prose_yew::prose_core::edit::{EditRules, read_model};
use prose_yew::prose_core::profile::Wording;
use prose_yew::prose_core::validate::{Options, Validator};
use prose_yew::{
    EditConfig, EditMode, OdrlProse, OdrlProseView, ProseProblems, SharedWording, StrictEdit,
    use_prose_editor, use_prose_editor_with, use_validation,
};
use std::rc::Rc;
use std::sync::Arc;
use yew::prelude::*;

const POLICY: &str = include_str!("../../prose-core/tests/fixtures/offer.json");

async fn ssr<C: BaseComponent>() -> String
where
    C::Properties: Default,
{
    let local = tokio::task::LocalSet::new();
    local
        .run_until(yew::LocalServerRenderer::<C>::new().render())
        .await
}

// region: reader
#[function_component(Reader)]
fn reader() -> Html {
    html! { <OdrlProse json={POLICY} /> }
}
// endregion: reader

// region: reader_wording
#[function_component(ReaderWithWording)]
fn reader_with_wording() -> Html {
    let wording = SharedWording(Rc::new(Wording::new()));
    html! { <OdrlProse json={POLICY} wording={Some(wording)} glossary={false} class="my-prose" /> }
}
// endregion: reader_wording

// region: reader_violations
#[function_component(ReaderWithViolations)]
fn reader_with_violations() -> Html {
    let wording = Rc::new(Wording::new());
    let validator = Validator::new(Arc::new((*wording).clone()), Options::strict());
    html! {
        <OdrlProse json={POLICY} wording={Some(SharedWording(wording))}
                   validator={Some(validator)} violations={true} />
    }
}
// endregion: reader_violations

// region: read_view
#[function_component(ReadOnlyView)]
fn read_only_view() -> Html {
    let doc = use_memo((), |_| read_model(POLICY).unwrap_or_default());
    html! { <OdrlProseView doc={doc} /> }
}
// endregion: read_view

// region: editor
#[function_component(Editor)]
fn editor() -> Html {
    let rules = use_memo((), |_| EditRules::default());
    let config = use_memo((), |_| EditConfig::default());
    let editor = use_prose_editor(|| read_model(POLICY).unwrap_or_default(), rules);
    html! {
        <OdrlProseView doc={editor.doc.clone()} mode={EditMode::Edit}
                       config={config} onedit={editor.onedit.clone()} />
    }
}
// endregion: editor

// region: own_css
#[function_component(OwnCss)]
fn own_css() -> Html {
    let rules = use_memo((), |_| EditRules::default());
    let config = use_memo((), |_| {
        let mut c = EditConfig::default();
        c.base_css = false; // no <style> in the output
        c.classes.add = classes!("pf-v5-c-button", "pf-m-link"); // next to prose-add
        c.classes.root = classes!("my-root");
        c.styles.slot = Some("border-bottom: 1px solid grey".into());
        c
    });
    let editor = use_prose_editor(|| read_model(POLICY).unwrap_or_default(), rules);
    html! {
        <OdrlProseView doc={editor.doc.clone()} mode={EditMode::Edit} config={config}
                       onedit={editor.onedit.clone()} class="extra" />
    }
}
// endregion: own_css

// region: strict
#[function_component(Strict)]
fn strict() -> Html {
    let cfg = Rc::new(
        EditConfig::default()
            .with_wording(Rc::new(Wording::new()))
            .with_strict(Options::strict()),
    );
    let with = use_prose_editor_with(
        || read_model(POLICY).unwrap_or_default(),
        Rc::new(cfg.rules.clone()),
        cfg.validator.clone(),
        StrictEdit::Flag,
    );
    let report = use_validation(&with.editor.doc, cfg.validator.as_ref());
    html! {
        <>
            <OdrlProseView doc={with.editor.doc.clone()} mode={EditMode::Edit} config={cfg}
                           onedit={with.editor.onedit.clone()}
                           refusal={with.refused.as_ref().and_then(|(ev, err)| err.issue(ev))} />
            <ProseProblems report={report} onfix={with.apply_all.clone()} />
        </>
    }
}
// endregion: strict

#[tokio::test]
async fn reader_renders_the_article() {
    let h = ssr::<Reader>().await;
    assert!(h.contains(r#"<article class="ds-prose">"#), "{h}");
    assert!(h.contains("prose-rule-sentence"));
    assert!(!h.contains("<style"), "the read-only component emits no CSS");
}

#[tokio::test]
async fn reader_with_wording_adds_the_class_after_ds_prose() {
    let h = ssr::<ReaderWithWording>().await;
    assert!(h.contains(r#"<article class="ds-prose my-prose">"#), "{h}");
}

#[tokio::test]
async fn reader_with_violations_adds_the_section_after_the_policies() {
    let h = ssr::<ReaderWithViolations>().await;
    let policies = h.find("</section>").expect("a policy section");
    let violations = h.find(r#"<section class="prose-violations">"#).expect("violations section");
    assert!(violations > policies, "{h}");
}

#[tokio::test]
async fn read_view_has_no_edit_class_and_no_style() {
    let h = ssr::<ReadOnlyView>().await;
    assert!(h.contains(r#"class="ds-prose prose-view""#), "{h}");
    assert!(!h.contains("prose-edit") && !h.contains("<style"));
}

#[tokio::test]
async fn editor_puts_base_css_first_in_the_root() {
    let h = ssr::<Editor>().await;
    assert!(h.contains("prose-edit"), "{h}");
    // Yew's server renderer escapes the text of <style>; the element is there.
    assert!(h.contains("<style>"), "base_css is on by default");
    assert!(h.contains(r#"contenteditable="plaintext-only""#));
}

#[tokio::test]
async fn own_css_turns_base_css_off_and_adds_classes_next_to_the_built_in_ones() {
    let h = ssr::<OwnCss>().await;
    assert!(!h.contains("<style"), "{h}");
    assert!(h.contains("class=\"ds-prose prose-view my-root prose-edit extra\""), "{h}");
    assert!(h.contains("prose-add pf-v5-c-button pf-m-link"), "{h}");
    assert!(h.contains("border-bottom: 1px solid grey"));
}

#[tokio::test]
async fn strict_renders_the_view_and_the_problems_panel() {
    let h = ssr::<Strict>().await;
    assert!(h.contains("prose-edit") && h.contains(r#"<section class="prose-problems"#), "{h}");
}

async fn read_with(validator: bool, violations: bool) -> String {
    let wording = Rc::new(Wording::new());
    let validator = validator.then(|| Validator::new(Arc::new((*wording).clone()), Options::strict()));
    let props = yew::props!(prose_yew::OdrlProseProps {
        json: AttrValue::from(POLICY),
        validator,
        violations
    });
    let local = tokio::task::LocalSet::new();
    local
        .run_until(yew::LocalServerRenderer::<OdrlProse>::with_props(props).render())
        .await
}

#[tokio::test]
async fn violations_need_a_validator_and_the_flag() {
    let plain = read_with(false, false).await;
    assert!(!plain.contains("prose-violations"));
    assert_eq!(read_with(true, false).await, plain, "a validator alone changes nothing");
    assert_eq!(read_with(false, true).await, plain, "the flag alone changes nothing");
    assert!(read_with(true, true).await.contains(r#"<section class="prose-violations">"#));
}

#[test]
fn base_css_is_scoped_to_the_editor_root() {
    assert!(prose_yew::BASE_CSS.contains(".ds-prose.prose-edit"));
}

// --- the home page -------------------------------------------------------------

fn site_example(name: &str) -> String {
    format!("{}/../site/content/examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// The renderer's output as the site stores it: without Yew's markers.
fn without_markers(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(at) = rest.find("<!--") {
        out.push_str(&rest[..at]);
        let end = rest[at..].find("-->").expect("unclosed comment") + at + 3;
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn check_or_write(name: &str, want: &str) {
    let path = site_example(name);
    if std::env::var("WRITE_SITE_EXAMPLES").is_ok() {
        std::fs::write(&path, want).unwrap();
    }
    let have = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{path}: {e} (WRITE_SITE_EXAMPLES=1 creates it)"));
    assert_eq!(have, want, "{name} is stale: regenerate with WRITE_SITE_EXAMPLES=1");
}

#[test]
fn the_home_page_policy_is_the_repository_fixture() {
    check_or_write("offer.jsonld", POLICY);
}

#[tokio::test]
async fn the_home_page_sentences_are_what_the_renderer_produces() {
    let h = ssr::<Reader>().await;
    check_or_write("offer.html", &without_markers(&h));
}
