//! Strict validation never certifies what the editor reader did not read.
//! The reader takes bare ODRL property keys only, so a property written as a
//! compact IRI, a full IRI or an @context alias is skipped; each such key is
//! an `input.unchecked` gap (an error under `fail_closed`), never a pass.
use prose_core::edit::{Severity, read_model, write_jsonld};
use prose_core::profile::Wording;
use prose_core::validate::*;
use serde_json::{Value, json};

const ODRL: &str = "http://www.w3.org/ns/odrl/2/";

fn run(v: &Value, o: &Options) -> Report {
    validate_json(&v.to_string(), &Wording::new(), o)
}

fn unchecked(r: &Report) -> Vec<&Finding> {
    r.findings
        .iter()
        .filter(|f| f.rule.to_string() == "input.unchecked")
        .collect()
}

fn prefixed() -> Value {
    json!({
        "@context": {"odrl": ODRL},
        "@type": "odrl:Set",
        "@id": "urn:p",
        "odrl:permission": [{
            "odrl:target": "urn:a",
            "odrl:action": {"@id": "odrl:notAnAction"}
        }]
    })
}

fn full_iri() -> Value {
    json!({
        "@context": {"odrl": ODRL},
        "@type": "odrl:Set",
        "@id": "urn:p",
        format!("{ODRL}permission"): [{"target": "urn:a", "action": "use"}]
    })
}

fn alias() -> Value {
    json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"when": {"@id": "odrl:constraint"}}],
        "@type": "Set",
        "uid": "urn:p",
        "permission": [{
            "target": "urn:a", "action": "use",
            "when": {"leftOperand": "notAnOdrlTerm", "operator": "eq", "rightOperand": "x"}
        }]
    })
}

fn prefix_alias() -> Value {
    json!({
        "@context": {"o": ODRL},
        "@type": "Set",
        "uid": "urn:p",
        "o:permission": [{"target": "urn:a", "action": "use"}]
    })
}

fn nested_prefixed() -> Value {
    json!({
        "@context": {"odrl": ODRL},
        "@type": "Set",
        "uid": "urn:p",
        "permission": [{
            "target": "urn:a", "action": "use",
            "constraint": [{
                "odrl:leftOperand": "notAnOdrlTerm", "odrl:operator": "eq", "odrl:rightOperand": "x"
            }]
        }]
    })
}

#[test]
fn skipped_odrl_keys_are_a_gap_not_a_pass() {
    for (name, v) in [
        ("prefixed", prefixed()),
        ("full iri", full_iri()),
        ("alias", alias()),
        ("prefix alias", prefix_alias()),
        ("nested prefixed", nested_prefixed()),
    ] {
        let r = run(&v, &Options::strict());
        assert_eq!(r.verdict, Verdict::Incomplete, "{name}: {:?}", r.findings);
        let u = unchecked(&r);
        assert!(!u.is_empty(), "{name}");
        assert!(
            u.iter().all(|f| f.gap && f.severity == Severity::Warning),
            "{name}"
        );
    }
}

#[test]
fn skipped_odrl_keys_fail_closed_when_asked() {
    for (name, v) in [
        ("prefixed", prefixed()),
        ("full iri", full_iri()),
        ("alias", alias()),
        ("prefix alias", prefix_alias()),
        ("nested prefixed", nested_prefixed()),
    ] {
        let r = run(&v, &Options::strict().fail_closed(true));
        assert_eq!(r.verdict, Verdict::Violations, "{name}");
        let u = unchecked(&r);
        assert!(!u.is_empty(), "{name}");
        assert!(u.iter().all(|f| f.severity == Severity::Error), "{name}");
    }
}

#[test]
fn the_message_names_the_skipped_key() {
    let r = run(&prefixed(), &Options::strict());
    let msgs: Vec<&str> = unchecked(&r).iter().map(|f| f.message.as_str()).collect();
    assert!(
        msgs.iter().any(|m| m.contains("odrl:permission")),
        "{msgs:?}"
    );
    let r = run(&alias(), &Options::strict());
    let msgs: Vec<&str> = unchecked(&r).iter().map(|f| f.message.as_str()).collect();
    assert!(msgs.iter().any(|m| m.contains("\"when\"")), "{msgs:?}");
}

#[test]
fn bare_keys_are_unchanged() {
    let bare = json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Set",
        "uid": "urn:p",
        "permission": [{"target": "urn:a", "action": "notAnAction"}]
    });
    let r = run(&bare, &Options::strict());
    assert!(unchecked(&r).is_empty(), "{:?}", r.findings);
    let clean = json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Set",
        "uid": "urn:p",
        "permission": [{"target": "urn:a", "action": "use"}]
    });
    let r = run(&clean, &Options::strict().fail_closed(true));
    assert_eq!(r.verdict, Verdict::Conforms, "{:?}", r.findings);
}

