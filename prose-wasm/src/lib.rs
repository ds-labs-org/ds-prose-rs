//! Bridge from `<OdrlProse>` (Yew) to JavaScript hosts such as Angular.
//!
//! A host owns a DOM element; [`ProseHandle`] mounts the Yew component into it,
//! re-renders it when the policy, class or profile changes, and tears it down.
//! Yew keeps its own virtual DOM inside that element, so the host must not
//! touch the element's children.
//!
//! `setProfile` reads an ODRL profile as JSON-LD or Turtle (prose-core's
//! `turtle` feature is on in this crate); the host fetches it.
pub mod validation;
use prose_core::profile::{ProfileError, ProfileVocab, Wording};
use prose_yew::{OdrlProse, OdrlProseProps, SharedWording};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;
use web_sys::Element;
use yew::{AttrValue, Classes, Renderer};

fn props(json: &str, class: Option<String>, wording: Option<SharedWording>) -> OdrlProseProps {
    OdrlProseProps {
        json: AttrValue::from(json.to_owned()),
        class: class.map(Classes::from).unwrap_or_default(),
        wording,
        glossary: true,
        validator: None,
        violations: false,
    }
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
}

#[wasm_bindgen]
impl ProseHandle {
    /// Mount into `root`. `class` is added to the component's root element.
    #[wasm_bindgen(constructor)]
    pub fn new(root: Element, json: &str, class: Option<String>) -> ProseHandle {
        let app =
            Renderer::<OdrlProse>::with_root_and_props(root, props(json, class.clone(), None))
                .render();
        ProseHandle {
            app: Some(app),
            json: json.to_owned(),
            class,
            wording: None,
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
        self.wording = wording;
        self.render();
        Ok(summary)
    }

    /// Unmount and remove the rendered DOM. Safe to call more than once.
    pub fn destroy(&mut self) {
        if let Some(app) = self.app.take() {
            app.destroy();
        }
    }
}

impl ProseHandle {
    fn render(&mut self) {
        if let Some(app) = self.app.as_mut() {
            app.update(props(&self.json, self.class.clone(), self.wording.clone()));
        }
    }
}

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
