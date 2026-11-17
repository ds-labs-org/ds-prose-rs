//! Validation for JavaScript hosts, without the DOM: normalising the options
//! a host passes, reading a list of profiles, and the report as the JSON
//! text every host receives (the same bytes in Rust, Node and Angular).
//!
//! `lib.rs` exposes these through wasm-bindgen as `validate`, `rules`,
//! `ProseValidator` and the `ProseHandle` methods; everything here is plain
//! Rust and tested on the host.
use prose_core::edit::EntityRole;
use prose_core::profile::{
    OdrlVocab, Overlay, ProfileError, ProfileVocab, SourceValue, ValueSource, ValueSources, Wording,
};
use prose_core::shapes::ShapesGraph;
use prose_core::validate::{
    Basis, Level, Options, RULES, RULESET, RuleId, RuleInfo, Tier, Validator,
};
use serde_json::{Map, Value, json};
use std::sync::Arc;

/// The TypeScript declarations of the validation API, added to the
/// package's `.d.ts`.
pub const TYPES_D_TS: &str = include_str!("types.d.ts");

/// The options keys a host may pass.
const OPTION_KEYS: [&str; 7] = [
    "strict",
    "structural",
    "failClosed",
    "denyDeprecated",
    "odrl",
    "ruleset",
    "rules",
];

fn flag(o: &Map<String, Value>, key: &str) -> Result<Option<bool>, String> {
    match o.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(v) => Err(format!("options.{key} must be true or false, not {v}")),
    }
}

/// Reads the options a host passes (`undefined`, `null` and `{}` all mean
/// the defaults). Strict is the default here: these entry points exist to
/// validate; `strict: false` asks for the advisory reading.
///
/// Anything the host got wrong is an error naming it: an unknown key, a
/// value of the wrong type, a ruleset this build does not have, a rule id
/// that is not in the catalogue.
pub fn options_from_value(v: Option<&Value>) -> Result<Options, String> {
    let o = match v {
        None | Some(Value::Null) => return Ok(Options::strict()),
        Some(Value::Object(o)) => o,
        Some(v) => return Err(format!("options must be an object, not {v}")),
    };
    if let Some(k) = o.keys().find(|k| !OPTION_KEYS.contains(&k.as_str())) {
        return Err(format!(
            "options.{k} is not an option (options are {})",
            OPTION_KEYS.join(", ")
        ));
    }
    let mut out = if flag(o, "strict")?.unwrap_or(true) {
        Options::strict()
    } else {
        Options::advisory()
    };
    if let Some(b) = flag(o, "structural")? {
        out = out.structural(b);
    }
    if let Some(b) = flag(o, "failClosed")? {
        out = out.fail_closed(b);
    }
    if let Some(b) = flag(o, "denyDeprecated")? {
        out = out.deny_deprecated(b);
    }
    match o.get("odrl") {
        None | Some(Value::Null) => {}
        Some(Value::String(s)) if s == "common" => out = out.odrl_vocabulary(OdrlVocab::Common),
        Some(Value::String(s)) if s == "core" => out = out.odrl_vocabulary(OdrlVocab::Core),
        Some(v) => {
            return Err(format!(
                "options.odrl must be \"common\" or \"core\", not {v}"
            ));
        }
    }
    match o.get("ruleset") {
        None | Some(Value::Null) => {}
        Some(v) => match v.as_u64() {
            Some(n) if (1..=u64::from(RULESET)).contains(&n) => out = out.ruleset(n as u32),
            _ => {
                return Err(format!(
                    "options.ruleset must be a whole number from 1 to {RULESET} (this build's rulesets), not {v}"
                ));
            }
        },
    }
    match o.get("rules") {
        None | Some(Value::Null) => {}
        Some(Value::Object(rules)) => {
            for (id, level) in rules {
                let rule: RuleId = id
                    .parse()
                    .map_err(|_| format!("options.rules: no rule is called \"{id}\""))?;
                let level = match level.as_str() {
                    Some("off") => Level::Off,
                    Some("info") => Level::Info,
                    Some("warning") => Level::Warning,
                    Some("error") => Level::Error,
                    _ => {
                        return Err(format!(
                            "options.rules[\"{id}\"] must be \"off\", \"info\", \"warning\" or \"error\", not {level}"
                        ));
                    }
                };
                out = out.level(rule, level);
            }
        }
        Some(v) => return Err(format!("options.rules must be an object, not {v}")),
    }
    Ok(out)
}

