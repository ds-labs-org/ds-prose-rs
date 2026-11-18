//! `validate`, `rules`, `ProseValidator` and `ProseHandle`'s
//! `setProfiles` / `setValidation` / `report`, called the way a JS host
//! calls them, in headless Chrome. Run with `cargo test -p prose-wasm
//! --target wasm32-unknown-unknown --test validate_wasm`.
#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use js_sys::{Array, JSON};
use prose_wasm::{ProseHandle, ProseValidator, ValidateOptionsArg, rules, validate};
use serde_json::Value;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;
use web_sys::Element;

mod common;
use common::{CLASS_SHAPES, chain_profile};

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

/// Options as the host passes them.
fn opts(json: &str) -> ValidateOptionsArg {
    js(json).unchecked_into()
}

/// The report text, if there is one (`null` is none).
fn report_of(h: &ProseHandle) -> Option<String> {
    h.report().as_string()
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
    let one = validate(CLEAN, JsValue::from_str(RESEARCH), None, None, None, None).unwrap();
    let many = validate(CLEAN, list(&[RESEARCH]), None, None, None, None).unwrap();
    assert_eq!(one, many);
    assert_eq!(parsed(&one)["verdict"], "conforms");
    let none = parsed(&validate(NOT_LOADED, JsValue::NULL, None, None, None, None).unwrap());
    assert_eq!(none["verdict"], "incomplete");
    assert_eq!(none["gaps"][0]["rule"], "profile.not-loaded");
}

