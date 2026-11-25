//! The pure half of the wasm validation API: option normalisation, profile
//! lists, the report text and the TypeScript declarations. Everything here
//! runs on the host; `tests/validate_wasm.rs` mounts the same thing in a
//! browser.
use prose_core::validate::{Level, RULES, RULESET, RuleId};
use prose_wasm::validation::*;
use serde_json::{Value, json};

mod common;
use common::{CLASS_SHAPES, chain_profile};

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

// --- companion shapes ----------------------------------------------------------

const DX_PROFILE: &str = include_str!("../../prose-core/tests/shapes/dataspace-profile.jsonld");
const DX_SHAPES: &str = include_str!("../../prose-core/tests/shapes/dataspace-shapes.jsonld");

const DX_POLICY: &str = r#"{
  "@context": ["http://www.w3.org/ns/odrl.jsonld", {"dx": "https://example.org/dataspace/"}],
  "@type": "Set", "uid": "urn:p", "profile": "https://example.org/dataspace/profile",
  "permission": [{"target": "urn:a", "action": "dx:train", "constraint": [
    {"leftOperand": "dx:purpose", "operator": "eq", "rightOperand": {"@id": "dx:clinical"}}
  ]}]
}"#;

fn shaped_report(shapes: Option<&str>) -> Value {
    let texts = vec![DX_PROFILE.to_string()];
    let v = validator_with_shapes(&texts, shapes, opts(json!({}))).expect("reads");
    serde_json::from_str(&validate_text(DX_POLICY, &v)).expect("the report is JSON")
}

#[test]
fn shapes_summary_says_what_was_read() {
    let s = shapes(DX_SHAPES).expect("shapes");
    let v: Value = serde_json::from_str(&shapes_summary_json(&s, None)).unwrap();
    assert_eq!(v["constraints"], 5);
    assert_eq!(
        v["actions"],
        json!([
            "https://example.org/dataspace/train",
            "https://example.org/dataspace/share"
        ])
    );
    assert_eq!(v["leftOperands"].as_array().unwrap().len(), 5);
    assert_eq!(v["warnings"], json!([]));
    let open = shapes(
        r#"{"@context":{"sh":"http://www.w3.org/ns/shacl#"},"@id":"http://e.org/S","sh:closed":true,"sh:property":{"sh:path":{"@id":"http://e.org/p"}}}"#,
    )
    .unwrap();
    let v: Value = serde_json::from_str(&shapes_summary_json(&open, None)).unwrap();
    assert_eq!(v["actions"], Value::Null, "no closed list");
    assert_eq!(v["leftOperands"], Value::Null);
    assert!(v["warnings"].as_array().unwrap().len() >= 2, "{v}");
}

#[test]
fn shapes_summary_lists_what_the_profiles_make_unusable() {
    let s = shapes(CLASS_SHAPES).expect("shapes");
    let deep = wording(&[chain_profile(70)]).expect("profile");
    let v: Value =
        serde_json::from_str(&shapes_summary_json(&s, Some(&with_shapes(deep, Some(&s))))).unwrap();
    let w = v["warnings"].as_array().unwrap();
    assert!(
        w.iter().any(|m| m.as_str().unwrap().contains("sh:class")
            && m.as_str().unwrap().contains("more members")),
        "{v}"
    );
    // A chain within the limits is read whole: nothing to say.
    let ok = wording(&[chain_profile(5)]).expect("profile");
    let v: Value =
        serde_json::from_str(&shapes_summary_json(&s, Some(&with_shapes(ok, Some(&s))))).unwrap();
    assert_eq!(v["warnings"], json!([]), "{v}");
}

#[test]
fn a_validator_with_shapes_names_the_shapes_in_its_findings() {
    let without = shaped_report(None);
    assert_eq!(without["verdict"], "conforms");
    let with = shaped_report(Some(DX_SHAPES));
    assert_eq!(with["verdict"], "violations");
    let f = &with["findings"][0];
    assert_eq!(f["rule"], "prose.value-not-allowed");
    assert_eq!(f["expected"]["source"], "shapes");
    assert!(f["message"].as_str().unwrap().contains("the shapes allow"));
}

#[test]
fn unreadable_shapes_say_what_is_wrong() {
    for bad in ["", "{", "<!doctype html><p>hi", DX_PROFILE] {
        let e = shapes(bad).unwrap_err();
        assert!(e.starts_with("shapes: "), "{e}");
    }
    let e = validator_with_shapes(&[DX_PROFILE.to_string()], Some("{"), opts(json!({})))
        .map(|_| ())
        .unwrap_err();
    assert!(e.starts_with("shapes: "), "{e}");
}

#[test]
fn dts_declares_the_shapes_api() {
    for name in [
        "ShapesSummary",
        "source?:",
        "pattern?:",
        "minInclusive?:",
        "maxInclusive?:",
    ] {
        assert!(TYPES_D_TS.contains(name), "{name}");
    }
}

