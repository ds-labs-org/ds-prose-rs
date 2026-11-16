//! `ProseEditorHandle`: mounts `<OdrlProseView>` in Edit mode into a DOM
//! element and reports every committed edit as JSON-LD text.
//!
//! The handle owns no document state of its own beyond the text it last set
//! or emitted. The Yew host component (`EditorHost`) owns the document and
//! its undo history; the handle replaces the document through a generation
//! counter and receives the committed documents through one callback.
use crate::{read_profile, summary_json, throw, to_json, validation};
use prose_yew::prose_core::edit::{
    EditDoc, EditRules, Limit, StrictEdit, check_profile, read_model, write_jsonld,
};
use prose_yew::prose_core::profile::Wording;
use prose_yew::prose_core::validate::{Options, Report, Validator};
use prose_yew::{
    EditConfig, EditMode, OdrlProseView, ProseProblems, SharedWording, use_prose_editor_with,
    use_validation,
};
use serde_json::{Map, Value};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::Element;
use yew::prelude::*;

/// The names of `EditLimits`, with the field each one sets.
fn limit_field<'a>(r: &'a mut EditRules, name: &str) -> Option<&'a mut Limit> {
    Some(match name {
        "policies" => &mut r.policies,
        "permission" => &mut r.permission,
        "prohibition" => &mut r.prohibition,
        "obligation" => &mut r.obligation,
        "duty" => &mut r.duty,
        "remedy" => &mut r.remedy,
        "consequence" => &mut r.consequence,
        "policyAssigner" => &mut r.policy_assigner,
        "policyAssignee" => &mut r.policy_assignee,
        "policyTarget" => &mut r.policy_target,
        "policyAction" => &mut r.policy_action,
        "ruleAssigner" => &mut r.rule_assigner,
        "ruleAssignee" => &mut r.rule_assignee,
        "ruleTarget" => &mut r.rule_target,
        "ruleAction" => &mut r.rule_action,
        "actionRefinement" => &mut r.action_refinement,
        "entityRefinement" => &mut r.entity_refinement,
        "constraints" => &mut r.constraints,
        "logicalChildren" => &mut r.logical_children,
        "rightOperandValues" => &mut r.right_operand_values,
        "profile" => &mut r.profile,
        "inheritFrom" => &mut r.inherit_from,
        _ => return None,
    })
}

const LIMIT_NAMES: &str = "policies, permission, prohibition, obligation, duty, remedy, \
consequence, policyAssigner, policyAssignee, policyTarget, policyAction, ruleAssigner, \
ruleAssignee, ruleTarget, ruleAction, actionRefinement, entityRefinement, constraints, \
logicalChildren, rightOperandValues, profile, inheritFrom";

/// A non-negative integer of a limit entry.
fn count(name: &str, key: &str, v: &Value) -> Result<usize, String> {
    v.as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| format!("limits.{name}.{key} must be a non-negative integer, not {v}"))
}

/// One `LimitSpec`: a given entry replaces the default, so `min` is 0 and
/// `max` unbounded unless it says otherwise.
fn limit_of(name: &str, v: &Value) -> Result<Limit, String> {
    let o: &Map<String, Value> = v
        .as_object()
        .ok_or_else(|| format!("limits.{name} must be an object, not {v}"))?;
    if let Some(k) = o.keys().find(|k| *k != "min" && *k != "max") {
        return Err(format!("limits.{name}.{k} is not a limit (min, max)"));
    }
    let min = match o.get("min") {
        None | Some(Value::Null) => 0,
        Some(v) => count(name, "min", v)?,
    };
    let max = match o.get("max") {
        None | Some(Value::Null) => None,
        Some(v) => Some(count(name, "max", v)?),
    };
    if max.is_some_and(|max| min > max) {
        return Err(format!(
            "limits.{name}: min {min} is above max {}",
            max.unwrap_or(0)
        ));
    }
    Ok(Limit { min, max })
}

