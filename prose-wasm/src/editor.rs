//! `ProseEditorHandle`: mounts `<OdrlProseView>` in Edit mode into a DOM
//! element and reports every committed edit as JSON-LD text.
//!
//! The handle owns no document state of its own beyond the text it last set
//! or emitted. The Yew host component (`EditorHost`) owns the document and
//! its undo history; the handle replaces the document through a generation
//! counter and receives the committed documents through one callback.
use crate::{read_profile, summary_json, throw, to_json, validation};
use prose_yew::prose_core::edit::{
    EditDoc, EditRules, IssueTarget, Limit, OutOfScope, StrictEdit, check_profile, out_of_scope,
    read_model, strip_out_of_scope, write_jsonld,
};
use prose_yew::prose_core::profile::{Overlay, ValueSources, Wording};
use prose_yew::prose_core::shapes::ShapesGraph;
use prose_yew::prose_core::validate::{Options, Report, Validator};
use prose_yew::{
    EditConfig, EditMode, OdrlProseView, ProseProblems, SharedWording, TermInput, go_to,
    use_prose_editor_with, use_validation,
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
        "policyUid" => &mut r.policy_uid,
        "ruleAssigner" => &mut r.rule_assigner,
        "ruleAssignee" => &mut r.rule_assignee,
        "ruleTarget" => &mut r.rule_target,
        "ruleAction" => &mut r.rule_action,
        "actionRefinement" => &mut r.action_refinement,
        "entityRefinement" => &mut r.entity_refinement,
        "constraints" => &mut r.constraints,
        "logicalChildren" => &mut r.logical_children,
        "logicalDepth" => &mut r.logical_depth,
        "rightOperandValues" => &mut r.right_operand_values,
        "profile" => &mut r.profile,
        "inheritFrom" => &mut r.inherit_from,
        _ => return None,
    })
}

const LIMIT_NAMES: &str = "policies, permission, prohibition, obligation, duty, remedy, \
consequence, policyAssigner, policyAssignee, policyTarget, policyAction, policyUid, ruleAssigner, \
ruleAssignee, ruleTarget, ruleAction, actionRefinement, entityRefinement, constraints, \
logicalChildren, logicalDepth, rightOperandValues, profile, inheritFrom";

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

/// What the editor does with a section whose limit is `max: 0` when the
/// document it loads holds one (`setOutOfScope`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Scope {
    /// Shown and kept, as before limits were a scope.
    #[default]
    Show,
    /// Dropped on load and reported.
    Strip,
    /// The load fails, naming the paths.
    Reject,
}

/// `Scope` by its `setOutOfScope` name.
pub(crate) fn scope_of(name: &str) -> Result<Scope, String> {
    match name {
        "show" => Ok(Scope::Show),
        "strip" => Ok(Scope::Strip),
        "reject" => Ok(Scope::Reject),
        other => Err(format!(
            "outOfScope must be \"show\", \"strip\" or \"reject\", not {other:?}"
        )),
    }
}

/// A document the scope refuses (`Scope::Reject`).
#[derive(Debug, PartialEq)]
pub(crate) struct ScopeError {
    pub message: String,
    pub paths: Vec<String>,
}

/// The text a document loads as, and what the scope took out of it.
#[derive(Debug, PartialEq)]
pub(crate) struct Scoped {
    pub text: String,
    pub removed: Vec<OutOfScope>,
}

/// `text` under `rules` and `mode`: unchanged while nothing is out of scope
/// (and always for `Show`); without the sections otherwise (`Strip`), or
/// refused (`Reject`). Text that is not JSON passes through: the reader
/// refuses it with its own error.
pub(crate) fn apply_scope(
    text: &str,
    rules: &EditRules,
    mode: Scope,
) -> Result<Scoped, ScopeError> {
    let keep = |text: &str| Scoped {
        text: text.to_owned(),
        removed: Vec::new(),
    };
    if mode == Scope::Show {
        return Ok(keep(text));
    }
    let Ok(mut value) = serde_json::from_str::<Value>(text) else {
        return Ok(keep(text));
    };
    match mode {
        Scope::Show => Ok(keep(text)),
        Scope::Strip => {
            let removed = strip_out_of_scope(&mut value, rules);
            if removed.is_empty() {
                return Ok(keep(text));
            }
            Ok(Scoped {
                text: serde_json::to_string(&value).unwrap_or_default(),
                removed,
            })
        }
        Scope::Reject => {
            let found = out_of_scope(&value, rules);
            if found.is_empty() {
                return Ok(keep(text));
            }
            let list: Vec<String> = found
                .iter()
                .map(|o| format!("{} ({})", o.path, o.limit))
                .collect();
            Err(ScopeError {
                message: format!(
                    "the document holds sections out of scope: {}",
                    list.join(", ")
                ),
                paths: found.into_iter().map(|o| o.path).collect(),
            })
        }
    }
}

