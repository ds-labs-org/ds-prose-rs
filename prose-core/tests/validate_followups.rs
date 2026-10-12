//! Follow-ups from the strict-validation audit: the size and cost of a
//! report for a hostile policy, fingerprints that tell findings apart, and
//! a constraint with several logical operands.
mod common;
mod corpus;
use corpus::*;
use prose_core::edit::Severity;
use prose_core::profile::{ProfileVocab, Wording};
use prose_core::validate::*;
use serde_json::{Value, json};
use std::sync::mpsc;
use std::time::Duration;

const EX: &str = "https://example.org/odrl/profile/big/";

/// Runs `f` on its own thread and fails if it takes longer than the bound.
fn within<T: Send + 'static>(what: &str, secs: u64, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(Duration::from_secs(secs)) {
        Ok(v) => v,
        Err(_) => panic!("{what}: still running after {secs} s"),
    }
}

/// A profile with one left operand that allows `allowed` values.
fn value_profile(allowed: usize, closed: bool) -> Wording {
    let values: Vec<Value> = (0..allowed)
        .map(|i| json!({"@id": format!("{EX}allowed-value-number-{i}")}))
        .collect();
    let profile = json!({
        "@context": {
            "odrl": "http://www.w3.org/ns/odrl/2/",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "owl": "http://www.w3.org/2002/07/owl#",
            "prose": "https://ds-labs-org.github.io/ds-prose-rs/profile#"
        },
        "@graph": [
            {"@id": "https://example.org/odrl/profile/big", "@type": "owl:Ontology"},
            {"@id": format!("{EX}lo"), "@type": "odrl:LeftOperand",
             "rdfs:isDefinedBy": {"@id": "https://example.org/odrl/profile/big"},
             "prose:rightOperand": values, "prose:closed": closed}
        ]
    });
    Wording::new().with_profile(ProfileVocab::parse(&profile.to_string()).unwrap())
}

fn value_policy(bad: usize) -> String {
    let values: Vec<Value> = (0..bad).map(|_| json!({"@id": "ex:b"})).collect();
    json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"ex": EX}],
        "@type": "Set", "uid": "urn:policy:v", "profile": "https://example.org/odrl/profile/big",
        "permission": [{"target": "urn:asset:1", "action": "use",
            "constraint": [{"leftOperand": "ex:lo", "operator": "isAnyOf",
                            "rightOperand": values}]}]
    })
    .to_string()
}