#[wasm_bindgen_test]
fn validate_reads_options() {
    let o = opts(r#"{"strict": false}"#);
    let r = parsed(
        &validate(
            CLEAN,
            JsValue::from_str(RESEARCH),
            Some(o),
            None,
            None,
            None,
        )
        .unwrap(),
    );
    assert_eq!(r["mode"], "advisory");
    let o = opts(r#"{"structural": true, "rules": {"odrl.no-rule": "off"}}"#);
    let r = parsed(
        &validate(
            CLEAN,
            JsValue::from_str(RESEARCH),
            Some(o),
            None,
            None,
            None,
        )
        .unwrap(),
    );
    assert_eq!(r["mode"], "strict");
    assert!(r["tiers"].as_array().unwrap().iter().any(|t| t == "odrl"));
}

#[wasm_bindgen_test]
fn two_profiles_conform_when_both_are_loaded() {
    let r = parsed(&validate(MIXED, list(&[RESEARCH, OTHER]), None, None, None, None).unwrap());
    assert_eq!(r["verdict"], "conforms", "{r}");
    let r = parsed(&validate(MIXED, list(&[RESEARCH]), None, None, None, None).unwrap());
    assert_ne!(r["verdict"], "conforms");
}

#[wasm_bindgen_test]
fn bad_input_throws_and_a_bad_policy_does_not() {
    assert!(
        validate(
            CLEAN,
            JsValue::from_str("<!doctype html><p>hi"),
            None,
            None,
            None,
            None
        )
        .is_err()
    );
    assert!(validate(CLEAN, JsValue::from_f64(3.0), None, None, None, None).is_err());
    assert!(
        validate(
            CLEAN,
            JsValue::NULL,
            Some(opts(r#"{"strictt": 1}"#)),
            None,
            None,
            None
        )
        .is_err()
    );
    assert!(
        validate(
            CLEAN,
            JsValue::NULL,
            Some(JsValue::from_str("strict").unchecked_into()),
            None,
            None,
            None
        )
        .is_err()
    );
    let r = parsed(&validate("not json at all", JsValue::NULL, None, None, None, None).unwrap());
    assert_eq!(r["findings"][0]["rule"], "input.unreadable");
    // Undefined and null options are the defaults.
    assert!(
        validate(
            CLEAN,
            JsValue::NULL,
            Some(JsValue::UNDEFINED.unchecked_into()),
            None,
            None,
            None
        )
        .is_ok()
    );
    assert!(
        validate(
            CLEAN,
            JsValue::NULL,
            Some(JsValue::NULL.unchecked_into()),
            None,
            None,
            None
        )
        .is_ok()
    );
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
    assert!(ProseValidator::new(list(&[RESEARCH]), Some(opts(r#"{"ruleset": 99}"#))).is_err());
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
    assert!(report_of(&h).is_none());
    h.set_validation(JsValue::NULL).unwrap();
    assert!(report_of(&h).is_none());
    h.set_profiles(list(&[RESEARCH])).unwrap();
    assert!(
        report_of(&h).is_none(),
        "profiles alone do not turn validation on"
    );
    h.set_validation(js("{}")).unwrap();
    let r = parsed(&report_of(&h).expect("a report"));
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
    assert_eq!(parsed(&report_of(&h).unwrap())["verdict"], "incomplete");
    h.set_profiles(list(&[RESEARCH])).unwrap();
    assert_eq!(parsed(&report_of(&h).unwrap())["verdict"], "incomplete");
    h.update(CLEAN, None);
    assert_eq!(parsed(&report_of(&h).unwrap())["verdict"], "conforms");
    // The single-profile call feeds the same report.
    h.set_profile(None).unwrap();
    assert_ne!(parsed(&report_of(&h).unwrap())["verdict"], "conforms");
    h.set_profile(Some(RESEARCH.to_string())).unwrap();
    assert_eq!(parsed(&report_of(&h).unwrap())["verdict"], "conforms");
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
    let before = report_of(&h).unwrap();
    assert!(has(&c, ".prose-violations"));
    assert!(h.set_validation(js(r#"{"odrl": "all"}"#)).is_err());
    assert!(h.set_validation(JsValue::from_str("strict")).is_err());
    assert!(h.set_profiles(list(&[RESEARCH, "{"])).is_err());
    assert!(
        h.set_profiles(JsValue::from_str(RESEARCH)).is_err(),
        "a list is required"
    );
    settle().await;
    assert_eq!(
        report_of(&h).unwrap(),
        before,
        "options and profiles are kept"
    );
    assert!(has(&c, ".prose-violations"));
}

#[wasm_bindgen_test]
async fn destroyed_handle_stays_harmless() {
    let (c, mut h) = mount(CLEAN).await;
    h.destroy();
    h.set_profiles(list(&[RESEARCH])).unwrap();
    h.set_validation(js("{}")).unwrap();
    assert!(report_of(&h).is_some());
    settle().await;
    assert_eq!(c.child_element_count(), 0);
}

// --- companion SHACL shapes (setShapes) ------------------------------------------

const DX_PROFILE: &str = include_str!("../../prose-core/tests/shapes/dataspace-profile.jsonld");
const DX_SHAPES: &str = include_str!("../../prose-core/tests/shapes/dataspace-shapes.jsonld");
const DX_POLICY: &str = r#"{"@context":["http://www.w3.org/ns/odrl.jsonld",{"dx":"https://example.org/dataspace/"}],"@type":"Set","uid":"urn:p","profile":"https://example.org/dataspace/profile","permission":[{"target":"urn:a","action":"dx:train","constraint":[{"leftOperand":"dx:purpose","operator":"eq","rightOperand":{"@id":"dx:clinical"}}]}]}"#;

#[wasm_bindgen_test]
fn a_validator_takes_shapes_and_drops_them() {
    let mut v = ProseValidator::new(list(&[DX_PROFILE]), None).unwrap();
    let clean: Value = serde_json::from_str(&v.validate(DX_POLICY)).unwrap();
    assert_eq!(clean["verdict"], "conforms");
    let summary = v.set_shapes(Some(DX_SHAPES.to_string())).unwrap();
    assert!(summary.contains("\"constraints\":5"), "{summary}");
    let report: Value = serde_json::from_str(&v.validate(DX_POLICY)).unwrap();
    assert_eq!(report["verdict"], "violations");
    assert_eq!(report["findings"][0]["expected"]["source"], "shapes");
    // Unreadable shapes throw and keep the previous.
    assert!(v.set_shapes(Some("{".to_string())).is_err());
    assert_eq!(
        serde_json::from_str::<Value>(&v.validate(DX_POLICY)).unwrap()["verdict"],
        "violations"
    );
    assert_eq!(v.set_shapes(None).unwrap(), "null");
    assert_eq!(
        serde_json::from_str::<Value>(&v.validate(DX_POLICY)).unwrap()["verdict"],
        "conforms"
    );
}

#[wasm_bindgen_test]
async fn the_rendering_handle_validates_with_shapes_too() {
    let (c, mut h) = mount(DX_POLICY).await;
    h.set_profiles(list(&[DX_PROFILE])).unwrap();
    h.set_validation(js("{}")).unwrap();
    settle().await;
    assert_eq!(
        serde_json::from_str::<Value>(&report_of(&h).unwrap()).unwrap()["verdict"],
        "conforms"
    );
    h.set_shapes(Some(DX_SHAPES.to_string())).unwrap();
    settle().await;
    let report: Value = serde_json::from_str(&report_of(&h).unwrap()).unwrap();
    assert_eq!(report["verdict"], "violations", "{}", c.inner_html());
    assert_eq!(report["findings"][0]["expected"]["source"], "shapes");
    // New profiles keep the shapes.
    h.set_profiles(list(&[DX_PROFILE])).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&report_of(&h).unwrap()).unwrap()["verdict"],
        "violations"
    );
    assert_eq!(h.set_shapes(None).unwrap(), "null");
    assert_eq!(
        serde_json::from_str::<Value>(&report_of(&h).unwrap()).unwrap()["verdict"],
        "conforms"
    );
}

#[wasm_bindgen_test]
fn validate_takes_shapes_as_its_last_argument() {
    let report = |shapes: Option<String>| -> Value {
        serde_json::from_str(
            &validate(DX_POLICY, list(&[DX_PROFILE]), None, shapes, None, None).expect("validates"),
        )
        .unwrap()
    };
    assert_eq!(report(None)["verdict"], "conforms");
    let with = report(Some(DX_SHAPES.to_string()));
    assert_eq!(with["verdict"], "violations");
    assert_eq!(with["findings"][0]["expected"]["source"], "shapes");
    assert!(
        validate(
            DX_POLICY,
            list(&[DX_PROFILE]),
            None,
            Some("{".to_string()),
            None,
            None
        )
        .is_err()
    );
}

// --- operator overlays (setOverlays) -----------------------------------------------

const DX_OVERLAY: &str = include_str!("../../prose-core/tests/overlay/dataspace-overlay.jsonld");

fn overlays(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[wasm_bindgen_test]
fn a_validator_takes_overlays_and_drops_them() {
    let mut v = ProseValidator::new(list(&[DX_PROFILE]), None).unwrap();
    assert_eq!(parsed(&v.validate(DX_POLICY))["verdict"], "conforms");
    let summary: Value =
        serde_json::from_str(&v.set_overlays(overlays(&[DX_OVERLAY])).unwrap()).unwrap();
    assert_eq!(summary[0]["terms"], 3, "{summary}");
    let report = parsed(&v.validate(DX_POLICY));
    assert_eq!(report["verdict"], "violations");
    assert_eq!(report["findings"][0]["expected"]["source"], "overlay");
    // Unreadable overlays throw and keep the previous.
    assert!(v.set_overlays(overlays(&["{"])).is_err());
    assert_eq!(parsed(&v.validate(DX_POLICY))["verdict"], "violations");
    // Shapes laid over the overlay win, and are kept when the overlays change.
    v.set_shapes(Some(DX_SHAPES.to_string())).unwrap();
    assert_eq!(
        parsed(&v.validate(DX_POLICY))["findings"][0]["expected"]["source"],
        "shapes"
    );
    assert_eq!(v.set_overlays(Vec::new()).unwrap(), "[]");
    assert_eq!(
        parsed(&v.validate(DX_POLICY))["findings"][0]["expected"]["source"],
        "shapes"
    );
}

#[wasm_bindgen_test]
async fn the_rendering_handle_validates_with_overlays_too() {
    let (c, mut h) = mount(DX_POLICY).await;
    h.set_profiles(list(&[DX_PROFILE])).unwrap();
    h.set_validation(js("{}")).unwrap();
    settle().await;
    assert_eq!(parsed(&report_of(&h).unwrap())["verdict"], "conforms");
    h.set_overlays(overlays(&[DX_OVERLAY])).unwrap();
    settle().await;
    let report = parsed(&report_of(&h).unwrap());
    assert_eq!(report["verdict"], "violations", "{}", c.inner_html());
    assert_eq!(report["findings"][0]["expected"]["source"], "overlay");
    // New profiles keep the overlays.
    h.set_profiles(list(&[DX_PROFILE])).unwrap();
    assert_eq!(parsed(&report_of(&h).unwrap())["verdict"], "violations");
    assert_eq!(h.set_overlays(Vec::new()).unwrap(), "[]");
    assert_eq!(parsed(&report_of(&h).unwrap())["verdict"], "conforms");
    assert!(h.set_overlays(overlays(&["{"])).is_err());
}

#[wasm_bindgen_test]
fn validate_takes_overlays_as_its_last_argument() {
    let report = |o: Option<Vec<String>>| -> Value {
        parsed(&validate(DX_POLICY, list(&[DX_PROFILE]), None, None, o, None).expect("validates"))
    };
    assert_eq!(report(None)["verdict"], "conforms");
    let with = report(Some(overlays(&[DX_OVERLAY])));
    assert_eq!(with["verdict"], "violations");
    assert_eq!(with["findings"][0]["expected"]["source"], "overlay");
    assert!(
        validate(
            DX_POLICY,
            list(&[DX_PROFILE]),
            None,
            None,
            Some(overlays(&["{"])),
            None
        )
        .is_err()
    );
}

#[wasm_bindgen_test]
async fn overlay_summaries_are_recomputed_when_the_profiles_change() {
    let (_c, mut h) = mount(DX_POLICY).await;
    let warnings = |h: &ProseHandle| -> Value {
        serde_json::from_str::<Value>(&h.overlay_summaries()).unwrap()[0]["warnings"].clone()
    };
    assert_eq!(h.overlay_summaries(), "[]");
    // No profile yet: every term the overlay names is undeclared.
    let at_load: Value =
        serde_json::from_str(&h.set_overlays(overlays(&[DX_OVERLAY])).unwrap()).unwrap();
    assert!(
        !at_load[0]["warnings"].as_array().unwrap().is_empty(),
        "{at_load}"
    );
    assert_eq!(warnings(&h), at_load[0]["warnings"]);
    // The profile declares them: the stale warnings are gone.
    h.set_profiles(list(&[DX_PROFILE])).unwrap();
    assert_eq!(warnings(&h), serde_json::json!([]));
    // And back.
    h.set_profiles(list(&[])).unwrap();
    assert_eq!(warnings(&h), at_load[0]["warnings"]);
}

// --- runtime value sources (setValueSource, setValueSources) ---------------------------

const DX_PURPOSE: &str = "https://example.org/dataspace/purpose";
const DX_VALUES: &str = r#"[{"value":"https://example.org/dataspace/research","label":"Research","group":"Purposes"},"https://example.org/dataspace/marketing"]"#;
const DX_SOURCES: &str = r#"{"https://example.org/dataspace/purpose":{"values":[{"value":"https://example.org/dataspace/research","label":"Research","group":"Purposes"},"https://example.org/dataspace/marketing"],"closed":true}}"#;

#[wasm_bindgen_test]
fn a_validator_takes_one_value_source_and_drops_it() {
    let mut v = ProseValidator::new(list(&[DX_PROFILE]), None).unwrap();
    assert_eq!(parsed(&v.validate(DX_POLICY))["verdict"], "conforms");
    let summary = parsed(
        &v.set_value_source(DX_PURPOSE, js(DX_VALUES), Some(js(r#"{"closed":true}"#)))
            .unwrap(),
    );
    assert_eq!(summary["values"], 2, "{summary}");
    assert_eq!(summary["closed"], true);
    let report = parsed(&v.validate(DX_POLICY));
    assert_eq!(report["verdict"], "violations");
    assert_eq!(report["findings"][0]["expected"]["source"], "host");
    // Suggest-only (no options) flags no error.
    v.set_value_source(DX_PURPOSE, js(DX_VALUES), None).unwrap();
    assert_ne!(parsed(&v.validate(DX_POLICY))["verdict"], "violations");
    // Unreadable values throw and keep the previous source.
    v.set_value_source(DX_PURPOSE, js(DX_VALUES), Some(js(r#"{"closed":true}"#)))
        .unwrap();
    assert!(v.set_value_source(DX_PURPOSE, js("[1]"), None).is_err());
    assert!(
        v.set_value_source(DX_PURPOSE, js(DX_VALUES), Some(js(r#"{"closd":true}"#)))
            .is_err()
    );
    assert_eq!(parsed(&v.validate(DX_POLICY))["verdict"], "violations");
    // null clears it.
    assert_eq!(
        v.set_value_source(DX_PURPOSE, JsValue::NULL, None).unwrap(),
        "null"
    );
    assert_eq!(parsed(&v.validate(DX_POLICY))["verdict"], "conforms");
}

#[wasm_bindgen_test]
fn a_validator_takes_all_value_sources_at_once_and_keeps_the_shapes_and_overlays() {
    let mut v = ProseValidator::new(list(&[DX_PROFILE]), None).unwrap();
    let summary = parsed(&v.set_value_sources(js(DX_SOURCES)).unwrap());
    assert_eq!(summary[DX_PURPOSE]["values"], 2, "{summary}");
    assert_eq!(parsed(&v.validate(DX_POLICY))["verdict"], "violations");
    // The shapes are laid under the host: the host still names itself.
    v.set_shapes(Some(DX_SHAPES.to_string())).unwrap();
    assert_eq!(
        parsed(&v.validate(DX_POLICY))["findings"][0]["expected"]["source"],
        "host"
    );
    assert!(v.set_value_sources(js(r#"{"k": 3}"#)).is_err());
    assert_eq!(
        parsed(&v.validate(DX_POLICY))["findings"][0]["expected"]["source"],
        "host",
        "an unreadable map keeps the previous sources"
    );
    assert_eq!(v.set_value_sources(JsValue::NULL).unwrap(), "{}");
    assert_eq!(
        parsed(&v.validate(DX_POLICY))["findings"][0]["expected"]["source"],
        "shapes"
    );
}

#[wasm_bindgen_test]
async fn the_rendering_handle_validates_with_value_sources_too() {
    let (c, mut h) = mount(DX_POLICY).await;
    h.set_profiles(list(&[DX_PROFILE])).unwrap();
    h.set_validation(js("{}")).unwrap();
    settle().await;
    assert_eq!(parsed(&report_of(&h).unwrap())["verdict"], "conforms");
    h.set_value_sources(js(DX_SOURCES)).unwrap();
    settle().await;
    let report = parsed(&report_of(&h).unwrap());
    assert_eq!(report["verdict"], "violations", "{}", c.inner_html());
    assert_eq!(report["findings"][0]["expected"]["source"], "host");
    // New profiles keep the sources.
    h.set_profiles(list(&[DX_PROFILE])).unwrap();
    assert_eq!(parsed(&report_of(&h).unwrap())["verdict"], "violations");
    // The summaries follow the profiles loaded now.
    let summaries = parsed(&h.value_source_summaries());
    assert_eq!(summaries[DX_PURPOSE]["warnings"], serde_json::json!([]));
    h.set_value_source(DX_PURPOSE, JsValue::NULL, None).unwrap();
    assert_eq!(parsed(&report_of(&h).unwrap())["verdict"], "conforms");
    assert!(h.set_value_source(DX_PURPOSE, js("[1]"), None).is_err());
}

#[wasm_bindgen_test]
fn validate_takes_value_sources_as_its_last_argument() {
    let report = |s: Option<JsValue>| -> Value {
        parsed(&validate(DX_POLICY, list(&[DX_PROFILE]), None, None, None, s).expect("validates"))
    };
    assert_eq!(report(None)["verdict"], "conforms");
    assert_eq!(report(Some(JsValue::NULL))["verdict"], "conforms");
    let with = report(Some(js(DX_SOURCES)));
    assert_eq!(with["verdict"], "violations");
    assert_eq!(with["findings"][0]["expected"]["source"], "host");
    assert!(
        validate(
            DX_POLICY,
            list(&[DX_PROFILE]),
            None,
            None,
            None,
            Some(js(r#"{"k": 3}"#))
        )
        .is_err()
    );
}

fn warns_of_the_cut_class(summary: &str) -> bool {
    let v: Value = serde_json::from_str(summary).unwrap();
    v["warnings"].as_array().unwrap().iter().any(|m| {
        let m = m.as_str().unwrap();
        m.contains("sh:class") && m.contains("more members")
    })
}

#[wasm_bindgen_test]
fn a_validator_reports_what_the_profiles_make_unusable_in_the_shapes_summary() {
    let mut v = ProseValidator::new(list(&[&chain_profile(70)]), None).unwrap();
    let summary = v.set_shapes(Some(CLASS_SHAPES.to_string())).unwrap();
    assert!(warns_of_the_cut_class(&summary), "{summary}");
    let mut ok = ProseValidator::new(list(&[&chain_profile(5)]), None).unwrap();
    let summary = ok.set_shapes(Some(CLASS_SHAPES.to_string())).unwrap();
    assert!(!warns_of_the_cut_class(&summary), "{summary}");
}

#[wasm_bindgen_test]
async fn the_rendering_handle_reports_what_the_profiles_make_unusable_in_the_shapes_summary() {
    let (_c, mut h) = mount(POLICY).await;
    h.set_profiles(list(&[&chain_profile(70)])).unwrap();
    let summary = h.set_shapes(Some(CLASS_SHAPES.to_string())).unwrap();
    assert!(warns_of_the_cut_class(&summary), "{summary}");
    // New profiles change what the shapes can say; shapesSummary follows.
    h.set_profiles(list(&[&chain_profile(5)])).unwrap();
    assert!(!warns_of_the_cut_class(&h.shapes_summary()));
    h.set_profiles(list(&[&chain_profile(70)])).unwrap();
    assert!(warns_of_the_cut_class(&h.shapes_summary()));
    h.set_shapes(None).unwrap();
    assert_eq!(h.shapes_summary(), "null");
}
