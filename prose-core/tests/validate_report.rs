//! The validation report as JSON, schema `ds-prose/validation-report@1`:
//! deterministic (no timestamps; findings in document order), with a
//! fingerprint per finding that survives reformatting the input.
mod common;
mod corpus;
use corpus::*;
use prose_core::edit::*;
use prose_core::profile::{OdrlVocab, Wording};
use prose_core::validate::*;
use serde_json::{Value, json};
use std::collections::HashSet;

const RESEARCH: &str = "https://example.org/odrl/profile/research";

fn strict_text(name: &str) -> String {
    std::fs::read_to_string(format!("{}/strict/{name}", tests_dir())).unwrap()
}

/// FNV-1a 64 over the rule id, the target path and the written term (or,
/// without a term, the message), separated by NUL bytes.
fn fnv(rule: &str, path: &str, term: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in [
        rule.as_bytes(),
        b"\0",
        path.as_bytes(),
        b"\0",
        term.as_bytes(),
    ]
    .concat()
    {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// The report JSON with the crate version masked, so a release does not
/// change a golden.
fn masked(r: &Report) -> Value {
    let mut v = r.to_json();
    assert_eq!(v["tool"]["version"], json!(env!("CARGO_PKG_VERSION")));
    v["tool"]["version"] = json!("<version>");
    v
}

#[test]
fn report_json_shape() {
    let r = validate_json(
        &strict_text("not-declared.json"),
        &wording_of(&["research.jsonld"]),
        &Options::strict(),
    );
    let path = "policy[0].permission[0].action[0]#name";
    assert_eq!(
        masked(&r),
        json!({
            "schema": "ds-prose/validation-report@1",
            "tool": {"name": "ds-prose-rs", "version": "<version>"},
            "mode": "strict",
            "tiers": ["profile", "prose", "input"],
            "ruleset": 1,
            "verdict": "violations",
            "vocabulary": {
                "odrl": "common",
                "profiles": [{"iri": RESEARCH, "title": "Research Data Profile", "version": "0.1.0"}]
            },
            "gaps": [],
            "counts": {"error": 1, "warning": 0, "info": 0},
            "findings": [{
                "rule": "profile.not-declared",
                "tier": "profile",
                "basis": "standard-derived",
                "severity": "error",
                "message": format!("\"ex:train\" comes from the profile <{RESEARCH}>, which the policy does not declare"),
                "target": {"kind": "slot", "path": path},
                "pointer": null,
                "term": {"written": "ex:train", "iri": "https://example.org/odrl/profile/research/train"},
                "profile": RESEARCH,
                "expected": null,
                "fixes": [{"label": format!("Declare the profile <{RESEARCH}>"), "safe": true,
                           "replacement": RESEARCH}],
                "fingerprint": fnv("profile.not-declared", path, "ex:train")
            }]
        })
    );
}

#[test]
fn report_json_header_follows_the_options() {
    let w = Wording::new();
    let text = strict_text("clean.json");
    let r = validate_json(&text, &w, &Options::advisory());
    let v = r.to_json();
    assert_eq!(v["mode"], "advisory");
    assert_eq!(v["tiers"], json!(["profile", "prose", "input"]));
    assert_eq!(v["verdict"], "conforms");
    assert_eq!(v["vocabulary"], json!({"odrl": "common", "profiles": []}));
    let o = Options::strict()
        .structural(true)
        .odrl_vocabulary(OdrlVocab::Core)
        .level(RuleId::HeuristicOrderedOnUnordered, Level::Info);
    let v = validate_json(&text, &w, &o).to_json();
    assert_eq!(
        v["tiers"],
        json!(["profile", "odrl", "prose", "input", "heuristic"])
    );
    assert_eq!(v["vocabulary"]["odrl"], "core");
    // A profile without an IRI, title or version.
    let anon = prose_core::profile::ProfileVocab::parse(
        r#"{"@id": "https://example.org/a", "@type": "http://www.w3.org/ns/odrl/2/Action"}"#,
    )
    .unwrap();
    let v = validate_json(
        &text,
        &Wording::new().with_profile(anon),
        &Options::strict(),
    )
    .to_json();
    assert_eq!(
        v["vocabulary"]["profiles"],
        json!([{"iri": null, "title": null, "version": null}])
    );
}

#[test]
fn report_json_gaps_and_expected() {
    let r = validate_json(
        &strict_text("not-loaded.json"),
        &wording_of(&["research.jsonld"]),
        &Options::strict(),
    );
    let v = r.to_json();
    assert_eq!(v["verdict"], "incomplete");
    assert_eq!(
        v["gaps"],
        json!([{"rule": "profile.not-loaded", "detail": "https://example.org/odrl/profile/missing"}])
    );
    assert_eq!(
        v["findings"][0]["target"],
        json!({"kind": "node", "path": "policy[0]"})
    );
    let r = validate_json(
        &strict_text("wrong-kind.json"),
        &wording_of(&["research.jsonld"]),
        &Options::strict(),
    );
    assert_eq!(
        r.to_json()["findings"][1]["expected"],
        json!({"kinds": ["action"], "operators": [], "values": [], "units": [],
               "datatype": null, "closed": false})
    );
    // A finding on the document itself.
    let r = validate_json("not json", &Wording::new(), &Options::strict());
    let f = &r.to_json()["findings"][0];
    assert_eq!(f["target"], json!({"kind": "doc", "path": ""}));
    assert_eq!(f["basis"], "input");
    assert_eq!(f["term"], Value::Null);
    assert_eq!(
        f["fingerprint"],
        json!(fnv("input.unreadable", "", f["message"].as_str().unwrap()))
    );
}

fn golden_cases() -> Vec<(&'static str, String, Wording, Options)> {
    vec![
        (
            "violating-closed",
            strict_text("violating.json"),
            wording_of(&["research-closed.jsonld"]),
            Options::strict(),
        ),
        (
            "clean",
            strict_text("clean.json"),
            wording_of(&["research.jsonld"]),
            Options::strict(),
        ),
        (
            "not-loaded",
            strict_text("not-loaded.json"),
            wording_of(&["research.jsonld"]),
            Options::strict(),
        ),
        (
            "structural-bad",
            strict_text("structural-bad.json"),
            Wording::new(),
            Options::strict().structural(true),
        ),
        (
            "locked-opaque-fail-closed",
            strict_text("locked-opaque.json"),
            wording_of(&["research.jsonld"]),
            Options::strict().fail_closed(true),
        ),
        (
            "research-policy-advisory",
            profile_text("research-policy.json"),
            wording_of(&["research.jsonld"]),
            Options::advisory(),
        ),
    ]
}

#[test]
fn report_json_goldens() {
    for (name, text, w, o) in golden_cases() {
        let r = validate_json(&text, &w, &o);
        let got = serde_json::to_string_pretty(&masked(&r)).unwrap() + "\n";
        compare_golden(&format!("report/{name}.json"), &got);
    }
}

#[test]
fn deterministic_order() {
    for (name, text, w, o) in golden_cases() {
        let a = validate_json(&text, &w, &o).to_json();
        let b = validate(&read_model(&text).unwrap(), &w, &o).to_json();
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap(),
            "{name}"
        );
        // The same validator twice.
        let v = Validator::new(std::sync::Arc::new(w.clone()), o.clone());
        assert_eq!(v.validate_json(&text).to_json(), a, "{name}");
    }
}

/// `v` with every object's keys in reverse order.
fn reversed(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut out = serde_json::Map::new();
            for (k, x) in m.iter().rev() {
                out.insert(k.clone(), reversed(x));
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.iter().map(reversed).collect()),
        other => other.clone(),
    }
}