// --- operator overlays ---------------------------------------------------------

const DX_OVERLAY: &str = include_str!("../../prose-core/tests/overlay/dataspace-overlay.jsonld");

fn overlaid_report(overlays: &[&str]) -> Value {
    let texts = vec![DX_PROFILE.to_string()];
    let overlays: Vec<String> = overlays.iter().map(|s| s.to_string()).collect();
    let v = validator_with(&texts, None, &overlays, opts(json!({}))).expect("reads");
    serde_json::from_str(&validate_text(DX_POLICY, &v)).expect("the report is JSON")
}

#[test]
fn overlay_summaries_say_what_was_read_and_what_was_ignored() {
    let o = overlays(&[DX_OVERLAY.to_string()]).expect("overlay");
    let w = with_overlays(wording(&[DX_PROFILE.to_string()]).unwrap(), &o);
    let v: Value = serde_json::from_str(&overlay_summaries_json(&w, &o)).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert_eq!(v[0]["terms"], 3);
    assert_eq!(v[0]["warnings"], json!([]));
    // A term no loaded profile declares is ignored, with a warning.
    let ghost = r#"{"@context":{"prose":"https://ds-labs-org.github.io/ds-prose-rs/profile#"},"@id":"https://example.org/dataspace/ghost","prose:closed":true}"#;
    let o = overlays(&[ghost.to_string()]).expect("overlay");
    let w = with_overlays(wording(&[DX_PROFILE.to_string()]).unwrap(), &o);
    let v: Value = serde_json::from_str(&overlay_summaries_json(&w, &o)).unwrap();
    assert!(
        v[0]["warnings"][0].as_str().unwrap().contains("ghost"),
        "{v}"
    );
}

#[test]
fn a_validator_with_an_overlay_names_the_overlay_in_its_findings() {
    assert_eq!(overlaid_report(&[])["verdict"], "conforms");
    let with = overlaid_report(&[DX_OVERLAY]);
    assert_eq!(with["verdict"], "violations");
    let f = &with["findings"][0];
    assert_eq!(f["rule"], "prose.value-not-allowed");
    assert_eq!(f["expected"]["source"], "overlay");
    assert!(
        f["message"]
            .as_str()
            .unwrap()
            .contains("the overlay allows")
    );
}

#[test]
fn shapes_win_over_an_overlay_in_one_validator() {
    let shapes = r#"{"@context":{"sh":"http://www.w3.org/ns/shacl#","odrl":"http://www.w3.org/ns/odrl/2/","dx":"https://example.org/dataspace/"},
      "@id":"dx:S","sh:property":[{"sh:path":{"@id":"odrl:leftOperand"},"sh:hasValue":{"@id":"dx:purpose"}},
      {"sh:path":{"@id":"odrl:rightOperand"},"sh:in":{"@list":[{"@id":"dx:marketing"}]}}]}"#;
    let v = validator_with(
        &[DX_PROFILE.to_string()],
        Some(shapes),
        &[DX_OVERLAY.to_string()],
        opts(json!({})),
    )
    .unwrap();
    let r: Value = serde_json::from_str(&validate_text(DX_POLICY, &v)).unwrap();
    assert_eq!(r["findings"][0]["expected"]["source"], "shapes");
}

#[test]
fn unreadable_overlays_say_what_is_wrong() {
    for bad in ["", "{", "<!doctype html><p>hi", DX_PROFILE] {
        let e = overlay(bad).unwrap_err();
        assert!(e.starts_with("overlay: "), "{e}");
    }
    let e = validator_with(
        &[DX_PROFILE.to_string()],
        None,
        &["{".to_string()],
        opts(json!({})),
    )
    .map(|_| ())
    .unwrap_err();
    assert!(e.starts_with("overlay: "), "{e}");
}

#[test]
fn dts_declares_the_overlay_api() {
    for name in ["OverlaySummary", "\"overlay\""] {
        assert!(TYPES_D_TS.contains(name), "{name}");
    }
}

// --- runtime value sources -------------------------------------------------------

const DX: &str = "https://example.org/dataspace/";

fn purposes(closed: bool) -> Value {
    json!({
        format!("{DX}purpose"): {
            "values": [
                {"value": format!("{DX}research"), "label": "Research", "group": "Purposes"},
                format!("{DX}marketing"),
            ],
            "closed": closed
        }
    })
}

fn sourced_report(sources: &Value) -> Value {
    let texts = vec![DX_PROFILE.to_string()];
    let sources = value_sources(sources).expect("reads");
    let v = validator_with_sources(&texts, None, &[], &sources, opts(json!({}))).expect("builds");
    serde_json::from_str(&validate_text(DX_POLICY, &v)).expect("the report is JSON")
}

#[test]
fn a_closed_value_source_names_the_host_in_its_findings() {
    assert_eq!(sourced_report(&json!({}))["verdict"], "conforms");
    let r = sourced_report(&purposes(true));
    assert_eq!(r["verdict"], "violations", "{r}");
    let f = &r["findings"][0];
    assert_eq!(f["rule"], "prose.value-not-allowed");
    assert_eq!(f["expected"]["source"], "host");
    assert!(f["message"].as_str().unwrap().contains("the host allows"));
}

