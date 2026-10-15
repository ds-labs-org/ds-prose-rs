//! The pure half of the wasm validation API: option normalisation, profile
//! lists, the report text and the TypeScript declarations. Everything here
//! runs on the host; `tests/validate_wasm.rs` mounts the same thing in a
//! browser.
use prose_core::validate::{Level, RULES, RULESET, RuleId};
use prose_wasm::validation::*;
use serde_json::{Value, json};

const RESEARCH: &str = include_str!("../../prose-core/tests/profiles/research.jsonld");
const RESEARCH_TTL: &str = include_str!("../../prose-core/tests/profiles/research.ttl");
const CLOSED: &str = include_str!("../../prose-core/tests/profiles/research-closed.jsonld");
const OTHER: &str = include_str!("../../prose-core/tests/profiles/other.jsonld");
const CLEAN: &str = include_str!("../../prose-core/tests/strict/clean.json");
const NOT_LOADED: &str = include_str!("../../prose-core/tests/strict/not-loaded.json");
const VIOLATING: &str = include_str!("../../prose-core/tests/strict/violating.json");
const MIXED: &str = include_str!("../../prose-core/tests/strict/mixed-two-profiles.json");
const GOLDEN_CLEAN: &str = include_str!("../../prose-core/tests/golden_validate/report/clean.json");
const GOLDEN_NOT_LOADED: &str =
    include_str!("../../prose-core/tests/golden_validate/report/not-loaded.json");
const GOLDEN_VIOLATING: &str =
    include_str!("../../prose-core/tests/golden_validate/report/violating-closed.json");

fn opts(v: Value) -> prose_core::validate::Options {
    options_from_value(Some(&v)).expect("valid options")
}

fn report(json: &str, profiles: &[&str], o: Value) -> Value {
    let texts: Vec<String> = profiles.iter().map(|s| s.to_string()).collect();
    let v = validator(&texts, opts(o)).expect("profiles read");
    serde_json::from_str(&validate_text(json, &v)).expect("the report is JSON")
}

/// The report with the crate version masked, as the core goldens do.
fn masked(mut v: Value) -> Value {
    v["tool"]["version"] = json!("<version>");
    v
}

