//! ODRL property keys written as compact IRIs (`odrl:permission`) or full
//! IRIs (`http://www.w3.org/ns/odrl/2/permission`) read like the bare terms.
//! That is how a JSON-LD processor compacts ODRL against a context that
//! declares the `odrl` prefix but not the terms, as EDC catalogs do.
use prose_core::render_value;
use serde_json::{Map, Value, json};

const ODRL: &str = "http://www.w3.org/ns/odrl/2/";

/// The ODRL properties the fixtures use.
const TERMS: &[&str] = &[
    "uid",
    "profile",
    "conflict",
    "inheritFrom",
    "assigner",
    "assignee",
    "target",
    "action",
    "permission",
    "prohibition",
    "obligation",
    "constraint",
    "refinement",
    "duty",
    "consequence",
    "remedy",
    "leftOperand",
    "operator",
    "rightOperand",
    "rightOperandReference",
    "unit",
    "and",
    "or",
    "xone",
    "andSequence",
    "source",
    "partOf",
];

fn fixture(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Every ODRL property key in `v`, renamed by `rename`.
fn rekey(v: &Value, rename: &dyn Fn(&str) -> String) -> Value {
    match v {
        Value::Array(a) => Value::Array(a.iter().map(|x| rekey(x, rename)).collect()),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| {
                    let k = if TERMS.contains(&k.as_str()) {
                        rename(k)
                    } else {
                        k.clone()
                    };
                    (k, rekey(x, rename))
                })
                .collect::<Map<_, _>>(),
        ),
        _ => v.clone(),
    }
}

#[test]
fn compact_and_full_iri_keys_read_like_bare_terms() {
    for name in [
        "offer",
        "agreement",
        "set",
        "inheritance",
        "entity-collection",
        "operand-reference",
        "policy-action",
        "rule-reference",
    ] {
        let bare = render_value(&fixture(name)).unwrap();
        let compact = rekey(&fixture(name), &|k| format!("odrl:{k}"));
        let full = rekey(&fixture(name), &|k| format!("{ODRL}{k}"));
        assert_eq!(
            render_value(&compact).unwrap(),
            bare,
            "{name} with odrl: keys"
        );
        assert_eq!(
            render_value(&full).unwrap(),
            bare,
            "{name} with full IRI keys"
        );
    }
}

/// The shape an EDC management API returns once compacted with its context.
#[test]
fn edc_compacted_offer_reads_its_rules() {
    let offer = json!({
        "@id": "offer-1",
        "@type": "odrl:Offer",
        "odrl:permission": [{
            "odrl:action": { "@id": "odrl:use" },
            "odrl:constraint": {
                "odrl:leftOperand": { "@id": "eox-policy:Membership" },
                "odrl:operator": { "@id": "odrl:eq" },
                "odrl:rightOperand": "active"
            }
        }],
        "odrl:prohibition": [],
        "odrl:obligation": [{ "odrl:action": { "@id": "odrl:reviewPolicy" } }],
        "target": "urn:asset:1",
        "assigner": "did:web:provider"
    });
    let doc = render_value(&offer).unwrap();
    assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
    let rules = &doc.policies[0].rules;
    assert_eq!(rules.len(), 2);
    assert_eq!(
        rules[0].sentence,
        "Anyone may use urn:asset:1, as permitted by did:web:provider."
    );
    assert_eq!(
        rules[0].conditions[0].text,
        "the membership is equal to \"active\""
    );
    assert!(
        rules[1]
            .sentence
            .starts_with("The assignee must review policy")
    );
}

/// A bare key and its prefixed twin on one object: the bare one is read, the
/// other is reported, never merged or silently dropped.
#[test]
fn bare_key_wins_and_its_prefixed_twin_is_reported() {
    let doc = render_value(&json!({
        "@type": "Set",
        "permission": [{ "action": "use", "target": "urn:a" }],
        "odrl:permission": [{ "action": "print", "target": "urn:a" }]
    }))
    .unwrap();
    let rules = &doc.policies[0].rules;
    assert_eq!(rules.len(), 1);
    assert!(
        rules[0].sentence.contains("use urn:a"),
        "{}",
        rules[0].sentence
    );
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.contains("\"odrl:permission\"")),
        "{:?}",
        doc.warnings
    );
}

/// No JSON-LD processing: when the document's own context binds `odrl` to
/// something else, `odrl:` keys are not ODRL and are reported as unknown.
#[test]
fn odrl_prefix_bound_elsewhere_is_not_odrl() {
    let doc = render_value(&json!({
        "@context": { "odrl": "https://example.com/not-odrl#" },
        "@type": "Set",
        "permission": [{ "action": "use", "target": "urn:a" }],
        "odrl:prohibition": [{ "action": "print", "target": "urn:a" }]
    }))
    .unwrap();
    assert_eq!(doc.policies[0].rules.len(), 1);
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.contains("\"odrl:prohibition\"")),
        "{:?}",
        doc.warnings
    );
}

/// Other prefixes are left alone, ODRL-looking local name or not.
#[test]
fn other_prefixes_are_not_read_as_odrl() {
    let doc = render_value(&json!({
        "@type": "Set",
        "permission": [{ "action": "use", "target": "urn:a" }],
        "ex:prohibition": [{ "action": "print", "target": "urn:a" }]
    }))
    .unwrap();
    assert_eq!(doc.policies[0].rules.len(), 1);
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.contains("\"ex:prohibition\""))
    );
}