/// The profile texts a host passes: one string, a list of strings, or
/// nothing.
pub fn profile_list(v: &Value) -> Result<Vec<String>, String> {
    match v {
        Value::Null => Ok(Vec::new()),
        Value::String(s) => Ok(vec![s.clone()]),
        Value::Array(a) => a
            .iter()
            .enumerate()
            .map(|(i, x)| {
                x.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| format!("profile {} is not a string", i + 1))
            })
            .collect(),
        _ => Err("profiles must be a string, a list of strings or null".to_string()),
    }
}

/// The message a host sees for a profile that cannot be read. Without the
/// `turtle` feature, a text that is not JSON-LD also names the build of the
/// npm package that reads Turtle.
pub fn profile_error(e: &ProfileError) -> String {
    match e {
        #[cfg(not(feature = "turtle"))]
        ProfileError::UnsupportedFormat => format!(
            "{e}; or use the build that reads Turtle, @ds-labs/prose-wasm/turtle (prose_wasm_turtle_bg.wasm)"
        ),
        _ => e.to_string(),
    }
}

/// Reads each text as a profile (JSON-LD, or Turtle with the `turtle`
/// feature) into one wording. The error names the position of the text that
/// is not a profile.
pub fn wording(texts: &[String]) -> Result<Wording, String> {
    let mut w = Wording::new();
    for (i, t) in texts.iter().enumerate() {
        let p = ProfileVocab::parse(t)
            .map_err(|e| format!("profile {}: {}", i + 1, profile_error(&e)))?;
        w = w.with_profile(p);
    }
    Ok(w)
}

/// A validator for the profiles in `texts`, prepared once.
pub fn validator(texts: &[String], options: Options) -> Result<Validator, String> {
    Ok(Validator::new(Arc::new(wording(texts)?), options))
}

/// Reads `text` as companion SHACL shapes (JSON-LD, or Turtle with the
/// `turtle` feature). The error starts with `shapes: `, so a host can tell
/// it from a profile's.
pub fn shapes(text: &str) -> Result<ShapesGraph, String> {
    ShapesGraph::parse(text).map_err(|e| format!("shapes: {}", shapes_error(&e)))
}

/// The message for shapes that cannot be read. Without the `turtle`
/// feature, a text that is not JSON-LD also names the build that reads Turtle.
fn shapes_error(e: &prose_core::shapes::ShapesError) -> String {
    match e {
        #[cfg(not(feature = "turtle"))]
        prose_core::shapes::ShapesError::UnsupportedFormat => format!(
            "{e}; or use the build that reads Turtle, @ds-labs/prose-wasm/turtle (prose_wasm_turtle_bg.wasm)"
        ),
        _ => e.to_string(),
    }
}

/// What `setShapes` returns for loaded shapes, as JSON text: `constraints`
/// (how many left operands a shape selects), `actions` and `leftOperands`
/// (the expanded IRIs the shapes close the policy to, `null` when they do
/// not) and `warnings` (what was not read).
pub fn shapes_summary_json(s: &ShapesGraph) -> String {
    json!({
        "constraints": s.constraints().count(),
        "actions": s.closed_actions(),
        "leftOperands": s.closed_left_operands(),
        "warnings": s.warnings(),
    })
    .to_string()
}

/// `base` with `shapes` laid over it, when there are some.
pub fn with_shapes(base: Wording, shapes: Option<&ShapesGraph>) -> Wording {
    match shapes {
        Some(s) => base.with_shapes(s.clone()),
        None => base,
    }
}

/// Reads `text` as an operator overlay (JSON-LD, or Turtle with the `turtle`
/// feature). The error starts with `overlay: `, so a host can tell it from a
/// profile's or the shapes'.
pub fn overlay(text: &str) -> Result<Overlay, String> {
    Overlay::parse(text).map_err(|e| format!("overlay: {}", overlay_error(&e)))
}

