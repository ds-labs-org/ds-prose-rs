//! `validate`, `rules`, `ProseValidator` and `ProseHandle`'s
//! `setProfiles` / `setValidation` / `report`, called the way a JS host
//! calls them, in headless Chrome. Run with `cargo test -p prose-wasm
//! --target wasm32-unknown-unknown --test validate_wasm`.
#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use js_sys::{Array, JSON};
use prose_wasm::{ProseHandle, ProseValidator, rules, validate};
use serde_json::Value;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;
use web_sys::Element;

wasm_bindgen_test_configure!(run_in_browser);

const RESEARCH: &str = include_str!("../../prose-core/tests/profiles/research.jsonld");
const OTHER: &str = include_str!("../../prose-core/tests/profiles/other.jsonld");
const CLEAN: &str = include_str!("../../prose-core/tests/strict/clean.json");
const NOT_LOADED: &str = include_str!("../../prose-core/tests/strict/not-loaded.json");
const MIXED: &str = include_str!("../../prose-core/tests/strict/mixed-two-profiles.json");
const POLICY: &str = include_str!("../../prose-core/tests/profiles/research-policy.json");

async fn settle() {
    for _ in 0..4 {
        TimeoutFuture::new(0).await;
    }
}

async fn mount(json: &str) -> (Element, ProseHandle) {
    let document = web_sys::window().unwrap().document().unwrap();
    let c = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&c).unwrap();
    let h = ProseHandle::new(c.clone(), json, None);
    settle().await;
    (c, h)
}

fn js(json: &str) -> JsValue {
    JSON::parse(json).expect("test JSON")
}

fn list(items: &[&str]) -> JsValue {
    items
        .iter()
        .map(|s| JsValue::from_str(s))
        .collect::<Array>()
        .into()
}

fn parsed(text: &str) -> Value {
    serde_json::from_str(text).expect("JSON text")
}

fn has(c: &Element, selector: &str) -> bool {
    c.query_selector(selector).unwrap().is_some()
}

#[wasm_bindgen_test]
fn validate_takes_a_string_a_list_or_null() {
    let one = validate(CLEAN, JsValue::from_str(RESEARCH), None).unwrap();
    let many = validate(CLEAN, list(&[RESEARCH]), None).unwrap();
    assert_eq!(one, many);
    assert_eq!(parsed(&one)["verdict"], "conforms");
    let none = parsed(&validate(NOT_LOADED, JsValue::NULL, None).unwrap());
    assert_eq!(none["verdict"], "incomplete");
    assert_eq!(none["gaps"][0]["rule"], "profile.not-loaded");
}