/// The removed sections as JSON text: `[{"path","key","limit"}]`.
pub(crate) fn removed_json(removed: &[OutOfScope]) -> String {
    let items: Vec<Value> = removed
        .iter()
        .map(|o| serde_json::json!({ "path": o.path, "key": o.key, "limit": o.limit }))
        .collect();
    serde_json::to_string(&items).unwrap_or_default()
}

/// The error a refused document throws: an `Error` named `OutOfScopeError`
/// with the offending `paths` as an array.
fn scope_error(e: &ScopeError) -> JsValue {
    let err = js_sys::Error::new(&e.message);
    err.set_name("OutOfScopeError");
    let paths = js_sys::Array::new();
    for p in &e.paths {
        paths.push(&JsValue::from_str(p));
    }
    let _ = js_sys::Reflect::set(&err, &JsValue::from_str("paths"), &paths);
    err.into()
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
        config.strict_edit,
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
    let view = use_node_ref();
    let onjump = {
        let view = view.clone();
        Callback::from(move |t: IssueTarget| {
            go_to(&view, &t);
        })
    };
    html! {
        <div ref={view}>
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
                <ProseProblems
                    report={report}
                    onjump={onjump}
                    onfix={with.apply_all.clone()}
                />
            }
        </div>
    }
}

/// What the callback and the handle share.
struct Shared {
    /// The text last set or emitted, as the scope left it.
    text: String,
    /// The text the host last set, or the last one emitted: setting it
    /// again changes nothing, even when the scope loaded another text from it.
    source: String,
    validator: Option<Validator>,
}

/// A mounted editor. Call `destroy()` when the host element goes away;
/// dropping or `free()`ing the handle unmounts it as well. The host must not touch the element's children, and must not
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
    /// What `setShapes` loaded, laid over `wording` whenever either changes.
    shapes: Option<ShapesGraph>,
    /// What `setOverlays` loaded, laid over `wording` under the shapes.
    overlays: Vec<Overlay>,
    /// What `setValueSource(s)` gave, laid over everything else.
    sources: ValueSources,
    options: Option<Options>,
    limits: Option<Value>,
    term_input: TermInput,
    strict_edit: StrictEdit,
    config: Rc<EditConfig>,
    scope: Scope,
    /// What the last load or scope change took out.
    removed: Vec<OutOfScope>,
}

/// `TermInput` by its `setTermInput` name.
pub(crate) fn term_input_of(name: &str) -> Result<TermInput, String> {
    match name {
        "free" => Ok(TermInput::Free),
        "select" => Ok(TermInput::Select),
        other => Err(format!(
            "termInput must be \"free\" or \"select\", not {other:?}"
        )),
    }
}