#[test]
fn a_suggest_only_value_source_flags_nothing_as_an_error() {
    let r = sourced_report(&purposes(false));
    assert_ne!(r["verdict"], "violations", "{r}");
}

#[test]
fn a_value_source_takes_text_or_labelled_values_and_a_closed_flag() {
    let s = value_sources(&purposes(true)).expect("reads");
    let src = s.operand(&format!("{DX}purpose")).expect("the source");
    assert!(src.is_closed());
    assert_eq!(src.values().len(), 2);
    assert_eq!(src.values()[0].label(), Some("Research"));
    assert_eq!(src.values()[0].group(), Some("Purposes"));
    assert_eq!(src.values()[1].value(), format!("{DX}marketing"));
    assert_eq!(src.values()[1].label(), None);
    // A bare list is suggest-only.
    let s = value_sources(&json!({ format!("{DX}purpose"): ["a", "b"] })).unwrap();
    assert!(!s.operand(&format!("{DX}purpose")).unwrap().is_closed());
}

#[test]
fn a_value_source_is_addressed_by_left_operand_or_party_role() {
    use prose_core::edit::EntityRole;
    let s = value_sources(&json!({
        "assignee": ["did:web:a"],
        "target": {"values": ["urn:asset:1"], "closed": true},
        "http://www.w3.org/ns/odrl/2/assigner": ["did:web:b"],
        format!("{DX}purpose"): ["x"],
    }))
    .expect("reads");
    assert!(s.role(EntityRole::Assignee).is_some());
    assert!(s.role(EntityRole::Target).unwrap().is_closed());
    assert!(s.role(EntityRole::Assigner).is_some());
    assert_eq!(s.operands().count(), 1);
}

#[test]
fn setting_a_source_to_null_clears_it() {
    let mut s = value_sources(&purposes(true)).unwrap();
    set_value_source(&mut s, &format!("{DX}purpose"), &Value::Null, false).expect("clears");
    assert!(s.is_empty());
    let mut s = value_sources(&json!({"assignee": ["did:web:a"]})).unwrap();
    set_value_source(&mut s, "assignee", &Value::Null, false).unwrap();
    assert!(s.is_empty());
}

#[test]
fn unreadable_value_sources_say_what_is_wrong() {
    for bad in [
        json!("x"),
        json!([1]),
        json!({"": ["a"]}),
        json!({"k": "not a list"}),
        json!({"k": [1]}),
        json!({"k": [{"label": "no value"}]}),
        json!({"k": [{"value": "v", "labell": "typo"}]}),
        json!({"k": {"values": [], "closd": true}}),
        json!({"k": {"values": [], "closed": "yes"}}),
    ] {
        let e = value_sources(&bad).unwrap_err();
        assert!(e.starts_with("valueSources: "), "{bad}: {e}");
    }
}

#[test]
fn value_source_summaries_say_what_was_read_and_what_was_ignored() {
    let mut json = purposes(true);
    json[format!("{DX}ghost")] = json!(["a"]);
    json["assignee"] = json!(["did:web:a", "did:web:b"]);
    let s = value_sources(&json).unwrap();
    let w = with_value_sources(wording(&[DX_PROFILE.to_string()]).unwrap(), &s);
    let v: Value = serde_json::from_str(&value_source_summaries_json(&w, &s)).unwrap();
    assert_eq!(v[format!("{DX}purpose")]["values"], 2, "{v}");
    assert_eq!(v[format!("{DX}purpose")]["closed"], true);
    assert_eq!(v[format!("{DX}purpose")]["warnings"], json!([]));
    assert_eq!(v["assignee"]["values"], 2);
    assert!(
        v[format!("{DX}ghost")]["warnings"][0]
            .as_str()
            .unwrap()
            .contains("ghost"),
        "{v}"
    );
}

#[test]
fn dts_declares_the_value_source_api() {
    for name in [
        "ValueSourceValue",
        "ValueSources",
        "ValueSourceSummary",
        "\"host\"",
    ] {
        assert!(TYPES_D_TS.contains(name), "{name}");
    }
}

#[test]
fn a_policy_type_key_is_not_a_value_source_use_the_limit() {
    for key in ["policyType", "policyTypes", "odrl:policyType"] {
        let err = value_sources(&json!({ key: ["Set"] })).unwrap_err();
        assert!(err.contains("limits.policyTypes"), "{key}: {err}");
        let mut s = Default::default();
        let err = set_value_source(&mut s, key, &json!(["Set"]), true).unwrap_err();
        assert!(err.contains("limits.policyTypes"), "{key}: {err}");
    }
    // Every other key reads as before.
    assert!(value_sources(&json!({ "assignee": ["did:web:a"] })).is_ok());
}