/// The message for an overlay that cannot be read; see [`shapes_error`].
fn overlay_error(e: &prose_core::profile::OverlayError) -> String {
    match e {
        #[cfg(not(feature = "turtle"))]
        prose_core::profile::OverlayError::UnsupportedFormat => format!(
            "{e}; or use the build that reads Turtle, @ds-labs/prose-wasm/turtle (prose_wasm_turtle_bg.wasm)"
        ),
        _ => e.to_string(),
    }
}

/// Reads every text as an overlay. The error names which one when there are
/// several.
pub fn overlays(texts: &[String]) -> Result<Vec<Overlay>, String> {
    texts
        .iter()
        .enumerate()
        .map(|(i, t)| {
            overlay(t).map_err(|e| match texts.len() {
                1 => e,
                _ => e.replacen("overlay: ", &format!("overlay {}: ", i + 1), 1),
            })
        })
        .collect()
}

/// What `setOverlays` returns, as the JSON text of a list with one entry per
/// overlay: `terms` (how many terms it names) and `warnings` (what was not
/// read, or cannot be used with the profiles in `wording`: a term no profile
/// declares as a left operand is ignored).
pub fn overlay_summaries_json(wording: &Wording, overlays: &[Overlay]) -> String {
    let items: Vec<Value> = overlays
        .iter()
        .map(|o| {
            json!({
                "terms": o.terms().count(),
                "warnings": wording.overlay_warnings(o),
            })
        })
        .collect();
    Value::Array(items).to_string()
}

/// `base` with `overlays` laid over it, in order.
pub fn with_overlays(base: Wording, overlays: &[Overlay]) -> Wording {
    overlays.iter().fold(base, |w, o| w.with_overlay(o.clone()))
}

/// [`validator`], with companion shapes and operator overlays, given as
/// texts (none when `None`, or empty).
pub fn validator_with(
    texts: &[String],
    shapes_text: Option<&str>,
    overlay_texts: &[String],
    options: Options,
) -> Result<Validator, String> {
    let shapes = shapes_text.map(shapes).transpose()?;
    let overlays = overlays(overlay_texts)?;
    let w = with_shapes(with_overlays(wording(texts)?, &overlays), shapes.as_ref());
    Ok(Validator::new(Arc::new(w), options))
}

/// Where a host's value source applies: the IRI of a left operand, or the
/// parties (the assets, for the target) of a role.
enum SourceKey {
    Operand(String),
    Role(EntityRole),
}

/// A key a host passes: a party role by its bare name (`"assigner"`,
/// `"assignee"`, `"target"`) or by ODRL's IRI for it; anything else is the
/// IRI of a left operand.
fn source_key(key: &str) -> Result<SourceKey, String> {
    const ODRL: &str = "http://www.w3.org/ns/odrl/2/";
    if key.trim().is_empty() {
        return Err("valueSources: a key is empty; use a left operand's IRI, or \"assigner\", \"assignee\" or \"target\"".into());
    }
    let bare = key.strip_prefix(ODRL).unwrap_or(key);
    Ok(match EntityRole::parse(bare) {
        Some(role) => SourceKey::Role(role),
        None => SourceKey::Operand(key.to_string()),
    })
}