fn fingerprints(r: &Report) -> Vec<String> {
    r.to_json()["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["fingerprint"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn fingerprint_stable_across_reformatting_and_key_order() {
    let w = wording_of(&["research-closed.jsonld"]);
    let text = strict_text("violating.json");
    let v: Value = serde_json::from_str(&text).unwrap();
    let base = fingerprints(&validate_json(&text, &w, &Options::strict()));
    assert!(base.len() >= 4, "{base:?}");
    for other in [
        serde_json::to_string(&v).unwrap(),
        serde_json::to_string_pretty(&v).unwrap(),
        serde_json::to_string(&reversed(&v)).unwrap(),
    ] {
        assert_eq!(
            fingerprints(&validate_json(&other, &w, &Options::strict())),
            base
        );
    }
    for fp in &base {
        assert_eq!(fp.len(), 16);
        assert!(
            fp.bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
    }
}

#[test]
fn fingerprint_changes_with_term_and_place() {
    let text = |action: &str, n: usize| {
        let mut rules = vec![json!({"target": "urn:asset:1", "action": "use"}); n];
        rules.push(json!({"target": "urn:asset:1", "action": action}));
        json!({"@context": ["http://www.w3.org/ns/odrl.jsonld", {"foo": "https://example.net/foo/"}],
               "@type": "Set", "uid": "urn:policy:f", "permission": rules})
        .to_string()
    };
    let fp = |t: String| fingerprints(&validate_json(&t, &Wording::new(), &Options::strict()));
    let a = fp(text("foo:bar", 0));
    let b = fp(text("foo:baz", 0));
    let c = fp(text("foo:bar", 1));
    assert_eq!(a.len(), 1);
    assert_ne!(a, b);
    assert_ne!(a, c);
    assert_eq!(
        a[0],
        fnv(
            "profile.unknown-term",
            "policy[0].permission[0].action[0]#name",
            "foo:bar"
        )
    );
}

#[test]
fn fingerprints_are_unique_within_a_report() {
    let options = [
        Options::strict(),
        Options::strict().structural(true),
        Options::strict().structural(true).fail_closed(true),
    ];
    for (name, text) in policy_corpus() {
        for (label, w) in wordings() {
            for o in &options {
                let fps = fingerprints(&validate_json(&text, &w, o));
                let unique: HashSet<&String> = fps.iter().collect();
                assert_eq!(unique.len(), fps.len(), "{name} under {label}: {fps:?}");
            }
        }
    }
}

#[test]
fn validate_json_unreadable_never_err() {
    let deep = "[".repeat(100_000) + &"]".repeat(100_000);
    let long = format!("\"{}\"", "x".repeat(1_000_000));
    for text in [
        "",
        " ",
        "null",
        "{",
        "[1,2",
        "\u{feff}{}",
        "{\"@graph\": 3}",
        deep.as_str(),
        long.as_str(),
    ] {
        for o in [Options::advisory(), Options::strict().structural(true)] {
            let r = validate_json(text, &Wording::new(), &o);
            assert_eq!(r.verdict, Verdict::Violations, "{:.40}", text);
            assert_eq!(r.findings.len(), 1, "{:.40}", text);
            assert_eq!(r.findings[0].rule, RuleId::InputUnreadable);
            let _ = r.to_json();
        }
    }
}

#[test]
fn counts_and_verdict() {
    let r = validate_json(
        &strict_text("violating.json"),
        &wording_of(&["research-closed.jsonld"]),
        &Options::strict(),
    );
    let c = r.counts();
    let by = |s: Severity| r.findings.iter().filter(|f| f.severity == s).count();
    assert_eq!(
        (c.error, c.warning, c.info),
        (
            by(Severity::Error),
            by(Severity::Warning),
            by(Severity::Info)
        )
    );
    assert!(c.error > 0);
    let v = r.to_json();
    assert_eq!(v["counts"]["error"], json!(c.error));
    assert_eq!(v["verdict"], "violations");
    assert_eq!(c.total(), r.findings.len());
}