#[test]
fn advisory_says_nothing_new() {
    for v in [prefixed(), alias(), nested_prefixed()] {
        let r = run(&v, &Options::default());
        assert!(unchecked(&r).is_empty(), "{:?}", r.findings);
    }
}

#[test]
fn reading_stays_lossless() {
    for v in [
        prefixed(),
        full_iri(),
        alias(),
        prefix_alias(),
        nested_prefixed(),
    ] {
        let doc = read_model(&v.to_string()).unwrap();
        assert_eq!(write_jsonld(&doc), v);
    }
}

#[test]
fn an_inline_context_that_spells_odrl_terms_by_their_own_names_is_read() {
    // The ODRL context's own shape, inline: every term is its own bare key.
    let v = json!({
        "@context": {
            "odrl": ODRL,
            "uid": "@id",
            "Set": "odrl:Set",
            "permission": {"@id": "odrl:permission", "@type": "@id"},
            "target": {"@id": "odrl:target", "@type": "@id"},
            "action": {"@id": "odrl:action", "@type": "@vocab"},
            "use": "odrl:use"
        },
        "@type": "Set",
        "uid": "urn:p",
        "permission": [{"target": "urn:a", "action": "use"}]
    });
    let r = run(&v, &Options::strict().fail_closed(true));
    assert_eq!(r.verdict, Verdict::Conforms, "{:?}", r.findings);
}

#[test]
fn an_alias_defined_but_not_used_is_not_reported() {
    let v = json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"when": {"@id": "odrl:constraint"}}],
        "@type": "Set",
        "uid": "urn:p",
        "permission": [{"target": "urn:a", "action": "use"}]
    });
    let r = run(&v, &Options::strict().fail_closed(true));
    assert_eq!(r.verdict, Verdict::Conforms, "{:?}", r.findings);
}

#[test]
fn an_alias_in_a_scoped_context_is_reported() {
    let v = json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {
            "permission": {"@id": "odrl:permission", "@context": {"when": "odrl:constraint"}}
        }],
        "@type": "Set",
        "uid": "urn:p",
        "permission": [{
            "target": "urn:a", "action": "use",
            "when": {"leftOperand": "notAnOdrlTerm", "operator": "eq", "rightOperand": "x"}
        }]
    });
    let r = run(&v, &Options::strict());
    assert_eq!(r.verdict, Verdict::Incomplete, "{:?}", r.findings);
    assert!(unchecked(&r).iter().any(|f| f.message.contains("\"when\"")));
}

#[test]
fn a_literal_value_is_data_not_keys() {
    let v = json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"odrl": ODRL}],
        "@type": "Set",
        "uid": "urn:p",
        "dc:description": {"@value": {"odrl:permission": []}, "@type": "@json"},
        "permission": [{"target": "urn:a", "action": "use"}]
    });
    let r = run(&v, &Options::strict().fail_closed(true));
    assert_eq!(r.verdict, Verdict::Conforms, "{:?}", r.findings);
}

#[test]
fn each_skipped_key_is_reported_once() {
    let v = json!({
        "@context": {"odrl": ODRL},
        "@type": "Set",
        "uid": "urn:p",
        "permission": [
            {"target": "urn:a", "action": "use", "odrl:duty": []},
            {"target": "urn:b", "action": "use", "odrl:duty": []}
        ]
    });
    let r = run(&v, &Options::strict());
    let u = unchecked(&r);
    assert_eq!(u.len(), 1, "{:?}", r.findings);
    assert_eq!(u[0].term.as_ref().unwrap().written, "odrl:duty");
}

#[test]
fn many_skipped_keys_are_named_up_to_a_cap_then_counted() {
    let mut rule = serde_json::Map::new();
    rule.insert("target".into(), json!("urn:a"));
    rule.insert("action".into(), json!("use"));
    for i in 0..25 {
        rule.insert(format!("odrl:k{i}"), json!("x"));
    }
    let v = json!({
        "@context": {"odrl": ODRL},
        "@type": "Set",
        "uid": "urn:p",
        "permission": [rule]
    });
    let r = run(&v, &Options::strict());
    let u = unchecked(&r);
    assert_eq!(u.len(), 21, "{:?}", r.findings);
    assert_eq!(u.iter().filter(|f| f.term.is_some()).count(), 20);
    assert!(u.iter().any(|f| f.message.starts_with("5 more")), "{u:?}");
}
