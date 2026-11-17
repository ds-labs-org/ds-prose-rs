//! Bridge from `<OdrlProse>` (Yew) to JavaScript hosts such as Angular.
//!
//! A host owns a DOM element; [`ProseHandle`] mounts the Yew component into it,
//! re-renders it when the policy, class or profile changes, and tears it down.
//! Yew keeps its own virtual DOM inside that element, so the host must not
//! touch the element's children.
//!
//! `setProfile` reads an ODRL profile as JSON-LD, or Turtle with this
//! crate's `turtle` feature (on by default); the host fetches it. `setProfiles`
//! reads several, and `setValidation` adds the validator's findings to the
//! rendering.
//!
//! [`ProseEditorHandle`] mounts the editing view (`OdrlProseView` in Edit
//! mode) the same way and reports each committed edit as JSON-LD text; see
//! the [`editor`] module.
//!
//! Validation also works without the DOM: [`validate`], [`rules`] and
//! [`ProseValidator`] are plain functions over text, so a Node script can
//! call them after `initSync`. They return the report as JSON text
//! (`ds-prose/validation-report@1`), the same bytes everywhere.
pub mod editor;
pub mod validation;
pub use editor::ProseEditorHandle;
use prose_core::profile::{Overlay, ProfileError, ProfileVocab, ValueSources, Wording};
use prose_core::shapes::ShapesGraph;
use prose_core::validate::{Options, Validator};
use prose_yew::{OdrlProse, OdrlProseProps, SharedWording};
use serde_json::{Value, json};
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use web_sys::Element;
use yew::{AttrValue, Classes, Renderer};

#[wasm_bindgen]
extern "C" {
    /// The `ValidateOptions` object a host passes (declared in the package's
    /// types); an imported type so that the parameter is optional in them.
    #[wasm_bindgen(typescript_type = "ValidateOptions")]
    pub type ValidateOptionsArg;
}

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
        links: Default::default(),
    }
}

