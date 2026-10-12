//! The time budget of strict validation, measured in a release build:
//!
//! ```text
//! cargo test -p prose-core --release --test validate_perf -- --ignored
//! ```
//!
//! A 10 000-rule policy against a 20 000-term profile is validated in about
//! 170 ms on a developer machine, and the validator's index is built in about
//! 1 ms. The asserted limits (500 ms and 200 ms) are loose on purpose: they
//! catch a quadratic regression, not a noisy shared CI runner, where the same
//! commit measured 206 ms once. A debug build is several times slower, so the
//! test is ignored by default.
use prose_core::profile::{ProfileVocab, Wording};
use prose_core::validate::{Options, Validator, Verdict};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};

const EX: &str = "https://example.org/odrl/profile/big/";
const BIG: &str = "https://example.org/odrl/profile/big";

/// 10 000 actions and 10 000 left operands; every tenth operand is closed
/// with operator, value and datatype hints.
fn big_profile() -> ProfileVocab {
    let mut graph: Vec<Value> = vec![json!({"@id": BIG, "@type": "owl:Ontology"})];
    for i in 0..10_000 {
        graph.push(json!({"@id": format!("ex:a{i}"), "@type": "odrl:Action",
                          "odrl:includedIn": {"@id": "odrl:use"}}));
        let mut l = json!({"@id": format!("ex:l{i}"), "@type": "odrl:LeftOperand"});
        if i % 10 == 0 {
            l["prose:operator"] = json!([{"@id": "odrl:eq"}, {"@id": "odrl:isAnyOf"}]);
            l["prose:datatype"] = json!({"@id": "xsd:integer"});
            l["prose:closed"] = json!(true);
        }
        graph.push(l);
    }
    let text = json!({
        "@context": {"odrl": "http://www.w3.org/ns/odrl/2/",
                     "owl": "http://www.w3.org/2002/07/owl#",
                     "xsd": "http://www.w3.org/2001/XMLSchema#",
                     "prose": "https://ds-labs-org.github.io/ds-prose-rs/profile#",
                     "ex": EX},
        "@graph": graph
    })
    .to_string();
    let v = ProfileVocab::parse(&text).unwrap();
    assert_eq!(v.terms().count(), 20_000);
    v
}

fn big_policy() -> String {
    let rules: Vec<Value> = (0..10_000)
        .map(|i| {
            json!({"action": format!("ex:a{}", (i * 7) % 10_000),
                   "constraint": [{"leftOperand": format!("ex:l{i}"), "operator": "eq",
                                   "rightOperand": {"@value": format!("{i}"), "@type": "xsd:integer"}},
                                  {"leftOperand": "dateTime", "operator": "lt",
                                   "rightOperand": {"@value": "2030-01-01T00:00:00Z", "@type": "xsd:dateTime"}}]})
        })
        .collect();
    json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"ex": EX}],
        "@type": "Agreement", "uid": "urn:policy:big", "profile": BIG,
        "assigner": "urn:party:a", "assignee": "urn:party:b", "target": "urn:asset:1",
        "permission": rules
    })
    .to_string()
}

/// The fastest of a few runs, so one slow scheduling slot does not fail
/// the budget.
fn best<T>(runs: usize, mut f: impl FnMut() -> T) -> (Duration, T) {
    let mut out = None;
    let mut fastest = Duration::MAX;
    for _ in 0..runs {
        let t = Instant::now();
        let v = f();
        fastest = fastest.min(t.elapsed());
        out = Some(v);
    }
    (fastest, out.unwrap())
}

#[test]
#[ignore = "a time budget: run with --release -- --ignored"]
fn strict_validation_within_budget() {
    let wording = Arc::new(Wording::new().with_profile(big_profile()));
    let policy = big_policy();
    let options = Options::strict().structural(true);
    let (built, v) = best(3, || Validator::new(wording.clone(), options.clone()));
    let (checked, report) = best(3, || v.validate_json(&policy));
    assert_eq!(
        report.verdict,
        Verdict::Conforms,
        "{:?}",
        &report.findings[..5.min(report.findings.len())]
    );
    eprintln!("index {built:?}, validate_json {checked:?}");
    assert!(built < Duration::from_millis(200), "index took {built:?}");
    assert!(
        checked < Duration::from_millis(500),
        "validate_json took {checked:?}"
    );
}