/// Sets the list limits of `rules` from a host's `EditLimits` value. `None`
/// and `null` restore the limits of `EditRules::default()`; the other
/// rules are left alone. On an error `rules` is unchanged.
pub(crate) fn apply_limits(rules: &mut EditRules, limits: Option<&Value>) -> Result<(), String> {
    let mut next = rules.clone();
    match limits {
        None | Some(Value::Null) => {
            let d = EditRules::default();
            let mut from = d.clone();
            for name in LIMIT_NAMES.split(", ") {
                if let (Some(to), Some(f)) =
                    (limit_field(&mut next, name), limit_field(&mut from, name))
                {
                    *to = *f;
                }
            }
        }
        Some(Value::Object(o)) => {
            for (name, spec) in o {
                let field = limit_field(&mut next, name).ok_or_else(|| {
                    format!("limits.{name} is not a list (lists are {LIMIT_NAMES})")
                })?;
                *field = limit_of(name, spec)?;
            }
        }
        Some(v) => return Err(format!("limits must be an object, not {v}")),
    }
    *rules = next;
    Ok(())
}

/// The JSON-LD the editor writes for `doc`, compact.
pub(crate) fn emitted_text(doc: &EditDoc) -> String {
    serde_json::to_string(&write_jsonld(doc)).unwrap_or_default()
}

/// `EditConfig::default()`, then the wording, then the validation options,
/// then the limits: `with_strict` rewrites the add-button defaults only, so
/// the limits are applied last and kept.
pub(crate) fn build_config(
    wording: Option<&Rc<Wording>>,
    options: Option<&Options>,
    limits: Option<&Value>,
) -> Result<EditConfig, String> {
    let mut c = EditConfig::default();
    if let Some(w) = wording {
        c = c.with_wording(w.clone());
    }
    if let Some(o) = options {
        c = c.with_strict(o.clone());
    }
    apply_limits(&mut c.rules, limits)?;
    Ok(c)
}

/// A document compared by identity, as a memo key.
#[derive(Clone)]
struct ByPtr<T>(Rc<T>);

impl<T> PartialEq for ByPtr<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Properties, Clone)]
struct HostProps {
    /// The document the host sets; read when `generation` changes.
    doc: Rc<EditDoc>,
    generation: u64,
    config: Rc<EditConfig>,
    class: Classes,
    /// The JSON-LD of each committed document.
    onchange: Callback<String>,
}

impl PartialEq for HostProps {
    fn eq(&self, o: &Self) -> bool {
        Rc::ptr_eq(&self.doc, &o.doc)
            && self.generation == o.generation
            && Rc::ptr_eq(&self.config, &o.config)
            && self.class == o.class
            && self.onchange == o.onchange
    }
}

/// What the document effect must not report.
struct Quiet {
    /// The mount.
    first: bool,
    /// A document the host set is on its way in.
    host_set: bool,
}