/// A JS value as JSON, through `JSON.stringify`: `None` for `undefined`.
/// Errors when the value has no JSON form (a cycle, a BigInt, a function).
pub(crate) fn to_json(v: &JsValue, what: &str) -> Result<Option<Value>, String> {
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

fn options_of(v: Option<&ValidateOptionsArg>) -> Result<Options, String> {
    let value = match v {
        Some(v) => to_json(v, "options")?,
        None => None,
    };
    validation::options_from_value(value.as_ref())
}

/// The profile texts of a list; unlike [`validate`], a lone string is not
/// accepted where the method says a list.
pub(crate) fn texts_of(v: &JsValue) -> Result<Vec<String>, String> {
    match to_json(v, "profiles")? {
        Some(l @ Value::Array(_)) => validation::profile_list(&l),
        _ => Err("profiles must be a list of strings".to_string()),
    }
}

pub(crate) fn throw(e: String) -> JsError {
    JsError::new(&e)
}

/// Reads `text` as a profile (JSON-LD, or Turtle with the `turtle`
/// feature). `None` means no profile.
pub fn read_profile(text: Option<&str>) -> Result<Option<SharedWording>, ProfileError> {
    match text {
        None => Ok(None),
        Some(t) => {
            let p = ProfileVocab::parse(t)?;
            Ok(Some(SharedWording::from(Wording::new().with_profile(p))))
        }
    }
}

/// `wording` (the profiles) with the operator overlays laid over it, then the
/// companion shapes (profile < overlay < shapes); the profiles alone when
/// there are neither.
pub(crate) fn shaped(
    wording: Option<&SharedWording>,
    shapes: Option<&ShapesGraph>,
    overlays: &[Overlay],
    sources: &ValueSources,
) -> Option<SharedWording> {
    if shapes.is_none() && overlays.is_empty() && sources.is_empty() {
        return wording.cloned();
    }
    let base = wording.map_or_else(Wording::new, |w| (*w.0).clone());
    let w = validation::with_overlays(base, overlays);
    let w = validation::with_shapes(w, shapes);
    Some(SharedWording::from(validation::with_value_sources(
        w, sources,
    )))
}

/// `current` with the source for `key` set to `values` (a list, or `null`
/// to clear) and `options` (`{closed?}`); `current` is left as it is. The
/// error is for the host to read.
pub(crate) fn next_sources(
    current: &ValueSources,
    key: &str,
    values: &JsValue,
    options: Option<&JsValue>,
) -> Result<ValueSources, String> {
    let values = to_json(values, "values")?.unwrap_or(Value::Null);
    let options = match options {
        Some(o) => to_json(o, "options")?,
        None => None,
    };
    let closed = validation::source_closed(options.as_ref())?;
    let mut next = current.clone();
    validation::set_value_source(&mut next, key, &values, closed)?;
    Ok(next)
}

/// The sources of the map a host passes to `setValueSources`; `null` and
/// `undefined` are none.
pub(crate) fn all_sources(map: &JsValue) -> Result<ValueSources, String> {
    match to_json(map, "valueSources")? {
        None | Some(Value::Null) => Ok(ValueSources::new()),
        Some(v) => validation::value_sources(&v),
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

/// A mounted `<OdrlProse>`. Call `destroy()` when the host element goes
/// away; `free()` alone releases the handle but leaves the rendered DOM and
/// its listeners in place.
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
    // Set by `setShapes`; laid over the profiles whenever either changes.
    shapes: Option<ShapesGraph>,
    // Set by `setOverlays`; laid over the profiles, under the shapes.
    overlays: Vec<Overlay>,
    // Set by `setValueSource(s)`; laid over everything else.
    sources: ValueSources,
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
            shapes: None,
            overlays: Vec::new(),
            sources: ValueSources::new(),
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
        let wording = read_profile(text.as_deref())
            .map_err(|e| JsError::new(&validation::profile_error(&e)))?;
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

    /// Load companion SHACL shapes (JSON-LD or Turtle text), or none for
    /// `null`: they add hints to the terms the loaded profiles declare, so
    /// the validation closes a right operand to the values a shape lists, and
    /// a finding names `"shapes"` as its source. Only a subset of SHACL is
    /// read; see `ShapesSummary`. Returns the summary as JSON text (`"null"`
    /// after clearing). Throws when the text is not readable shapes; the
    /// previous shapes then stay. Kept when the profiles change.
    #[wasm_bindgen(js_name = setShapes)]
    pub fn set_shapes(&mut self, text: Option<String>) -> Result<String, JsError> {
        let shapes = text
            .as_deref()
            .map(validation::shapes)
            .transpose()
            .map_err(throw)?;
        let summary = shapes
            .as_ref()
            .map_or_else(|| "null".to_string(), validation::shapes_summary_json);
        self.shapes = shapes;
        self.refresh();
        self.render();
        Ok(summary)
    }

    /// Load operator overlays (JSON-LD or Turtle text each), or none for `[]`:
    /// annotation-only graphs of `prose:` hints about terms the loaded
    /// profiles declare. They merge onto those terms and never add or relabel
    /// one; the shapes, when loaded, win over them. A finding names
    /// `"overlay"` as its source. Returns one summary per overlay as the JSON
    /// text of a list (`OverlaySummary`). Throws when a text is not a
    /// readable overlay (the message names which); the previous overlays then
    /// stay. Kept when the profiles change.
    #[wasm_bindgen(js_name = setOverlays)]
    pub fn set_overlays(&mut self, texts: Vec<String>) -> Result<String, JsError> {
        let overlays = validation::overlays(&texts).map_err(throw)?;
        self.overlays = overlays;
        let summary = self.overlay_summaries();
        self.refresh();
        self.render();
        Ok(summary)
    }

    /// The summaries of the loaded overlays, as `setOverlays` returns them,
    /// recomputed against the profiles loaded now: an overlay's `warnings`
    /// name the terms no loaded profile declares, so they change when the
    /// profiles do (`setOverlays` only reports the state at its own call).
    /// `"[]"` without overlays.
    #[wasm_bindgen(js_name = overlaySummaries)]
    pub fn overlay_summaries(&self) -> String {
        let w = self
            .wording
            .as_ref()
            .map_or_else(Wording::new, |w| (*w.0).clone());
        let w = validation::with_overlays(w, &self.overlays);
        validation::overlay_summaries_json(&w, &self.overlays)
    }

    /// Gives the values the host took from its registry or catalog for one
    /// left operand (`key` is its IRI) or one party role (`"assigner"`,
    /// `"assignee"`, `"target"`): a list of strings or
    /// `{value, label?, group?}`, which fills the select of a right operand
    /// (with `setTermInput("select")`) and the suggestions of a party slot.
    /// `options.closed` makes the list binding in strict validation; without
    /// it the list only suggests. `null` clears the source, and an empty list
    /// is no source. The values sit on top of the profile, the overlays and
    /// the shapes, for the values alone, and a finding names `"host"` as its
    /// source. A value the source no longer lists stays in the document and
    /// is flagged; it is never dropped. Returns the summary as JSON text
    /// (`ValueSourceSummary`, `"null"` after clearing). Throws when the
    /// values or the options cannot be read; the previous source then
    /// stays. Kept when the profiles change.
    #[wasm_bindgen(js_name = setValueSource)]
    pub fn set_value_source(
        &mut self,
        key: &str,
        #[wasm_bindgen(unchecked_param_type = "ValueSourceValue[] | null")] values: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "{ closed?: boolean }")] options: Option<
            JsValue,
        >,
    ) -> Result<String, JsError> {
        self.sources =
            next_sources(&self.sources, key, &values, options.as_ref()).map_err(throw)?;
        let summary = self.value_source_summary(key);
        self.refresh();
        self.render();
        Ok(summary)
    }

    /// Replaces every value source at once: an object keyed by left operand
    /// IRI or party role, each a list (suggestions) or `{values, closed?}`;
    /// `null` or `{}` clears them. Returns one summary per key as the JSON
    /// text of an object. Throws when it cannot be read; the previous sources
    /// then stay.
    #[wasm_bindgen(js_name = setValueSources)]
    pub fn set_value_sources(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "ValueSources | null")] sources: JsValue,
    ) -> Result<String, JsError> {
        self.sources = all_sources(&sources).map_err(throw)?;
        let summary = self.value_source_summaries();
        self.refresh();
        self.render();
        Ok(summary)
    }

    /// The summaries of the value sources, as `setValueSources` returns them,
    /// recomputed against the profiles loaded now: a source for a left
    /// operand no loaded profile declares is ignored, with a warning.
    #[wasm_bindgen(js_name = valueSourceSummaries)]
    pub fn value_source_summaries(&self) -> String {
        let w = self
            .wording
            .as_ref()
            .map_or_else(Wording::new, |w| (*w.0).clone());
        validation::value_source_summaries_json(&w, &self.sources)
    }

    fn value_source_summary(&self, key: &str) -> String {
        let w = self
            .wording
            .as_ref()
            .map_or_else(Wording::new, |w| (*w.0).clone());
        validation::value_source_summary_json(&w, &self.sources, key)
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
    #[wasm_bindgen(unchecked_return_type = "string | null")]
    pub fn report(&self) -> JsValue {
        match &self.validator {
            Some(v) => JsValue::from_str(&validation::validate_text(&self.json, v)),
            None => JsValue::NULL,
        }
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
            let w = shaped(
                self.wording.as_ref(),
                self.shapes.as_ref(),
                &self.overlays,
                &self.sources,
            )
            .map_or_else(Wording::new, |w| (*w.0).clone());
            Validator::new(Arc::new(w), o)
        });
    }

    fn render(&mut self) {
        if let Some(app) = self.app.as_mut() {
            app.update(props(
                &self.json,
                self.class.clone(),
                shaped(
                    self.wording.as_ref(),
                    self.shapes.as_ref(),
                    &self.overlays,
                    &self.sources,
                ),
                self.validator.clone(),
            ));
        }
    }
}