#[test]
fn expected_values_are_capped_per_finding() {
    for closed in [true, false] {
        let w = value_profile(256, closed);
        let r = validate_json(&value_policy(200), &w, &Options::strict());
        assert!(!r.findings.is_empty());
        let rules: Vec<_> = r.findings.iter().map(|f| f.rule.to_string()).collect();
        assert!(
            rules.iter().any(|r| r == "prose.value-not-allowed"),
            "{rules:?}"
        );
        let text = r.to_json().to_string();
        // Linear in the findings: a few hundred bytes each, not 256 IRIs.
        assert!(
            text.len() < r.findings.len() * 1500,
            "closed={closed}: {} bytes for {} findings",
            text.len(),
            r.findings.len()
        );
        for f in &r.findings {
            if let Some(e) = &f.expected {
                assert!(e.values.len() <= 8, "{} values", e.values.len());
            }
        }
        let j = r.to_json();
        let f = j["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["rule"] == "prose.value-not-allowed")
            .unwrap();
        assert_eq!(f["expected"]["values"].as_array().unwrap().len(), 8);
        assert_eq!(f["expected"]["valuesTotal"], 256);
    }
}

#[test]
fn expected_values_within_the_cap_are_listed_whole() {
    let w = value_profile(5, true);
    let r = validate_json(&value_policy(1), &w, &Options::strict());
    let j = r.to_json();
    let f = j["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["rule"] == "prose.value-not-allowed")
        .unwrap();
    assert_eq!(f["expected"]["values"].as_array().unwrap().len(), 5);
    assert!(f["expected"].get("valuesTotal").is_none());
}

#[test]
fn many_declared_profiles_cost_linear_time() {
    let n = 40_000;
    let other = profile_text("other.jsonld");
    let copy = other.replace("profile/other", "profile/othe2");
    let w = Wording::new()
        .with_profile(profile("research-closed.jsonld"))
        .with_profile(ProfileVocab::parse(&copy).unwrap())
        .with_profile(profile("other.jsonld"));
    let profiles = vec!["https://example.com/odrl/profile/othe2"; n];
    let rules: Vec<_> = (0..n)
        .map(|_| json!({"target": "urn:a", "action": "https://example.com/odrl/profile/other/archiveCopy"}))
        .collect();
    let policy = json!({"@context": "http://www.w3.org/ns/odrl.jsonld", "@type": "Set",
        "uid": "urn:p", "profile": profiles, "permission": rules})
    .to_string();
    let r = within("declared profiles", 6, move || {
        validate_json(&policy, &w, &Options::strict())
    });
    assert!(r.findings.len() >= n);
}

#[test]
fn fingerprints_tell_importing_profiles_apart() {
    let imp = |me: &str| {
        json!({
            "@context": {"owl": "http://www.w3.org/2002/07/owl#",
                         "odrl": "http://www.w3.org/ns/odrl/2/"},
            "@graph": [{"@id": me, "@type": "owl:Ontology",
                        "owl:imports": {"@id": "https://example.org/base"}},
                       {"@id": format!("{me}/act"), "@type": "odrl:Action"}]
        })
        .to_string()
    };
    let w = Wording::new()
        .with_profile(ProfileVocab::parse(&imp("https://example.org/a")).unwrap())
        .with_profile(ProfileVocab::parse(&imp("https://example.org/b")).unwrap());
    let policy = json!({"@context": "http://www.w3.org/ns/odrl.jsonld", "@type": "Set",
        "uid": "urn:p", "permission": [{"target": "urn:a", "action": "use"}]})
    .to_string();
    let r = validate_json(&policy, &w, &Options::strict());
    let imports: Vec<&Finding> = r
        .findings
        .iter()
        .filter(|f| f.rule.to_string() == "profile.import-not-loaded")
        .collect();
    assert_eq!(imports.len(), 2, "{:?}", r.findings);
    assert_ne!(imports[0].fingerprint(), imports[1].fingerprint());
    // The report lists each gap once.
    let j = r.to_json();
    let gaps = j["gaps"].as_array().unwrap();
    let mut seen: Vec<String> = gaps.iter().map(|g| g.to_string()).collect();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), gaps.len(), "{gaps:?}");
}

#[test]
fn fingerprints_are_unique_over_the_corpus() {
    for (name, text) in policy_corpus() {
        for (wn, w) in wordings() {
            let r = validate_json(&text, &w, &Options::strict().structural(true));
            let mut seen = std::collections::HashMap::new();
            for f in &r.findings {
                if let Some(prev) = seen.insert(f.fingerprint(), f.message.clone()) {
                    // The same finding made twice is one finding; two
                    // different messages under one fingerprint are not.
                    assert_eq!(prev, f.message, "{name} / {wn}: {}", f.fingerprint());
                }
            }
        }
    }
}

#[test]
fn several_logical_operands_violate_the_information_model() {
    let text = json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld", "@type": "Set", "uid": "urn:p",
        "permission": [{"target": "urn:a", "action": "use", "constraint": [{
            "and": [{"leftOperand": "count", "operator": "lt", "rightOperand": 3}],
            "or": [{"leftOperand": "count", "operator": "gt", "rightOperand": 1}]
        }]}]
    })
    .to_string();
    let r = validate_json(&text, &Wording::new(), &Options::strict().structural(true));
    let shape: Vec<&Finding> = r
        .findings
        .iter()
        .filter(|f| f.rule == RuleId::OdrlConstraintShape)
        .collect();
    assert_eq!(shape.len(), 1, "{:?}", r.findings);
    assert_eq!(
        target_str(&shape[0].target),
        "policy[0].permission[0].constraint[0]"
    );
    assert_eq!(shape[0].severity, Severity::Error);
    assert_eq!(
        shape[0].message,
        "this logical constraint has several operands (and, or); it must have exactly one of or, xone, and, andSequence"
    );
    assert_eq!(r.verdict, Verdict::Violations);
    // Without the structural tier the constraint is still reported unchecked.
    let r = validate_json(&text, &Wording::new(), &Options::strict());
    assert!(r.findings.iter().any(|f| f.rule == RuleId::InputUnchecked));
    assert!(
        r.findings
            .iter()
            .all(|f| f.rule != RuleId::OdrlConstraintShape)
    );
}