/// One value of a source: a string, or `{value, label?, group?}`.
fn source_value(key: &str, v: &Value) -> Result<SourceValue, String> {
    match v {
        Value::String(s) if !s.is_empty() => Ok(SourceValue::new(s)),
        Value::Object(o) => {
            if let Some(bad) = o
                .keys()
                .find(|k| !["value", "label", "group"].contains(&k.as_str()))
            {
                return Err(format!(
                    "valueSources: \"{key}\": a value has no key \"{bad}\" (use value, label, group)"
                ));
            }
            let text = |k: &str| match o.get(k) {
                None | Some(Value::Null) => Ok(None),
                Some(Value::String(s)) => Ok(Some(s.as_str())),
                Some(_) => Err(format!(
                    "valueSources: \"{key}\": \"{k}\" of a value must be a string"
                )),
            };
            let Some(value) = text("value")?.filter(|v| !v.is_empty()) else {
                return Err(format!(
                    "valueSources: \"{key}\": a value needs a \"value\" string"
                ));
            };
            let mut out = SourceValue::new(value);
            if let Some(l) = text("label")?.filter(|l| !l.is_empty()) {
                out = out.with_label(l);
            }
            if let Some(g) = text("group")?.filter(|g| !g.is_empty()) {
                out = out.with_group(g);
            }
            Ok(out)
        }
        _ => Err(format!(
            "valueSources: \"{key}\": a value is a string or {{value, label, group}}"
        )),
    }
}

/// Sets, or with `null` clears, the source `values` for `key` in `sources`.
/// `closed` says whether the values are the only ones strict validation
/// takes (`false`: suggestions). The error starts with `valueSources: `.
pub fn set_value_source(
    sources: &mut ValueSources,
    key: &str,
    values: &Value,
    closed: bool,
) -> Result<(), String> {
    let target = source_key(key)?;
    let source = match values {
        Value::Null => None,
        Value::Array(items) => {
            let list = items
                .iter()
                .map(|v| source_value(key, v))
                .collect::<Result<Vec<_>, _>>()?;
            Some(ValueSource::new(list, closed))
        }
        _ => {
            return Err(format!(
                "valueSources: \"{key}\": the values are a list (or null to clear)"
            ));
        }
    };
    match (target, source) {
        (SourceKey::Operand(iri), Some(s)) => sources.set_operand(&iri, s),
        (SourceKey::Operand(iri), None) => {
            sources.remove_operand(&iri);
        }
        (SourceKey::Role(r), Some(s)) => sources.set_role(r, s),
        (SourceKey::Role(r), None) => {
            sources.remove_role(r);
        }
    }
    Ok(())
}

fn closed_flag(v: Option<&Value>) -> Result<bool, String> {
    match v {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err("valueSources: \"closed\" must be true or false".into()),
    }
}

/// The `closed` of the options a host passes to `setValueSource`
/// (`{closed?: boolean}`; absent is suggest-only).
pub fn source_closed(options: Option<&Value>) -> Result<bool, String> {
    match options {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Object(o)) => {
            if let Some(bad) = o.keys().find(|k| k.as_str() != "closed") {
                return Err(format!(
                    "valueSources: options have no key \"{bad}\" (use closed)"
                ));
            }
            closed_flag(o.get("closed"))
        }
        Some(_) => Err("valueSources: the options are an object, {closed?: boolean}".into()),
    }
}

/// Reads the map a host passes to `setValueSources`: key (a left operand's
/// IRI, or a party role) to a list of values, or to `{values, closed?}`.
pub fn value_sources(v: &Value) -> Result<ValueSources, String> {
    let Value::Object(map) = v else {
        return Err(
            "valueSources: expected an object keyed by left operand IRI or party role".into(),
        );
    };
    let mut out = ValueSources::new();
    for (key, entry) in map {
        match entry {
            Value::Object(o) => {
                if let Some(bad) = o
                    .keys()
                    .find(|k| !["values", "closed"].contains(&k.as_str()))
                {
                    return Err(format!(
                        "valueSources: \"{key}\": no key \"{bad}\" (use values, closed)"
                    ));
                }
                let closed = closed_flag(o.get("closed"))?;
                let values = o.get("values").cloned().unwrap_or(Value::Null);
                if values.is_null() {
                    return Err(format!("valueSources: \"{key}\": missing \"values\""));
                }
                set_value_source(&mut out, key, &values, closed)?;
            }
            other => set_value_source(&mut out, key, other, false)?,
        }
    }
    Ok(out)
}

/// `base` with the host's `sources` laid over it.
pub fn with_value_sources(base: Wording, sources: &ValueSources) -> Wording {
    if sources.is_empty() {
        return base;
    }
    base.with_value_sources(sources.clone())
}