#[function_component(EditorHost)]
fn editor_host(props: &HostProps) -> Html {
    let config = props.config.clone();
    let rules = use_memo(config.clone(), |c| c.rules.clone());
    let initial = props.doc.clone();
    let with = use_prose_editor_with(
        move || (*initial).clone(),
        rules,
        config.validator.clone(),
        StrictEdit::Flag,
    );
    let editor = with.editor.clone();
    let quiet = use_mut_ref(|| Quiet {
        first: true,
        host_set: false,
    });
    // The host replaced the document: take it, and do not report it back.
    {
        let set_doc = editor.set_doc.clone();
        let quiet = quiet.clone();
        let doc = props.doc.clone();
        let mounted = use_mut_ref(|| false);
        use_effect_with(props.generation, move |_| {
            if std::mem::replace(&mut *mounted.borrow_mut(), true) {
                quiet.borrow_mut().host_set = true;
                set_doc.emit((*doc).clone());
            }
            || ()
        });
    }
    // A committed edit: report the JSON-LD.
    {
        let doc = editor.doc.clone();
        let onchange = props.onchange.clone();
        use_effect_with(Rc::as_ptr(&editor.doc) as usize, move |_| {
            let mut q = quiet.borrow_mut();
            if q.first {
                q.first = false;
            } else if q.host_set {
                q.host_set = false;
            } else {
                drop(q);
                onchange.emit(emitted_text(&doc));
            }
            || ()
        });
    }
    let validating = config.validator.is_some();
    let report: Rc<Report> = use_validation(&editor.doc, config.validator.as_ref());
    let issues = {
        let report = report.clone();
        use_memo(
            (ByPtr(editor.doc.clone()), config.clone()),
            move |(doc, c)| match (&c.validator, c.wording.is_empty()) {
                (Some(_), _) => report.issues(),
                (None, false) => check_profile(&doc.0, &c.wording),
                (None, true) => Vec::new(),
            },
        )
    };
    let refusal = with.refused.as_ref().and_then(|(ev, e)| e.issue(ev));
    html! {
        <>
            if let Some(e) = &editor.last_error {
                <p class="prose-error" role="alert">{ e.to_string() }</p>
            }
            <OdrlProseView
                doc={editor.doc.clone()}
                mode={EditMode::Edit}
                config={config}
                issues={issues}
                refusal={refusal}
                onedit={editor.onedit.clone()}
                class={props.class.clone()}
            />
            if validating {
                <ProseProblems report={report} onfix={with.apply_all.clone()} />
            }
        </>
    }
}

/// What the callback and the handle share.
struct Shared {
    /// The text last set or emitted.
    text: String,
    validator: Option<Validator>,
}

/// A mounted editor. Call `destroy()` (or `free()`) when the host element
/// goes away. The host must not touch the element's children, and must not
/// call a method of the handle from inside its `onChange`.
#[wasm_bindgen]
pub struct ProseEditorHandle {
    app: Option<yew::AppHandle<EditorHost>>,
    shared: Rc<RefCell<Shared>>,
    onchange: Callback<String>,
    doc: Rc<EditDoc>,
    generation: u64,
    class: Option<String>,
    wording: Option<SharedWording>,
    options: Option<Options>,
    limits: Option<Value>,
    config: Rc<EditConfig>,
}

#[wasm_bindgen]
impl ProseEditorHandle {
    /// Mounts the editor into `root`. Throws, mounting nothing, when `json`
    /// is not a readable ODRL policy. `onChange(json, report)` is called
    /// after every committed edit with the JSON-LD the editor writes and
    /// the validation report text (`null` while validation is off). It is
    /// not called on mount, or for a document set by the constructor or
    /// `update()`.
    #[wasm_bindgen(constructor)]
    pub fn new(
        root: Element,
        json: &str,
        class: Option<String>,
        #[wasm_bindgen(unchecked_param_type = "(json: string, report: string | null) => void")]
        on_change: JsValue,
    ) -> Result<ProseEditorHandle, JsError> {
        let f = on_change
            .dyn_into::<js_sys::Function>()
            .map_err(|_| throw("onChange must be a function".to_string()))?;
        let doc = Rc::new(read_model(json).map_err(|e| throw(e.to_string()))?);
        let shared = Rc::new(RefCell::new(Shared {
            text: json.to_owned(),
            validator: None,
        }));
        let onchange = {
            let shared = shared.clone();
            Callback::from(move |text: String| {
                // The report is made here, and the borrow ends before the
                // host's function runs, so the host may read `json()` at once.
                let report = {
                    let mut s = shared.borrow_mut();
                    s.text.clone_from(&text);
                    s.validator
                        .as_ref()
                        .map(|v| validation::validate_text(&text, v))
                };
                let report = report.map_or(JsValue::NULL, |r| JsValue::from_str(&r));
                if let Err(e) = f.call2(&JsValue::NULL, &JsValue::from_str(&text), &report) {
                    web_sys::console::error_1(&e);
                }
            })
        };
        let config = Rc::new(EditConfig::default());
        let mut h = ProseEditorHandle {
            app: None,
            shared,
            onchange,
            doc,
            generation: 0,
            class,
            wording: None,
            options: None,
            limits: None,
            config,
        };
        h.app = Some(yew::Renderer::<EditorHost>::with_root_and_props(root, h.props()).render());
        Ok(h)
    }

