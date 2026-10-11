//! Bridge from `<OdrlProse>` (Yew) to JavaScript hosts such as Angular.
//!
//! A host owns a DOM element; [`ProseHandle`] mounts the Yew component into it,
//! re-renders it when the policy, class or profile changes, and tears it down.
//! Yew keeps its own virtual DOM inside that element, so the host must not
//! touch the element's children.
//!
//! `setProfile` reads an ODRL profile as JSON-LD or Turtle (prose-core's
//! `turtle` feature is on in this crate); the host fetches it. `setProfiles`
//! reads several, and `setValidation` adds the validator's findings to the
//! rendering.
//!
//! Validation also works without the DOM: [`validate`], [`rules`] and
//! [`ProseValidator`] are plain functions over text, so a Node script can
//! call them after `initSync`. They return the report as JSON text
//! (`ds-prose/validation-report@1`), the same bytes everywhere.
pub mod validation;
use prose_core::profile::{ProfileError, ProfileVocab, Wording};
use prose_core::validate::{Options, Validator};
use prose_yew::{OdrlProse, OdrlProseProps, SharedWording};
use serde_json::{Value, json};
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use web_sys::Element;
use yew::{AttrValue, Classes, Renderer};

fn props(
    json: &str,
    class: Option<String>,
    wording: Option<SharedWording>,
    validator: Option<Validator>,
) -> OdrlProseProps {
    OdrlProseProps {
        json: AttrValue::from(json.to_owned()),
        class: class.map(Classes::from).unwrap_or_default(),
        wording,
        glossary: true,
        violations: validator.is_some(),
        validator,
    }
}

/// A JS value as JSON, through `JSON.stringify`: `None` for `undefined`.
/// Errors when the value has no JSON form (a cycle, a BigInt, a function).
fn to_json(v: &JsValue, what: &str) -> Result<Option<Value>, String> {
    if v.is_undefined() {
        return Ok(None);
    }
    let text = js_sys::JSON::stringify(v)
        .ok()
        .and_then(|t| t.as_string())
        .ok_or_else(|| format!("{what} is not plain data"))?;
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| format!("{what} is not plain data: {e}"))
}

fn options_of(v: Option<&JsValue>) -> Result<Options, String> {
    let value = match v {
        Some(v) => to_json(v, "options")?,
        None => None,
    };
    validation::options_from_value(value.as_ref())
}

/// The profile texts of a list; unlike [`validate`], a lone string is not
/// accepted where the method says a list.
fn texts_of(v: &JsValue) -> Result<Vec<String>, String> {
    match to_json(v, "profiles")? {
        Some(l @ Value::Array(_)) => validation::profile_list(&l),
        _ => Err("profiles must be a list of strings".to_string()),
    }
}

fn throw(e: String) -> JsError {
    JsError::new(&e)
}

/// Reads `text` as a profile (JSON-LD, or Turtle: this crate turns on
/// prose-core's `turtle` feature). `None` means no profile.
pub fn read_profile(text: Option<&str>) -> Result<Option<SharedWording>, ProfileError> {
    match text {
        None => Ok(None),
        Some(t) => {
            let p = ProfileVocab::parse(t)?;
            Ok(Some(SharedWording::from(Wording::new().with_profile(p))))
        }
    }
}

/// What `setProfile` returns for a loaded profile, as JSON text: `iri`,
/// `title` and `version` (each `null` when the profile does not say),
/// `terms`, `counts` (only the kinds present, keyed in camelCase) and
/// `warnings` (what was not read, or read with a guess).
pub fn summary_json(p: &ProfileVocab) -> String {
    let s = p.summary();
    let counts: serde_json::Map<String, Value> = s
        .counts
        .iter()
        .map(|(k, n)| (camel(&format!("{k:?}")), Value::from(*n)))
        .collect();
    json!({
        "iri": s.iri,
        "title": p.title().pick(&[]),
        "version": s.version,
        "terms": s.terms,
        "counts": counts,
        "warnings": p.warnings(),
    })
    .to_string()
}

/// `LeftOperand` -> `leftOperand`.
fn camel(name: &str) -> String {
    let mut c = name.chars();
    match c.next() {
        Some(f) => f.to_ascii_lowercase().to_string() + c.as_str(),
        None => String::new(),
    }
}

/// A mounted `<OdrlProse>`. Call `destroy()` (or `free()`) when the host
/// element goes away.
#[wasm_bindgen]
pub struct ProseHandle {
    app: Option<yew::AppHandle<OdrlProse>>,
    // Kept so `update` and `setProfile` each change one input only.
    json: String,
    class: Option<String>,
    wording: Option<SharedWording>,
    // Set by `setValidation`; the validator is rebuilt from it and the
    // profiles whenever either changes.
    options: Option<Options>,
    validator: Option<Validator>,
}

