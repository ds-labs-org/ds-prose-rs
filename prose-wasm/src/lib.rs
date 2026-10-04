//! Bridge from `<OdrlProse>` (Yew) to JavaScript hosts such as Angular.
//!
//! A host owns a DOM element; [`ProseHandle`] mounts the Yew component into it,
//! re-renders it when the policy or class changes, and tears it down. Yew keeps
//! its own virtual DOM inside that element, so the host must not touch the
//! element's children.
use prose_yew::{OdrlProse, OdrlProseProps};
use wasm_bindgen::prelude::*;
use web_sys::Element;
use yew::{AttrValue, Classes, Renderer};

fn props(json: &str, class: Option<String>) -> OdrlProseProps {
    OdrlProseProps {
        json: AttrValue::from(json.to_owned()),
        class: class.map(Classes::from).unwrap_or_default(),
    }
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

    /// Unmount and remove the rendered DOM. Safe to call more than once.
    pub fn destroy(&mut self) {
        if let Some(app) = self.app.take() {
            app.destroy();
        }
    }
}