    /// Replaces the document. No-op when `json` equals the text the handle
    /// holds (the last set or emitted), so binding back what `onChange`
    /// emitted keeps the undo history. Throws on unreadable `json`; the
    /// document stays. `class` replaces the root element's class.
    pub fn update(&mut self, json: &str, class: Option<String>) -> Result<(), JsError> {
        let same = self.shared.borrow().text == json;
        if !same {
            self.doc = Rc::new(read_model(json).map_err(|e| throw(e.to_string()))?);
            self.generation += 1;
            self.shared.borrow_mut().text = json.to_owned();
        }
        let class_changed = self.class != class;
        self.class = class;
        if !same || class_changed {
            self.render();
        }
        Ok(())
    }

    /// Loads the ODRL profile (JSON-LD, or Turtle in the Turtle build) the
    /// sentences and suggestions use, or none for `null`. Returns its
    /// summary as JSON text (`"null"` after clearing), as
    /// `ProseHandle.setProfile` does. Throws when the text is not a
    /// readable profile; the previous profile stays. The document and its
    /// history stay.
    #[wasm_bindgen(js_name = setProfile)]
    pub fn set_profile(&mut self, text: Option<String>) -> Result<String, JsError> {
        let wording = read_profile(text.as_deref())
            .map_err(|e| JsError::new(&validation::profile_error(&e)))?;
        let summary = match wording.as_ref().and_then(|w| w.0.profiles().next()) {
            Some(p) => summary_json(p),
            None => "null".to_string(),
        };
        self.set_wording(wording)?;
        Ok(summary)
    }

    /// Loads several profiles, as `ProseHandle.setProfiles` does; replaces
    /// what `setProfile` loaded.
    #[wasm_bindgen(js_name = setProfiles)]
    pub fn set_profiles(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string[]")] texts: JsValue,
    ) -> Result<String, JsError> {
        let texts = crate::texts_of(&texts).map_err(throw)?;
        let w = validation::wording(&texts).map_err(throw)?;
        let summaries: Vec<String> = w.profiles().map(summary_json).collect();
        let wording = (!w.is_empty()).then(|| SharedWording::from(w));
        self.set_wording(wording)?;
        Ok(format!("[{}]", summaries.join(",")))
    }

    /// Validation on with `options` (see `ValidateOptions`; `{}` is strict
    /// mode), or off with `null`. While on, the editor shows inline issue
    /// markers and a problems panel with fix buttons, and `onChange`
    /// receives the report. Edits are flagged, never refused. Throws on
    /// options it cannot read; the previous options stay.
    #[wasm_bindgen(js_name = setValidation)]
    pub fn set_validation(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "ValidateOptions | null")] options: JsValue,
    ) -> Result<(), JsError> {
        let options = match to_json(&options, "options").map_err(throw)? {
            Some(Value::Null) | None => None,
            Some(v) => Some(validation::options_from_value(Some(&v)).map_err(throw)?),
        };
        let config = make(
            self.wording.as_ref(),
            options.as_ref(),
            self.limits.as_ref(),
        )?;
        self.install(config);
        self.options = options;
        self.render();
        Ok(())
    }

    /// Sets how many items each list may hold (see `EditLimits`); `null`
    /// restores the defaults. Throws on an unknown name, a negative or
    /// fractional number, or `min` above `max`; the previous limits stay.
    #[wasm_bindgen(js_name = setLimits)]
    pub fn set_limits(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "EditLimits | null")] limits: JsValue,
    ) -> Result<(), JsError> {
        let limits = to_json(&limits, "limits")
            .map_err(throw)?
            .filter(|v| !v.is_null());
        let config = make(
            self.wording.as_ref(),
            self.options.as_ref(),
            limits.as_ref(),
        )?;
        self.install(config);
        self.limits = limits;
        self.render();
        Ok(())
    }

    /// The validation report for the current text, as JSON text, or `null`
    /// while validation is off.
    #[wasm_bindgen(unchecked_return_type = "string | null")]
    pub fn report(&self) -> JsValue {
        let s = self.shared.borrow();
        match &s.validator {
            Some(v) => JsValue::from_str(&validation::validate_text(&s.text, v)),
            None => JsValue::NULL,
        }
    }

    /// The text the handle holds: the last set or emitted.
    pub fn json(&self) -> String {
        self.shared.borrow().text.clone()
    }

    /// Unmounts and removes the rendered DOM. Safe to call more than once.
    pub fn destroy(&mut self) {
        if let Some(app) = self.app.take() {
            app.destroy();
        }
    }
}