fn golden(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

#[test]
fn validate_matches_the_core_goldens() {
    let r = report(CLEAN, &[RESEARCH], json!({}));
    assert_eq!(masked(r), golden(GOLDEN_CLEAN));
    let r = report(NOT_LOADED, &[RESEARCH], json!({}));
    assert_eq!(masked(r), golden(GOLDEN_NOT_LOADED));
    let r = report(VIOLATING, &[CLOSED], json!({}));
    assert_eq!(masked(r), golden(GOLDEN_VIOLATING));
}

#[test]
fn report_is_the_same_bytes_as_the_core_makes() {
    let texts = vec![RESEARCH.to_string()];
    let v = validator(&texts, opts(json!({"strict": true}))).unwrap();
    let w = prose_core::profile::Wording::new()
        .with_profile(prose_core::profile::ProfileVocab::parse(RESEARCH).unwrap());
    let core =
        prose_core::validate::validate_json(CLEAN, &w, &prose_core::validate::Options::strict())
            .to_json()
            .to_string();
    assert_eq!(validate_text(CLEAN, &v), core);
}

#[test]
fn options_default_to_strict() {
    let o = options_from_value(None).unwrap();
    assert_eq!(o, prose_core::validate::Options::strict());
    assert_eq!(options_from_value(Some(&Value::Null)).unwrap(), o);
    assert_eq!(options_from_value(Some(&json!({}))).unwrap(), o);
    assert_eq!(
        opts(json!({"strict": false})),
        prose_core::validate::Options::advisory()
    );
}

#[test]
fn every_option_is_read() {
    use prose_core::profile::OdrlVocab;
    let o = opts(json!({
        "structural": true, "failClosed": true, "denyDeprecated": true,
        "odrl": "core", "ruleset": 1,
        "rules": {"profile.unknown-term": "warning", "prose.unit-not-allowed": "off"},
    }));
    let want = prose_core::validate::Options::strict()
        .structural(true)
        .fail_closed(true)
        .deny_deprecated(true)
        .odrl_vocabulary(OdrlVocab::Core)
        .ruleset(1)
        .level(RuleId::ProfileUnknownTerm, Level::Warning)
        .level(RuleId::ProseUnitNotAllowed, Level::Off);
    assert_eq!(o, want);
    assert_eq!(
        opts(json!({"odrl": "common"})),
        prose_core::validate::Options::strict()
    );
}

#[test]
fn bad_options_are_named() {
    let bad = [
        (json!("strict"), "object"),
        (json!({"strictt": true}), "strictt"),
        (json!({"strict": "yes"}), "strict"),
        (json!({"structural": 1}), "structural"),
        (json!({"odrl": "all"}), "odrl"),
        (json!({"ruleset": 0}), "ruleset"),
        (json!({"ruleset": RULESET + 1}), "ruleset"),
        (json!({"ruleset": 1.5}), "ruleset"),
        (json!({"ruleset": "1"}), "ruleset"),
        (json!({"rules": []}), "rules"),
        (json!({"rules": {"profile.nope": "off"}}), "profile.nope"),
        (json!({"rules": {"profile.unknown-term": "loud"}}), "loud"),
        (
            json!({"rules": {"profile.unknown-term": 3}}),
            "profile.unknown-term",
        ),
    ];
    for (v, needle) in bad {
        let e = options_from_value(Some(&v)).expect_err(&v.to_string());
        assert!(e.contains(needle), "{v}: {e}");
    }
}

#[test]
fn profiles_are_a_string_a_list_or_nothing() {
    assert_eq!(profile_list(&Value::Null).unwrap(), Vec::<String>::new());
    assert_eq!(profile_list(&json!("a")).unwrap(), vec!["a".to_string()]);
    assert_eq!(
        profile_list(&json!(["a", "b"])).unwrap(),
        vec!["a".to_string(), "b".to_string()]
    );
    for bad in [json!(1), json!([1]), json!({"a": 1}), json!(["a", null])] {
        assert!(profile_list(&bad).is_err(), "{bad}");
    }
}

#[test]
fn unreadable_profiles_name_their_position() {
    let texts = vec![RESEARCH.to_string(), "<!doctype html><p>hi".to_string()];
    let e = validator(&texts, opts(json!({}))).unwrap_err();
    assert!(e.contains("profile 2"), "{e}");
    let e = wording(&["{".to_string()]).unwrap_err();
    assert!(e.contains("profile 1"), "{e}");
}

#[cfg(feature = "turtle")]
#[test]
fn json_and_turtle_profiles_validate_alike() {
    let a = report(CLEAN, &[RESEARCH], json!({}));
    let b = report(CLEAN, &[RESEARCH_TTL], json!({}));
    assert_eq!(a, b);
    assert_eq!(a["verdict"], "conforms");
}

/// The default npm file has no Turtle reader: the message says which file
/// has one.
#[cfg(not(feature = "turtle"))]
#[test]
fn turtle_profile_names_the_turtle_build() {
    let e = wording(&[RESEARCH_TTL.to_string()]).unwrap_err();
    assert!(e.contains("profile 1"), "{e}");
    assert!(e.contains("@ds-labs/prose-wasm/turtle"), "{e}");
}

#[test]
fn incomplete_when_profile_missing() {
    let r = report(NOT_LOADED, &[], json!({}));
    assert_eq!(r["verdict"], "incomplete");
    assert_eq!(r["gaps"][0]["rule"], "profile.not-loaded");
    // With nothing loaded, a clean policy is incomplete too, never a pass.
    let r = report(CLEAN, &[], json!({}));
    assert_ne!(r["verdict"], "conforms");
}

#[test]
fn two_profiles_conform_together() {
    let r = report(MIXED, &[RESEARCH, OTHER], json!({}));
    assert_eq!(r["verdict"], "conforms", "{r}");
    let r = report(MIXED, &[RESEARCH], json!({}));
    assert_ne!(r["verdict"], "conforms");
}

#[test]
fn a_policy_that_is_not_json_is_a_finding_not_an_error() {
    let r = report("not json", &[], json!({}));
    assert_eq!(r["verdict"], "violations");
    assert_eq!(r["findings"][0]["rule"], "input.unreadable");
}

#[test]
fn advisory_option_gives_the_advisory_report() {
    let r = report(CLEAN, &[RESEARCH], json!({"strict": false}));
    assert_eq!(r["mode"], "advisory");
}

/// Every field of every rule reaches the host as the core states it.
#[test]
fn rules_carry_every_field_of_the_core_catalogue() {
    use prose_core::validate::{Basis, Tier};
    let v: Value = serde_json::from_str(&rules_json()).unwrap();
    let lvl = |l: Level| match l {
        Level::Off => "off",
        Level::Info => "info",
        Level::Warning => "warning",
        Level::Error => "error",
    };
    for (j, r) in v.as_array().unwrap().iter().zip(RULES) {
        let id = r.id.as_str();
        let tier = match r.tier {
            Tier::Profile => "profile",
            Tier::Odrl => "odrl",
            Tier::Prose => "prose",
            Tier::Input => "input",
            Tier::Heuristic => "heuristic",
            _ => unreachable!("a new tier needs a name here"),
        };
        let basis = match r.basis {
            Basis::Standard => "standard",
            Basis::StandardDerived => "standard-derived",
            Basis::Convention => "convention",
            Basis::Heuristic => "heuristic",
            Basis::Input => "input",
            _ => unreachable!("a new basis needs a name here"),
        };
        assert_eq!(j["id"], id);
        assert_eq!(j["tier"], tier, "{id}");
        assert_eq!(j["title"], r.title, "{id}");
        assert_eq!(j["basis"], basis, "{id}");
        assert_eq!(j["source"], r.source, "{id}");
        assert_eq!(j["sinceRuleset"], r.since_ruleset, "{id}");
        assert_eq!(j["advisory"], lvl(r.advisory), "{id}");
        assert_eq!(j["strict"], lvl(r.strict), "{id}");
        assert_eq!(
            j["strictClosed"],
            r.strict_closed.map_or(Value::Null, |l| json!(lvl(l))),
            "{id}"
        );
        assert_eq!(j["gap"], r.gap, "{id}");
        assert_eq!(j["refusable"], r.refusable, "{id}");
    }
    assert!(
        RULES.iter().any(|r| r.refusable) && RULES.iter().any(|r| r.gap),
        "the catalogue must exercise both flags for this test to mean anything"
    );
}

#[test]
fn rules_lists_the_catalogue() {
    let v: Value = serde_json::from_str(&rules_json()).unwrap();
    let list = v.as_array().expect("an array");
    assert_eq!(list.len(), RULES.len());
    let first = &list[0];
    for key in [
        "id",
        "tier",
        "title",
        "basis",
        "source",
        "sinceRuleset",
        "advisory",
        "strict",
        "strictClosed",
        "gap",
        "refusable",
    ] {
        assert!(first.get(key).is_some(), "rules()[0] has no {key}");
    }
    let ids: Vec<&str> = list.iter().map(|r| r["id"].as_str().unwrap()).collect();
    let want: Vec<&str> = RULES.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, want);
    let unit = list
        .iter()
        .find(|r| r["id"] == "prose.unit-not-allowed")
        .unwrap();
    assert_eq!(unit["tier"], "prose");
    assert_eq!(unit["basis"], "convention");
    assert_eq!(unit["strict"], "info");
    assert_eq!(unit["strictClosed"], "error");
    assert_eq!(list[0]["strictClosed"], Value::Null);
}