#[wasm_bindgen]
impl ProseHandle {
    /// Mount into `root`. `class` is added to the component's root element.
    #[wasm_bindgen(constructor)]
    pub fn new(root: Element, json: &str, class: Option<String>) -> ProseHandle {
        let app = Renderer::<OdrlProse>::with_root_and_props(
            root,
            props(json, class.clone(), None, None),
        )
        .render();
        ProseHandle {
            app: Some(app),
            json: json.to_owned(),
            class,
            wording: None,
            options: None,
            validator: None,
        }
    }

    /// Re-render with a new policy and class. The profile is kept. No-op
    /// after `destroy()`.
    pub fn update(&mut self, json: &str, class: Option<String>) {
        self.json = json.to_owned();
        self.class = class;
        self.render();
    }

    /// Load the ODRL profile the prose is worded with: JSON-LD or Turtle
    /// text, or `null` for none. Returns the profile's summary as JSON
    /// text (`"null"` after clearing). Throws with a message that names
    /// the likely cause when the text is not a readable profile; the
    /// previous profile then stays.
    #[wasm_bindgen(js_name = setProfile)]
    pub fn set_profile(&mut self, text: Option<String>) -> Result<String, JsError> {
        let wording = read_profile(text.as_deref()).map_err(|e| JsError::new(&e.to_string()))?;
        let summary = match wording.as_ref().and_then(|w| w.0.profiles().next()) {
            Some(p) => summary_json(p),
            None => "null".to_string(),
        };
        self.set_wording(wording);
        Ok(summary)
    }

    /// Load several ODRL profiles at once (JSON-LD or Turtle text each), so
    /// a policy that mixes them can be read and validated against all of
    /// them. An empty list means no profile. Returns the profiles' summaries
    /// as the JSON text of a list, in order. Throws when a text is not a
    /// readable profile (the message names which); the previous profiles
    /// then stay. Replaces what `setProfile` loaded.
    #[wasm_bindgen(js_name = setProfiles)]
    pub fn set_profiles(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string[]")] texts: JsValue,
    ) -> Result<String, JsError> {
        let texts = texts_of(&texts).map_err(throw)?;
        let w = validation::wording(&texts).map_err(throw)?;
        let summaries: Vec<String> = w.profiles().map(summary_json).collect();
        let wording = (!w.is_empty()).then(|| SharedWording::from(w));
        self.set_wording(wording);
        Ok(format!("[{}]", summaries.join(",")))
    }

    /// Turn validation on with `options` (see `ValidateOptions`; `{}` is
    /// strict mode), or off with `null`. While on, the rendering gains a
    /// section with the findings and the verdict, and `report()` returns
    /// the report. Throws on options it cannot read; the previous options
    /// then stay.
    #[wasm_bindgen(js_name = setValidation)]
    pub fn set_validation(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "ValidateOptions | null")] options: JsValue,
    ) -> Result<(), JsError> {
        let options = match to_json(&options, "options").map_err(throw)? {
            Some(Value::Null) | None => None,
            Some(v) => Some(validation::options_from_value(Some(&v)).map_err(throw)?),
        };
        self.options = options;
        self.refresh();
        self.render();
        Ok(())
    }

    /// The validation report for the current policy and profiles, as the
    /// JSON text of `ValidationReport`, or `null` while validation is off
    /// (see `setValidation`).
    pub fn report(&self) -> Option<String> {
        self.validator
            .as_ref()
            .map(|v| validation::validate_text(&self.json, v))
    }

    /// Unmount and remove the rendered DOM. Safe to call more than once.
    pub fn destroy(&mut self) {
        if let Some(app) = self.app.take() {
            app.destroy();
        }
    }
}

impl ProseHandle {
    fn set_wording(&mut self, wording: Option<SharedWording>) {
        self.wording = wording;
        self.refresh();
        self.render();
    }

    /// Rebuilds the validator from the options and the profiles.
    fn refresh(&mut self) {
        self.validator = self.options.clone().map(|o| {
            let w = self
                .wording
                .as_ref()
                .map_or_else(Wording::new, |w| (*w.0).clone());
            Validator::new(Arc::new(w), o)
        });
    }

    fn render(&mut self) {
        if let Some(app) = self.app.as_mut() {
            app.update(props(
                &self.json,
                self.class.clone(),
                self.wording.clone(),
                self.validator.clone(),
            ));
        }
    }
}

