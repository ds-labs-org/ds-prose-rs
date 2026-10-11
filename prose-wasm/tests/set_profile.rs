//! `ProseHandle.setProfile`, mounted in headless Chrome the way a JS host
//! uses it. Run with `cargo test -p prose-wasm --target
//! wasm32-unknown-unknown --test set_profile`.
#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use prose_wasm::ProseHandle;
use wasm_bindgen_test::*;
use web_sys::Element;

wasm_bindgen_test_configure!(run_in_browser);

const RESEARCH: &str = include_str!("../../prose-core/tests/profiles/research.jsonld");
const RESEARCH_TTL: &str = include_str!("../../prose-core/tests/profiles/research.ttl");
const POLICY: &str = include_str!("../../prose-core/tests/profiles/research-policy.json");

async fn settle() {
    for _ in 0..4 {
        TimeoutFuture::new(0).await;
    }
}

async fn mount() -> (Element, ProseHandle) {
    let document = web_sys::window().unwrap().document().unwrap();
    let c = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&c).unwrap();
    let h = ProseHandle::new(c.clone(), POLICY, None);
    settle().await;
    (c, h)
}

fn has_glossary(c: &Element) -> bool {
    c.query_selector(".prose-glossary").unwrap().is_some()
}

fn text(c: &Element) -> String {
    c.text_content().unwrap_or_default()
}

#[wasm_bindgen_test]
async fn set_profile_renders_glossary() {
    let (c, mut h) = mount().await;
    assert!(!has_glossary(&c));
    assert!(text(&c).contains("fine tune"), "no profile: the local name");
    let summary = h.set_profile(Some(RESEARCH.to_string())).expect("loads");
    assert!(summary.contains("\"Research Data Profile\""), "{summary}");
    settle().await;
    assert!(has_glossary(&c), "{}", c.inner_html());
    assert!(text(&c).contains("may fine-tune"), "{}", text(&c));
}

#[wasm_bindgen_test]
async fn turtle_profile_reads_the_same() {
    let (c, mut h) = mount().await;
    h.set_profile(Some(RESEARCH_TTL.to_string()))
        .expect("Turtle is read in the npm build");
    settle().await;
    assert!(has_glossary(&c));
    assert!(text(&c).contains("may fine-tune"));
}

#[wasm_bindgen_test]
async fn update_two_args_keeps_profile() {
    let (c, mut h) = mount().await;
    h.set_profile(Some(RESEARCH.to_string())).unwrap();
    h.update(POLICY, Some("host".to_string()));
    settle().await;
    assert!(has_glossary(&c), "update does not drop the profile");
    assert!(c.query_selector("article.ds-prose.host").unwrap().is_some());
}

#[wasm_bindgen_test]
async fn set_profile_null_clears() {
    let (c, mut h) = mount().await;
    h.set_profile(Some(RESEARCH.to_string())).unwrap();
    settle().await;
    assert_eq!(h.set_profile(None).unwrap(), "null");
    settle().await;
    assert!(!has_glossary(&c));
    assert!(!text(&c).contains("may fine-tune"));
}

#[wasm_bindgen_test]
async fn bad_profile_throws_keeps_previous() {
    let (c, mut h) = mount().await;
    h.set_profile(Some(RESEARCH.to_string())).unwrap();
    settle().await;
    for bad in [
        "<!doctype html><p>a landing page</p>",
        "{\"@context\": {}}",
        "{",
    ] {
        assert!(h.set_profile(Some(bad.to_string())).is_err(), "{bad}");
        settle().await;
        assert!(has_glossary(&c), "the previous profile stays after {bad}");
    }
}

#[wasm_bindgen_test]
async fn set_profile_after_destroy_is_harmless() {
    let (c, mut h) = mount().await;
    h.destroy();
    h.set_profile(Some(RESEARCH.to_string())).unwrap();
    h.update(POLICY, None);
    settle().await;
    assert_eq!(c.child_element_count(), 0);
}