/// The members of the `type X = | "a" | "b"` union named `name` in the d.ts.
fn union(dts: &str, name: &str) -> Vec<String> {
    let start = dts
        .find(&format!("type {name} ="))
        .unwrap_or_else(|| panic!("no type {name}"));
    let rest = &dts[start..];
    let end = rest.find(';').expect("the union ends");
    rest[..end]
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

#[test]
fn rules_match_dts_union() {
    let mut ts = union(TYPES_D_TS, "RuleId");
    let mut rs: Vec<String> = RULES.iter().map(|r| r.id.as_str().to_string()).collect();
    ts.sort();
    rs.sort();
    assert_eq!(ts, rs, "RuleId in the declarations is the catalogue");
}

#[test]
fn dts_unions_match_the_report() {
    let mut verdicts = union(TYPES_D_TS, "Verdict");
    verdicts.sort();
    assert_eq!(verdicts, ["conforms", "incomplete", "violations"]);
    let mut sev = union(TYPES_D_TS, "Severity");
    sev.sort();
    assert_eq!(sev, ["error", "info", "warning"]);
    let mut levels = union(TYPES_D_TS, "RuleLevel");
    levels.sort();
    assert_eq!(levels, ["error", "info", "off", "warning"]);
}

#[test]
fn dts_names_every_key_of_the_report() {
    let r = report(VIOLATING, &[CLOSED], json!({}));
    for key in r.as_object().unwrap().keys() {
        assert!(
            TYPES_D_TS.contains(&format!("{key}:")),
            "ValidationReport.{key}"
        );
    }
    for key in r["findings"][0].as_object().unwrap().keys() {
        assert!(TYPES_D_TS.contains(&format!("{key}:")), "Finding.{key}");
    }
    for key in [
        "strict",
        "structural",
        "failClosed",
        "denyDeprecated",
        "odrl",
        "ruleset",
        "rules",
    ] {
        assert!(
            TYPES_D_TS.contains(&format!("{key}?:")),
            "ValidateOptions.{key}"
        );
    }
}