/// Validates the policy `json` against the profiles (one text, a list of
/// texts, or `null`) and returns the report as JSON text
/// (`ds-prose/validation-report@1`). Strict unless `options.strict` is
/// `false`. `shapes` is the text of companion SHACL shapes (see
/// `ProseHandle.setShapes`), if any, and `overlays` the texts of operator
/// overlays (see `ProseHandle.setOverlays`), and `valueSources` the host's runtime
/// value sources (see `ProseHandle.setValueSources`). Needs no DOM. Throws on
/// options, profiles, shapes or overlays it cannot read; a problem in the policy is never
/// thrown, it is a finding.
#[wasm_bindgen]
pub fn validate(
    json: &str,
    #[wasm_bindgen(unchecked_param_type = "string | string[] | null")] profiles: JsValue,
    options: Option<ValidateOptionsArg>,
    shapes: Option<String>,
    overlays: Option<Vec<String>>,
    #[wasm_bindgen(unchecked_optional_param_type = "ValueSources | null")] value_sources: Option<
        JsValue,
    >,
) -> Result<String, JsError> {
    let sources = match value_sources {
        Some(v) => all_sources(&v).map_err(throw)?,
        None => ValueSources::new(),
    };
    let options = options_of(options.as_ref()).map_err(throw)?;
    let texts = match to_json(&profiles, "profiles").map_err(throw)? {
        Some(v) => validation::profile_list(&v).map_err(throw)?,
        None => Vec::new(),
    };
    let v = validation::validator_with_sources(
        &texts,
        shapes.as_deref(),
        &overlays.unwrap_or_default(),
        &sources,
        options,
    )
    .map_err(throw)?;
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
    // What `setShapes` and `setOverlays` rebuild the validator from.
    wording: Wording,
    shapes: Option<ShapesGraph>,
    overlays: Vec<Overlay>,
    sources: ValueSources,
    options: Options,
}

