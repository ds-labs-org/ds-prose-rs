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