#[wasm_bindgen_test]
fn validate_reads_options() {
    let o = js(r#"{"strict": false}"#);
    let r = parsed(&validate(CLEAN, JsValue::from_str(RESEARCH), Some(o)).unwrap());
    assert_eq!(r["mode"], "advisory");
    let o = js(r#"{"structural": true, "rules": {"odrl.no-rule": "off"}}"#);
    let r = parsed(&validate(CLEAN, JsValue::from_str(RESEARCH), Some(o)).unwrap());
    assert_eq!(r["mode"], "strict");
    assert!(r["tiers"].as_array().unwrap().iter().any(|t| t == "odrl"));
}

#[wasm_bindgen_test]
fn two_profiles_conform_when_both_are_loaded() {
    let r = parsed(&validate(MIXED, list(&[RESEARCH, OTHER]), None).unwrap());
    assert_eq!(r["verdict"], "conforms", "{r}");
    let r = parsed(&validate(MIXED, list(&[RESEARCH]), None).unwrap());
    assert_ne!(r["verdict"], "conforms");
}

#[wasm_bindgen_test]
fn bad_input_throws_and_a_bad_policy_does_not() {
    assert!(validate(CLEAN, JsValue::from_str("<!doctype html><p>hi"), None).is_err());
    assert!(validate(CLEAN, JsValue::from_f64(3.0), None).is_err());
    assert!(validate(CLEAN, JsValue::NULL, Some(js(r#"{"strictt": 1}"#))).is_err());
    assert!(validate(CLEAN, JsValue::NULL, Some(JsValue::from_str("strict"))).is_err());
    let r = parsed(&validate("not json at all", JsValue::NULL, None).unwrap());
    assert_eq!(r["findings"][0]["rule"], "input.unreadable");
    // Undefined and null options are the defaults.
    assert!(validate(CLEAN, JsValue::NULL, Some(JsValue::UNDEFINED)).is_ok());
    assert!(validate(CLEAN, JsValue::NULL, Some(JsValue::NULL)).is_ok());
}

#[wasm_bindgen_test]
fn rules_lists_the_catalogue() {
    let r = parsed(&rules());
    assert!(r.as_array().unwrap().len() >= 30);
    assert!(
        r.as_array()
            .unwrap()
            .iter()
            .any(|x| x["id"] == "profile.unknown-term")
    );
}

#[wasm_bindgen_test]
fn validator_parses_profiles_once_and_validates_many() {
    let v = ProseValidator::new(list(&[RESEARCH]), None).unwrap();
    assert_eq!(parsed(&v.validate(CLEAN))["verdict"], "conforms");
    assert_eq!(parsed(&v.validate(NOT_LOADED))["verdict"], "incomplete");
    assert_eq!(parsed(&v.validate("{")).findings_rule(), "input.unreadable");
    assert!(ProseValidator::new(list(&["{"]), None).is_err());
    assert!(ProseValidator::new(list(&[RESEARCH]), Some(js(r#"{"ruleset": 99}"#))).is_err());
    // Several profiles, and none.
    assert!(ProseValidator::new(list(&[]), None).is_ok());
    let both = ProseValidator::new(list(&[RESEARCH, OTHER]), None).unwrap();
    assert_eq!(parsed(&both.validate(MIXED))["verdict"], "conforms");
}

trait FirstRule {
    fn findings_rule(&self) -> &str;
}
impl FirstRule for Value {
    fn findings_rule(&self) -> &str {
        self["findings"][0]["rule"].as_str().unwrap_or("")
    }
}

#[wasm_bindgen_test]
async fn report_is_null_until_validation_is_on() {
    let (_c, mut h) = mount(CLEAN).await;
    assert!(h.report().is_none());
    h.set_validation(JsValue::NULL).unwrap();
    assert!(h.report().is_none());
    h.set_profiles(list(&[RESEARCH])).unwrap();
    assert!(
        h.report().is_none(),
        "profiles alone do not turn validation on"
    );
    h.set_validation(js("{}")).unwrap();
    let r = parsed(&h.report().expect("a report"));
    assert_eq!(r["schema"], "ds-prose/validation-report@1");
    assert_eq!(r["verdict"], "conforms");
}

#[wasm_bindgen_test]
async fn set_validation_shows_the_violations_section() {
    let (c, mut h) = mount(NOT_LOADED).await;
    assert!(!has(&c, ".prose-violations"));
    h.set_validation(js("{}")).unwrap();
    settle().await;
    assert!(has(&c, ".prose-violations"), "{}", c.inner_html());
    assert!(has(&c, "[role=status]"));
    h.set_validation(JsValue::NULL).unwrap();
    settle().await;
    assert!(
        !has(&c, ".prose-violations"),
        "null switches validation off"
    );
}

#[wasm_bindgen_test]
async fn report_follows_update_and_profiles() {
    let (_c, mut h) = mount(NOT_LOADED).await;
    h.set_validation(js("{}")).unwrap();
    assert_eq!(parsed(&h.report().unwrap())["verdict"], "incomplete");
    h.set_profiles(list(&[RESEARCH])).unwrap();
    assert_eq!(parsed(&h.report().unwrap())["verdict"], "incomplete");
    h.update(CLEAN, None);
    assert_eq!(parsed(&h.report().unwrap())["verdict"], "conforms");
    // The single-profile call feeds the same report.
    h.set_profile(None).unwrap();
    assert_ne!(parsed(&h.report().unwrap())["verdict"], "conforms");
    h.set_profile(Some(RESEARCH.to_string())).unwrap();
    assert_eq!(parsed(&h.report().unwrap())["verdict"], "conforms");
}

#[wasm_bindgen_test]
async fn set_profiles_returns_each_summary_and_renders_the_glossary() {
    let (c, mut h) = mount(POLICY).await;
    let s = parsed(&h.set_profiles(list(&[RESEARCH, OTHER])).unwrap());
    let s = s.as_array().expect("a list of summaries");
    assert_eq!(s.len(), 2);
    assert_eq!(s[0]["title"], "Research Data Profile");
    settle().await;
    assert!(has(&c, ".prose-glossary"));
    assert_eq!(
        parsed(&h.set_profiles(list(&[])).unwrap()),
        serde_json::json!([])
    );
    settle().await;
    assert!(!has(&c, ".prose-glossary"));
}

#[wasm_bindgen_test]
async fn bad_options_or_profiles_throw_and_keep_the_previous() {
    let (c, mut h) = mount(CLEAN).await;
    h.set_profiles(list(&[RESEARCH])).unwrap();
    h.set_validation(js(r#"{"structural": true}"#)).unwrap();
    settle().await;
    let before = h.report().unwrap();
    assert!(has(&c, ".prose-violations"));
    assert!(h.set_validation(js(r#"{"odrl": "all"}"#)).is_err());
    assert!(h.set_validation(JsValue::from_str("strict")).is_err());
    assert!(h.set_profiles(list(&[RESEARCH, "{"])).is_err());
    assert!(
        h.set_profiles(JsValue::from_str(RESEARCH)).is_err(),
        "a list is required"
    );
    settle().await;
    assert_eq!(h.report().unwrap(), before, "options and profiles are kept");
    assert!(has(&c, ".prose-violations"));
}

#[wasm_bindgen_test]
async fn destroyed_handle_stays_harmless() {
    let (c, mut h) = mount(CLEAN).await;
    h.destroy();
    h.set_profiles(list(&[RESEARCH])).unwrap();
    h.set_validation(js("{}")).unwrap();
    assert!(h.report().is_some());
    settle().await;
    assert_eq!(c.child_element_count(), 0);
}