impl Drop for ProseEditorHandle {
    fn drop(&mut self) {
        self.destroy();
    }
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
            source: json.to_owned(),
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
                    s.source.clone_from(&text);
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
            shapes: None,
            overlays: Vec::new(),
            sources: ValueSources::new(),
            options: None,
            limits: None,
            term_input: TermInput::Free,
            strict_edit: StrictEdit::Flag,
            config,
            scope: Scope::Show,
            removed: Vec::new(),
        };
        h.app = Some(yew::Renderer::<EditorHost>::with_root_and_props(root, h.props()).render());
        Ok(h)
    }

    /// Replaces the document. No-op when `json` equals the text the handle
    /// holds (the last set or emitted), or the last one the host set, so
    /// binding back what `onChange` emitted keeps the undo history. Under
    /// the scope (`setOutOfScope`) the sections whose limit is `max: 0` are
    /// dropped (`"strip"`; see `removed()`) or refuse the document
    /// (`"reject"`). Throws on unreadable `json`, or on one the scope
    /// refuses (an `OutOfScopeError` with the offending `paths`); the
    /// document stays. `class` replaces the root element's class.
    pub fn update(&mut self, json: &str, class: Option<String>) -> Result<(), JsValue> {
        let same = {
            let s = self.shared.borrow();
            s.text == json || s.source == json
        };
        if !same {
            let scoped =
                apply_scope(json, &self.config.rules, self.scope).map_err(|e| scope_error(&e))?;
            self.doc =
                Rc::new(read_model(&scoped.text).map_err(|e| JsValue::from(throw(e.to_string())))?);
            self.generation += 1;
            {
                let mut s = self.shared.borrow_mut();
                s.text = scoped.text;
                s.source = json.to_owned();
            }
            self.removed = scoped.removed;
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

    /// Loads companion SHACL shapes (JSON-LD, or Turtle in the Turtle build),
    /// or none for `null`. They add hints to the terms the loaded profiles
    /// declare: a right operand a shape closes with `sh:in` becomes a select
    /// of those values (with `setTermInput("select")`) and any other value
    /// is an error naming `"shapes"` as its source; `sh:in` on `odrl:action`
    /// or `odrl:leftOperand` closes those lists for the whole policy. Only a
    /// subset of SHACL is read (see `ShapesSummary`), and the rest is
    /// listed in the `warnings` of the summary this returns as JSON text
    /// (`"null"` after clearing). Throws when the text is not readable
    /// shapes; the previous shapes stay. The document and its history stay,
    /// and a later `setProfile` keeps the shapes.
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
        let config = make(
            crate::shaped(
                self.wording.as_ref(),
                shapes.as_ref(),
                &self.overlays,
                &self.sources,
            )
            .as_ref(),
            self.options.as_ref(),
            self.limits.as_ref(),
        )?;
        self.install(config);
        self.shapes = shapes;
        self.render();
        Ok(summary)
    }

    /// Loads operator overlays (JSON-LD, or Turtle in the Turtle build, one
    /// text each), or none for `[]`: annotation-only graphs of `prose:` hints
    /// about terms the loaded profiles declare. They merge onto those terms
    /// and never add or relabel one, and the shapes, when loaded, win over
    /// them. A right operand an overlay closes becomes a select (with
    /// `setTermInput("select")`) and any other value is an error naming
    /// `"overlay"` as its source. Returns one summary per overlay as the JSON
    /// text of a list (`OverlaySummary`: `terms`, `warnings`). Throws when a
    /// text is not a readable overlay; the previous overlays stay. The
    /// document and its history stay, and a later `setProfile` keeps the
    /// overlays.
    #[wasm_bindgen(js_name = setOverlays)]
    pub fn set_overlays(&mut self, texts: Vec<String>) -> Result<String, JsError> {
        let overlays = validation::overlays(&texts).map_err(throw)?;
        let config = make(
            crate::shaped(
                self.wording.as_ref(),
                self.shapes.as_ref(),
                &overlays,
                &self.sources,
            )
            .as_ref(),
            self.options.as_ref(),
            self.limits.as_ref(),
        )?;
        let base = self
            .wording
            .as_ref()
            .map_or_else(Wording::new, |w| (*w.0).clone());
        let summary = validation::overlay_summaries_json(
            &validation::with_overlays(base, &overlays),
            &overlays,
        );
        self.install(config);
        self.overlays = overlays;
        self.render();
        Ok(summary)
    }

    /// The summaries of the loaded overlays, as `setOverlays` returns them,
    /// recomputed against the profiles loaded now (their `warnings` name the
    /// terms no loaded profile declares, so they change with the profiles).
    /// `"[]"` without overlays.
    #[wasm_bindgen(js_name = overlaySummaries)]
    pub fn overlay_summaries(&self) -> String {
        let base = self
            .wording
            .as_ref()
            .map_or_else(Wording::new, |w| (*w.0).clone());
        validation::overlay_summaries_json(
            &validation::with_overlays(base, &self.overlays),
            &self.overlays,
        )
    }

    /// Gives the values the host took from its registry or catalog for one
    /// left operand (`key` is its IRI) or one party role (`"assigner"`,
    /// `"assignee"`, `"target"`): a list of strings or
    /// `{value, label?, group?}`. With `options.closed` a right operand
    /// becomes a select of exactly those values (with `setTermInput("select")`),
    /// listed with their labels under their groups, and any other value is an
    /// error naming `"host"` as its source; without it the list only
    /// suggests. A party role's list feeds the suggestions of those slots.
    /// `null` clears the source; an empty list is no source. The host's values
    /// sit on top of the profile, the overlays and the shapes, for the values
    /// alone. A value the source no longer lists stays in the document,
    /// selected and flagged (with validation on); it is never dropped, and
    /// loading a source is not an edit. Returns the summary as JSON text
    /// (`ValueSourceSummary`, `"null"` after clearing). Throws when the values
    /// or options cannot be read; the previous source then stays. A later
    /// `setProfile` keeps the sources.
    #[wasm_bindgen(js_name = setValueSource)]
    pub fn set_value_source(
        &mut self,
        key: &str,
        #[wasm_bindgen(unchecked_param_type = "ValueSourceValue[] | null")] values: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "{ closed?: boolean }")] options: Option<
            JsValue,
        >,
    ) -> Result<String, JsError> {
        let next =
            crate::next_sources(&self.sources, key, &values, options.as_ref()).map_err(throw)?;
        self.apply_sources(next, Some(key))
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
        let next = crate::all_sources(&sources).map_err(throw)?;
        self.apply_sources(next, None)
    }

    /// The summaries of the value sources, as `setValueSources` returns
    /// them, recomputed against the profiles loaded now: a source for a left
    /// operand no loaded profile declares is ignored, with a warning.
    #[wasm_bindgen(js_name = valueSourceSummaries)]
    pub fn value_source_summaries(&self) -> String {
        validation::value_source_summaries_json(&self.base_wording(), &self.sources)
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
            crate::shaped(
                self.wording.as_ref(),
                self.shapes.as_ref(),
                &self.overlays,
                &self.sources,
            )
            .as_ref(),
            options.as_ref(),
            self.limits.as_ref(),
        )?;
        self.install(config);
        self.options = options;
        self.render();
        Ok(())
    }

    /// Sets how many items each list may hold (see `EditLimits`); `null`
    /// restores the defaults. Under `setOutOfScope("strip")` a list limited
    /// to `max: 0` is also dropped from the document on screen (see
    /// `removed()`); under `"reject"` a document that holds one refuses the
    /// limits (an `OutOfScopeError`). Throws on an unknown name, a negative
    /// or fractional number, or `min` above `max`; the previous limits stay.
    #[wasm_bindgen(js_name = setLimits)]
    pub fn set_limits(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "EditLimits | null")] limits: JsValue,
    ) -> Result<(), JsValue> {
        let limits = to_json(&limits, "limits")
            .map_err(throw)?
            .filter(|v| !v.is_null());
        let config = make(
            crate::shaped(
                self.wording.as_ref(),
                self.shapes.as_ref(),
                &self.overlays,
                &self.sources,
            )
            .as_ref(),
            self.options.as_ref(),
            limits.as_ref(),
        )?;
        let scoped = self.rescope(&config.rules, self.scope)?;
        self.install(config);
        self.limits = limits;
        self.take(scoped);
        self.render();
        Ok(())
    }

    /// What the editor does with a section whose limit is `max: 0` when a
    /// document holds one: `"show"` (the default) keeps and shows it;
    /// `"strip"` drops it on load and on every later `update()` or limits
    /// change, and `removed()` lists what went; `"reject"` refuses the
    /// document (an `OutOfScopeError` with the offending `paths`). The
    /// document on screen is checked at once. Throws on any other name, or
    /// when `"reject"` refuses the document on screen (limits of `max: 0`
    /// that the `"show"` mode left in it); the call then changes nothing:
    /// the previous mode stays, the document and `removed()` are as they
    /// were. Switch to `"strip"` instead, or `update` the document first.
    #[wasm_bindgen(js_name = setOutOfScope)]
    pub fn set_out_of_scope(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "OutOfScopeMode")] mode: &str,
    ) -> Result<(), JsValue> {
        let mode = scope_of(mode).map_err(|e| JsValue::from(throw(e)))?;
        let scoped = self.rescope(&self.config.rules.clone(), mode)?;
        self.scope = mode;
        self.take(scoped);
        self.render();
        Ok(())
    }

    /// What the last load took out under `"strip"` (the constructor,
    /// `update()`, `setLimits()` or `setOutOfScope()`), as JSON text:
    /// `[{"path","key","limit"}]`, `"[]"` when it took nothing out.
    pub fn removed(&self) -> String {
        removed_json(&self.removed)
    }

    /// How the term slots take their value: `"free"` (the default) is
    /// typed text with suggestions; `"select"` is a `<select>` of the terms
    /// the loaded profiles (and the ODRL core vocabulary) define, filtered
    /// by strict validation when it is on. A unit or a right operand is a
    /// select only when the profile lists units or values for its left
    /// operand. A stored value outside the list stays, selected and
    /// flagged. Throws on any other name; the previous mode stays.
    #[wasm_bindgen(js_name = setTermInput)]
    pub fn set_term_input(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "TermInput")] mode: &str,
    ) -> Result<(), JsError> {
        let mode = term_input_of(mode).map_err(throw)?;
        if mode != self.term_input {
            self.term_input = mode;
            let mut config = (*self.config).clone();
            config.term_input = mode;
            self.install(config);
            self.render();
        }
        Ok(())
    }

    /// Under validation, whether an edit that writes a term the validator
    /// does not allow into a vocabulary slot (an action, a left operand, an
    /// operator, a unit, a conflict strategy or a policy type) is refused
    /// (`true`, as `StrictEdit::RefuseTerms`) or committed and flagged
    /// (`false`, the default). The document and its history stay. Without
    /// validation it does nothing.
    #[wasm_bindgen(js_name = setRefuseTerms)]
    pub fn set_refuse_terms(&mut self, on: bool) {
        let mode = if on {
            StrictEdit::RefuseTerms
        } else {
            StrictEdit::Flag
        };
        if mode != self.strict_edit {
            self.strict_edit = mode;
            let mut config = (*self.config).clone();
            config.strict_edit = mode;
            self.install(config);
            self.render();
        }
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

    /// The current text under `rules` and `mode`, or the error to throw.
    fn rescope(&self, rules: &EditRules, mode: Scope) -> Result<Option<Scoped>, JsValue> {
        let text = self.shared.borrow().text.clone();
        let scoped = apply_scope(&text, rules, mode).map_err(|e| scope_error(&e))?;
        Ok(Some(scoped).filter(|s| s.text != text))
    }

    /// Takes the document a scope change made, if it made one. It is set
    /// by the host, so it is not reported as an edit.
    fn take(&mut self, scoped: Option<Scoped>) {
        self.removed.clear();
        if let Some(s) = scoped {
            if let Ok(doc) = read_model(&s.text) {
                self.doc = Rc::new(doc);
                self.generation += 1;
                self.shared.borrow_mut().text = s.text;
            }
            self.removed = s.removed;
        }
    }

    fn render(&mut self) {
        let props = self.props();
        if let Some(app) = self.app.as_mut() {
            app.update(props);
        }
    }

    fn install(&mut self, mut config: EditConfig) {
        config.term_input = self.term_input;
        config.strict_edit = self.strict_edit;
        self.shared.borrow_mut().validator = config.validator.clone();
        self.config = Rc::new(config);
    }

    /// The loaded profiles as one wording.
    fn base_wording(&self) -> Wording {
        self.wording
            .as_ref()
            .map_or_else(Wording::new, |w| (*w.0).clone())
    }

    /// Installs `sources` and renders. The summary is the one for `key`, or
    /// all of them.
    fn apply_sources(
        &mut self,
        sources: ValueSources,
        key: Option<&str>,
    ) -> Result<String, JsError> {
        let config = make(
            crate::shaped(
                self.wording.as_ref(),
                self.shapes.as_ref(),
                &self.overlays,
                &sources,
            )
            .as_ref(),
            self.options.as_ref(),
            self.limits.as_ref(),
        )?;
        let base = self.base_wording();
        let summary = match key {
            Some(k) => validation::value_source_summary_json(&base, &sources, k),
            None => validation::value_source_summaries_json(&base, &sources),
        };
        self.install(config);
        self.sources = sources;
        self.render();
        Ok(summary)
    }

    fn set_wording(&mut self, wording: Option<SharedWording>) -> Result<(), JsError> {
        let config = make(
            crate::shaped(
                wording.as_ref(),
                self.shapes.as_ref(),
                &self.overlays,
                &self.sources,
            )
            .as_ref(),
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
    fn fields(r: &EditRules) -> [(&'static str, Limit); 24] {
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
            ("policyUid", r.policy_uid),
            ("ruleAssigner", r.rule_assigner),
            ("ruleAssignee", r.rule_assignee),
            ("ruleTarget", r.rule_target),
            ("ruleAction", r.rule_action),
            ("actionRefinement", r.action_refinement),
            ("entityRefinement", r.entity_refinement),
            ("constraints", r.constraints),
            ("logicalChildren", r.logical_children),
            ("logicalDepth", r.logical_depth),
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
    fn the_unknown_name_error_lists_policy_uid() {
        let err = limits(json!({ "bogus": { "max": 0 } })).unwrap_err();
        assert!(err.contains("policyUid"), "{err}");
    }

    #[test]
    fn every_name_sets_its_own_field() {
        let defaults = EditRules::default();
        let names: Vec<&str> = fields(&defaults).iter().map(|(n, _)| *n).collect();
        assert_eq!(names.len(), 24);
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

    const PARTIES: &str = r#"{"@type":"Set","uid":"urn:p:1","assigner":"urn:a","permission":[{"action":"use","odrl:assignee":"urn:b"}]}"#;

    fn no_parties() -> EditRules {
        let mut r = EditRules::default();
        apply_limits(
            &mut r,
            Some(&json!({
                "policyAssigner": { "max": 0 }, "policyAssignee": { "max": 0 },
                "ruleAssigner": { "max": 0 }, "ruleAssignee": { "max": 0 }
            })),
        )
        .unwrap();
        r
    }

    #[test]
    fn the_scope_names_are_show_strip_reject() {
        assert_eq!(scope_of("show"), Ok(Scope::Show));
        assert_eq!(scope_of("strip"), Ok(Scope::Strip));
        assert_eq!(scope_of("reject"), Ok(Scope::Reject));
        let e = scope_of("drop").unwrap_err();
        assert!(
            e.contains("show") && e.contains("strip") && e.contains("reject"),
            "{e}"
        );
    }

    #[test]
    fn show_keeps_the_text_and_reports_nothing() {
        let s = apply_scope(PARTIES, &no_parties(), Scope::Show).unwrap();
        assert_eq!(s.text, PARTIES);
        assert!(s.removed.is_empty());
    }

    #[test]
    fn strip_drops_the_sections_in_every_spelling_and_reports_them() {
        let s = apply_scope(PARTIES, &no_parties(), Scope::Strip).unwrap();
        let v: Value = serde_json::from_str(&s.text).unwrap();
        assert_eq!(
            v,
            json!({"@type":"Set","uid":"urn:p:1","permission":[{"action":"use"}]})
        );
        let paths: Vec<&str> = s.removed.iter().map(|o| o.path.as_str()).collect();
        assert_eq!(paths, ["assigner", "permission[0].odrl:assignee"]);
        let j: Value = serde_json::from_str(&removed_json(&s.removed)).unwrap();
        assert_eq!(
            j[1],
            json!({"path":"permission[0].odrl:assignee","key":"odrl:assignee","limit":"ruleAssignee"})
        );
    }

    #[test]
    fn strip_and_reject_leave_a_document_in_scope_alone() {
        let in_scope = r#"{"@type":"Set","permission":[{"action":"use"}]}"#;
        for mode in [Scope::Strip, Scope::Reject] {
            let s = apply_scope(in_scope, &no_parties(), mode).unwrap();
            assert_eq!(s.text, in_scope);
            assert!(s.removed.is_empty());
        }
    }

    #[test]
    fn reject_names_the_offending_paths() {
        let e = apply_scope(PARTIES, &no_parties(), Scope::Reject).unwrap_err();
        assert_eq!(e.paths, ["assigner", "permission[0].odrl:assignee"]);
        assert!(e.message.contains("assigner"), "{}", e.message);
        assert!(
            e.message.contains("permission[0].odrl:assignee"),
            "{}",
            e.message
        );
        assert!(e.message.contains("ruleAssignee"), "{}", e.message);
    }

    #[test]
    fn unreadable_text_passes_through_for_the_reader_to_refuse() {
        for mode in [Scope::Strip, Scope::Reject] {
            let s = apply_scope("not json", &no_parties(), mode).unwrap();
            assert_eq!(s.text, "not json");
        }
    }
}