/// The configuration for the given parts, or the error to throw.
fn make(
    wording: Option<&SharedWording>,
    options: Option<&Options>,
    limits: Option<&Value>,
) -> Result<EditConfig, JsError> {
    build_config(wording.map(|w| &w.0), options, limits).map_err(throw)
}

impl ProseEditorHandle {
    fn props(&self) -> HostProps {
        HostProps {
            doc: self.doc.clone(),
            generation: self.generation,
            config: self.config.clone(),
            class: self.class.clone().map(Classes::from).unwrap_or_default(),
            onchange: self.onchange.clone(),
        }
    }

    fn render(&mut self) {
        let props = self.props();
        if let Some(app) = self.app.as_mut() {
            app.update(props);
        }
    }

    fn install(&mut self, config: EditConfig) {
        self.shared.borrow_mut().validator = config.validator.clone();
        self.config = Rc::new(config);
    }

    fn set_wording(&mut self, wording: Option<SharedWording>) -> Result<(), JsError> {
        let config = make(
            wording.as_ref(),
            self.options.as_ref(),
            self.limits.as_ref(),
        )?;
        self.install(config);
        self.wording = wording;
        self.render();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prose_yew::prose_core::edit::read_model;
    use prose_yew::prose_core::profile::ProfileVocab;
    use serde_json::json;

    const RESEARCH: &str = include_str!("../../prose-core/tests/profiles/research.jsonld");
    const POLICY: &str = include_str!("../../prose-core/tests/profiles/research-policy.json");

    /// Every name with the limit it sets.
    fn fields(r: &EditRules) -> [(&'static str, Limit); 22] {
        [
            ("policies", r.policies),
            ("permission", r.permission),
            ("prohibition", r.prohibition),
            ("obligation", r.obligation),
            ("duty", r.duty),
            ("remedy", r.remedy),
            ("consequence", r.consequence),
            ("policyAssigner", r.policy_assigner),
            ("policyAssignee", r.policy_assignee),
            ("policyTarget", r.policy_target),
            ("policyAction", r.policy_action),
            ("ruleAssigner", r.rule_assigner),
            ("ruleAssignee", r.rule_assignee),
            ("ruleTarget", r.rule_target),
            ("ruleAction", r.rule_action),
            ("actionRefinement", r.action_refinement),
            ("entityRefinement", r.entity_refinement),
            ("constraints", r.constraints),
            ("logicalChildren", r.logical_children),
            ("rightOperandValues", r.right_operand_values),
            ("profile", r.profile),
            ("inheritFrom", r.inherit_from),
        ]
    }

    fn limits(v: Value) -> Result<EditRules, String> {
        let mut r = EditRules::default();
        apply_limits(&mut r, Some(&v)).map(|_| r)
    }

    #[test]
    fn every_name_sets_its_own_field() {
        let defaults = EditRules::default();
        let names: Vec<&str> = fields(&defaults).iter().map(|(n, _)| *n).collect();
        assert_eq!(names.len(), 22);
        for (i, name) in names.iter().enumerate() {
            let r = limits(json!({ *name: { "min": 2, "max": 5 } })).unwrap();
            let want = Limit {
                min: 2,
                max: Some(5),
            };
            for (j, (n, l)) in fields(&r).iter().enumerate() {
                if i == j {
                    assert_eq!(*l, want, "{name} sets {n}");
                } else {
                    assert_eq!(*l, fields(&defaults)[j].1, "{name} must not set {n}");
                }
            }
        }
    }

    #[test]
    fn a_given_entry_replaces_the_default() {
        // policies defaults to at least 1; {max: 1} replaces it: min 0.
        let r = limits(json!({ "policies": { "max": 1 } })).unwrap();
        assert_eq!(
            r.policies,
            Limit {
                min: 0,
                max: Some(1)
            }
        );
        // ruleAction defaults to at least 1; {} replaces it with any.
        assert_eq!(
            limits(json!({ "ruleAction": {} })).unwrap().rule_action,
            Limit::ANY
        );
        assert_eq!(
            limits(json!({ "permission": { "min": 1, "max": null } }))
                .unwrap()
                .permission,
            Limit::at_least(1)
        );
        assert_eq!(
            limits(json!({ "policyAssigner": { "max": 0 } }))
                .unwrap()
                .policy_assigner,
            Limit::NONE
        );
    }

    #[test]
    fn null_and_none_reset_to_the_default_limits() {
        let mut r = limits(json!({ "policyAssigner": { "max": 0 }, "policies": {} })).unwrap();
        r.allow_reorder = !r.allow_reorder;
        let reorder = r.allow_reorder;
        apply_limits(&mut r, Some(&Value::Null)).unwrap();
        let d = EditRules::default();
        assert_eq!(fields(&r), fields(&d));
        assert_eq!(r.allow_reorder, reorder, "only the limits are reset");
        let mut r = limits(json!({ "profile": { "max": 0 } })).unwrap();
        apply_limits(&mut r, None).unwrap();
        assert_eq!(fields(&r), fields(&d));
    }

    #[test]
    fn bad_limits_are_refused_and_change_nothing() {
        for bad in [
            json!({ "bogus": {} }),
            json!({ "permission": { "min": -1 } }),
            json!({ "permission": { "max": -1 } }),
            json!({ "permission": { "min": 1.5 } }),
            json!({ "permission": { "max": 0.5 } }),
            json!({ "permission": { "min": "1" } }),
            json!({ "permission": { "min": 3, "max": 2 } }),
            json!({ "permission": { "foo": 1 } }),
            json!({ "permission": 3 }),
            json!([]),
            json!("x"),
        ] {
            let mut r = EditRules::default();
            // A valid entry before the bad one must not stick.
            let mut v = bad.clone();
            if let Some(o) = v.as_object_mut() {
                o.insert("duty".into(), json!({ "max": 0 }));
            }
            for value in [bad, v] {
                let before = fields(&r);
                assert!(apply_limits(&mut r, Some(&value)).is_err(), "{value}");
                assert_eq!(fields(&r), before, "{value}");
            }
        }
    }

    #[test]
    fn emitted_text_is_compact_write_jsonld() {
        let doc = read_model(POLICY).unwrap();
        let text = emitted_text(&doc);
        assert_eq!(text, serde_json::to_string(&write_jsonld(&doc)).unwrap());
        assert!(!text.contains('\n') && !text.contains(": "), "{text}");
        assert_eq!(read_model(&text).unwrap(), doc, "it reads back");
    }

    #[test]
    fn limits_survive_with_strict() {
        let l = json!({ "policyAssigner": { "max": 0 }, "policies": { "min": 1, "max": 1 } });
        let w = Rc::new(Wording::new().with_profile(ProfileVocab::parse(RESEARCH).unwrap()));
        let c = build_config(Some(&w), Some(&Options::strict()), Some(&l)).unwrap();
        assert!(c.validator.is_some());
        assert_eq!(c.rules.policy_assigner, Limit::NONE);
        assert_eq!(c.rules.policies, Limit::exactly(1));
        assert!(Rc::ptr_eq(&c.wording, &w));
        let c = build_config(None, None, None).unwrap();
        assert!(c.validator.is_none());
        assert_eq!(fields(&c.rules), fields(&EditRules::default()));
        assert!(build_config(None, None, Some(&json!({ "x": {} }))).is_err());
    }
}