#[wasm_bindgen]
impl ProseValidator {
    /// Reads `profiles` (JSON-LD or Turtle text each; possibly none) with
    /// `options`. Throws when a profile or the options cannot be read.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "string[]")] profiles: JsValue,
        options: Option<ValidateOptionsArg>,
    ) -> Result<ProseValidator, JsError> {
        let options = options_of(options.as_ref()).map_err(throw)?;
        let texts = texts_of(&profiles).map_err(throw)?;
        let wording = validation::wording(&texts).map_err(throw)?;
        let validator = Validator::new(Arc::new(wording.clone()), options.clone());
        Ok(ProseValidator {
            validator,
            wording,
            shapes: None,
            overlays: Vec::new(),
            sources: ValueSources::new(),
            options,
        })
    }

    /// Loads companion SHACL shapes (JSON-LD or Turtle text), or none for
    /// `null`; see `ProseHandle.setShapes`. Returns the summary as JSON text
    /// (`"null"` after clearing). Throws when the text is not readable
    /// shapes; the previous shapes then stay.
    #[wasm_bindgen(js_name = setShapes)]
    pub fn set_shapes(&mut self, text: Option<String>) -> Result<String, JsError> {
        let shapes = text
            .as_deref()
            .map(validation::shapes)
            .transpose()
            .map_err(throw)?;
        let summary = shapes
            .as_ref()
            .map_or_else(|| "null".to_string(), validation::shapes_summary_json);
        self.shapes = shapes;
        self.rebuild();
        Ok(summary)
    }

    /// Loads operator overlays (JSON-LD or Turtle text each), or none for
    /// `[]`; see `ProseHandle.setOverlays`. Returns the summaries as the JSON
    /// text of a list. Throws when a text is not a readable overlay; the
    /// previous overlays then stay.
    #[wasm_bindgen(js_name = setOverlays)]
    pub fn set_overlays(&mut self, texts: Vec<String>) -> Result<String, JsError> {
        let overlays = validation::overlays(&texts).map_err(throw)?;
        let summary = validation::overlay_summaries_json(
            &validation::with_overlays(self.wording.clone(), &overlays),
            &overlays,
        );
        self.overlays = overlays;
        self.rebuild();
        Ok(summary)
    }

    /// Sets, or with `null` clears, the host's value source for one left
    /// operand or party role; see `ProseHandle.setValueSource`. Returns the
    /// summary as JSON text. Throws when it cannot be read; the previous
    /// source then stays.
    #[wasm_bindgen(js_name = setValueSource)]
    pub fn set_value_source(
        &mut self,
        key: &str,
        #[wasm_bindgen(unchecked_param_type = "ValueSourceValue[] | null")] values: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "{ closed?: boolean }")] options: Option<
            JsValue,
        >,
    ) -> Result<String, JsError> {
        self.sources =
            next_sources(&self.sources, key, &values, options.as_ref()).map_err(throw)?;
        self.rebuild();
        Ok(validation::value_source_summary_json(
            &self.wording,
            &self.sources,
            key,
        ))
    }

    /// Replaces every value source at once; see `ProseHandle.setValueSources`.
    /// Returns the summaries as the JSON text of an object. Throws when it
    /// cannot be read; the previous sources then stay.
    #[wasm_bindgen(js_name = setValueSources)]
    pub fn set_value_sources(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "ValueSources | null")] sources: JsValue,
    ) -> Result<String, JsError> {
        self.sources = all_sources(&sources).map_err(throw)?;
        self.rebuild();
        Ok(validation::value_source_summaries_json(
            &self.wording,
            &self.sources,
        ))
    }

    /// The validator for the profiles with the overlays, then the shapes,
    /// then the host's value sources.
    fn rebuild(&mut self) {
        let wording = validation::with_value_sources(
            validation::with_shapes(
                validation::with_overlays(self.wording.clone(), &self.overlays),
                self.shapes.as_ref(),
            ),
            &self.sources,
        );
        self.validator = Validator::new(Arc::new(wording), self.options.clone());
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

    #[cfg(feature = "turtle")]
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
        #[cfg(feature = "turtle")]
        assert!(read_profile(Some(RESEARCH_TTL)).unwrap().is_some());
        #[cfg(not(feature = "turtle"))]
        assert_eq!(
            read_profile(Some(RESEARCH_TTL)).unwrap_err(),
            ProfileError::UnsupportedFormat
        );
        assert_eq!(
            read_profile(Some("<!doctype html><p>hi")).unwrap_err(),
            ProfileError::LooksLikeHtml
        );
    }
}