/// Validates the policy `json` against the profiles (one text, a list of
/// texts, or `null`) and returns the report as JSON text
/// (`ds-prose/validation-report@1`). Strict unless `options.strict` is
/// `false`. Needs no DOM. Throws on options or profiles it cannot read; a
/// problem in the policy is never thrown, it is a finding.
#[wasm_bindgen]
pub fn validate(
    json: &str,
    #[wasm_bindgen(unchecked_param_type = "string | string[] | null")] profiles: JsValue,
    #[wasm_bindgen(unchecked_param_type = "ValidateOptions")] options: Option<JsValue>,
) -> Result<String, JsError> {
    let options = options_of(options.as_ref()).map_err(throw)?;
    let texts = match to_json(&profiles, "profiles").map_err(throw)? {
        Some(v) => validation::profile_list(&v).map_err(throw)?,
        None => Vec::new(),
    };
    let v = validation::validator(&texts, options).map_err(throw)?;
    Ok(validation::validate_text(json, &v))
}

/// The rule catalogue as the JSON text of a list of `RuleInfo`.
#[wasm_bindgen]
pub fn rules() -> String {
    validation::rules_json()
}

/// Profiles read once, to validate many policies against: the index of the
/// profiles' terms is built here, not on every call.
#[wasm_bindgen]
pub struct ProseValidator {
    validator: Validator,
}

#[wasm_bindgen]
impl ProseValidator {
    /// Reads `profiles` (JSON-LD or Turtle text each; possibly none) with
    /// `options`. Throws when a profile or the options cannot be read.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "string[]")] profiles: JsValue,
        #[wasm_bindgen(unchecked_param_type = "ValidateOptions")] options: Option<JsValue>,
    ) -> Result<ProseValidator, JsError> {
        let options = options_of(options.as_ref()).map_err(throw)?;
        let texts = texts_of(&profiles).map_err(throw)?;
        let validator = validation::validator(&texts, options).map_err(throw)?;
        Ok(ProseValidator { validator })
    }

    /// The report for the policy `json`, as JSON text.
    pub fn validate(&self, json: &str) -> String {
        validation::validate_text(json, &self.validator)
    }
}

#[wasm_bindgen(typescript_custom_section)]
const TYPES: &str = validation::TYPES_D_TS;

#[cfg(test)]
mod tests {
    use super::*;

    const RESEARCH: &str = include_str!("../../prose-core/tests/profiles/research.jsonld");
    const RESEARCH_TTL: &str = include_str!("../../prose-core/tests/profiles/research.ttl");

    fn summary(text: &str) -> Value {
        let p = ProfileVocab::parse(text).expect("a readable profile");
        serde_json::from_str(&summary_json(&p)).expect("the summary is JSON")
    }

    #[test]
    fn summary_names_the_profile_and_counts_each_kind() {
        let s = summary(RESEARCH);
        assert_eq!(s["iri"], "https://example.org/odrl/profile/research");
        assert_eq!(s["title"], "Research Data Profile");
        assert_eq!(s["version"], "0.1.0");
        assert_eq!(s["counts"]["action"], 3);
        assert_eq!(s["counts"]["leftOperand"], 3);
        assert_eq!(s["counts"]["operator"], 1);
        assert_eq!(s["counts"]["rightOperand"], 3);
        assert_eq!(s["counts"]["conflictTerm"], 1);
        assert!(
            s["counts"].get("ruleClass").is_none(),
            "absent kinds are left out"
        );
        assert_eq!(s["warnings"], json!([]));
    }

    #[test]
    fn summary_of_turtle_twin_is_the_same() {
        assert_eq!(summary(RESEARCH_TTL), summary(RESEARCH));
    }

    #[test]
    fn summary_lists_parse_warnings() {
        // An unbound skos: prefix is read with the well-known namespace, and
        // says so.
        let text = r#"{"@id":"https://example.org/x","@type":"http://www.w3.org/ns/odrl/2/Action","skos:definition":"d"}"#;
        let s = summary(text);
        assert_eq!(s["title"], Value::Null);
        assert_eq!(s["version"], Value::Null);
        let w = s["warnings"].as_array().expect("warnings is an array");
        assert!(
            w.iter().any(|w| w.as_str().unwrap_or("").contains("skos")),
            "{w:?}"
        );
    }

    #[test]
    fn read_profile_none_clears_and_errors_pass_through() {
        assert!(read_profile(None).unwrap().is_none());
        assert!(read_profile(Some(RESEARCH)).unwrap().is_some());
        assert!(read_profile(Some(RESEARCH_TTL)).unwrap().is_some());
        assert_eq!(
            read_profile(Some("<!doctype html><p>hi")).unwrap_err(),
            ProfileError::LooksLikeHtml
        );
    }
}
