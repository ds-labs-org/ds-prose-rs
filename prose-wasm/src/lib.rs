//! Bridge from `<OdrlProse>` (Yew) to JavaScript hosts such as Angular.
//!
//! A host owns a DOM element; [`ProseHandle`] mounts the Yew component into it,
//! re-renders it when the policy or class changes, and tears it down. Yew keeps
//! its own virtual DOM inside that element, so the host must not touch the
//! element's children.
use prose_core::profile::{ProfileError, ProfileVocab};
use prose_yew::{OdrlProse, OdrlProseProps, SharedWording};
use wasm_bindgen::prelude::*;
use web_sys::Element;
use yew::{AttrValue, Classes, Renderer};

fn props(json: &str, class: Option<String>) -> OdrlProseProps {
    OdrlProseProps {
        json: AttrValue::from(json.to_owned()),
        class: class.map(Classes::from).unwrap_or_default(),
        wording: None,
        glossary: true,
    }
}

/// Reads `text` as a profile (JSON-LD, or Turtle: this crate turns on
/// prose-core's `turtle` feature). `None` means no profile.
pub fn read_profile(_text: Option<&str>) -> Result<Option<SharedWording>, ProfileError> {
    Ok(None)
}

/// What `setProfile` returns for a loaded profile, as JSON text.
pub fn summary_json(_p: &ProfileVocab) -> String {
    "{}".to_string()
}

/// A mounted `<OdrlProse>`. Call `destroy()` (or `free()`) when the host
/// element goes away.
#[wasm_bindgen]
pub struct ProseHandle {
    app: Option<yew::AppHandle<OdrlProse>>,
}

#[wasm_bindgen]
impl ProseHandle {
    /// Mount into `root`. `class` is added to the component's root element.
    #[wasm_bindgen(constructor)]
    pub fn new(root: Element, json: &str, class: Option<String>) -> ProseHandle {
        let app = Renderer::<OdrlProse>::with_root_and_props(root, props(json, class)).render();
        ProseHandle { app: Some(app) }
    }

    /// Re-render with a new policy and class. No-op after `destroy()`.
    pub fn update(&mut self, json: &str, class: Option<String>) {
        if let Some(app) = self.app.as_mut() {
            app.update(props(json, class));
        }
    }

    /// Not implemented yet.
    #[wasm_bindgen(js_name = setProfile)]
    pub fn set_profile(&mut self, _text: Option<String>) -> Result<String, JsError> {
        Ok("null".to_string())
    }

    /// Unmount and remove the rendered DOM. Safe to call more than once.
    pub fn destroy(&mut self) {
        if let Some(app) = self.app.take() {
            app.destroy();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

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
