//! Validation for JavaScript hosts, without the DOM: normalising the options
//! a host passes, reading a list of profiles, and the report as the JSON
//! text every host receives (the same bytes in Rust, Node and Angular).
//!
//! `lib.rs` exposes these through wasm-bindgen as `validate`, `rules`,
//! `ProseValidator` and the `ProseHandle` methods; everything here is plain
//! Rust and tested on the host.
use prose_core::profile::{OdrlVocab, ProfileVocab, Wording};
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

/// Reads each text as a profile (JSON-LD, or Turtle) into one wording. The
/// error names the position of the text that is not a profile.
pub fn wording(texts: &[String]) -> Result<Wording, String> {
    let mut w = Wording::new();
    for (i, t) in texts.iter().enumerate() {
        let p = ProfileVocab::parse(t).map_err(|e| format!("profile {}: {e}", i + 1))?;
        w = w.with_profile(p);
    }
    Ok(w)
}

/// A validator for the profiles in `texts`, prepared once.
pub fn validator(texts: &[String], options: Options) -> Result<Validator, String> {
    Ok(Validator::new(Arc::new(wording(texts)?), options))
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