/// What `setValueSources` returns, as the JSON text of an object with one
/// entry per source, by its key: `values` (how many), `closed` and
/// `warnings` (a left operand no loaded profile declares is ignored).
pub fn value_source_summaries_json(wording: &Wording, sources: &ValueSources) -> String {
    let mut out = Map::new();
    for (iri, s) in sources.operands() {
        let warnings: Vec<String> = wording.value_source_warning(iri).into_iter().collect();
        out.insert(
            iri.to_string(),
            json!({"values": s.values().len(), "closed": s.is_closed(), "warnings": warnings}),
        );
    }
    for role in EntityRole::ALL {
        if let Some(s) = sources.role(role) {
            out.insert(
                role.as_str().to_string(),
                json!({"values": s.values().len(), "closed": s.is_closed(), "warnings": []}),
            );
        }
    }
    Value::Object(out).to_string()
}

/// The summary of the source for `key` (see [`value_source_summaries_json`]),
/// as JSON text; `"null"` when there is none.
pub fn value_source_summary_json(wording: &Wording, sources: &ValueSources, key: &str) -> String {
    let name = match source_key(key) {
        Ok(SourceKey::Role(r)) => r.as_str().to_string(),
        _ => key.to_string(),
    };
    let all: Value =
        serde_json::from_str(&value_source_summaries_json(wording, sources)).unwrap_or(Value::Null);
    all.get(&name).cloned().unwrap_or(Value::Null).to_string()
}

/// [`validator_with`], with the host's value sources.
pub fn validator_with_sources(
    texts: &[String],
    shapes_text: Option<&str>,
    overlay_texts: &[String],
    sources: &ValueSources,
    options: Options,
) -> Result<Validator, String> {
    let shapes = shapes_text.map(shapes).transpose()?;
    let overlays = overlays(overlay_texts)?;
    let w = with_shapes(with_overlays(wording(texts)?, &overlays), shapes.as_ref());
    Ok(Validator::new(
        Arc::new(with_value_sources(w, sources)),
        options,
    ))
}

/// [`validator`], with the companion shapes in `shapes` (none when `None`).
pub fn validator_with_shapes(
    texts: &[String],
    shapes_text: Option<&str>,
    options: Options,
) -> Result<Validator, String> {
    let shapes = shapes_text.map(shapes).transpose()?;
    let w = with_shapes(wording(texts)?, shapes.as_ref());
    Ok(Validator::new(Arc::new(w), options))
}

/// The report for `json` as `ds-prose/validation-report@1` text. Never
/// fails: a text that is not a policy is an `input.unreadable` finding.
pub fn validate_text(json: &str, v: &Validator) -> String {
    v.validate_json(json).to_json().to_string()
}

fn level(l: Level) -> &'static str {
    match l {
        Level::Off => "off",
        Level::Info => "info",
        Level::Warning => "warning",
        Level::Error => "error",
    }
}

fn tier(t: Tier) -> &'static str {
    match t {
        Tier::Profile => "profile",
        Tier::Odrl => "odrl",
        Tier::Prose => "prose",
        Tier::Input => "input",
        Tier::Heuristic => "heuristic",
        _ => "unknown",
    }
}

fn basis(b: Basis) -> &'static str {
    match b {
        Basis::Standard => "standard",
        Basis::StandardDerived => "standard-derived",
        Basis::Convention => "convention",
        Basis::Heuristic => "heuristic",
        Basis::Input => "input",
        _ => "unknown",
    }
}

fn rule(r: &RuleInfo) -> Value {
    json!({
        "id": r.id.as_str(),
        "tier": tier(r.tier),
        "title": r.title,
        "basis": basis(r.basis),
        "source": r.source,
        "sinceRuleset": r.since_ruleset,
        "advisory": level(r.advisory),
        "strict": level(r.strict),
        "strictClosed": r.strict_closed.map(level),
        "gap": r.gap,
        "refusable": r.refusable,
    })
}

/// The rule catalogue as JSON text: an array of `RuleInfo`.
pub fn rules_json() -> String {
    Value::Array(RULES.iter().map(rule).collect()).to_string()
}
