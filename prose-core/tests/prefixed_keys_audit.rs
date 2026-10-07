//! What an independent audit of the prefixed-key reader found, and the
//! behaviours its mutation testing showed nothing pinned. Each test names the
//! finding it comes from.
use prose_core::{Document, render_value};
use serde_json::{Value, json};

const ODRL: &str = "http://www.w3.org/ns/odrl/2/";

fn render(v: Value) -> Document {
    render_value(&v).expect("a policy")
}

fn warned(doc: &Document, needle: &str) -> bool {
    doc.warnings.iter().any(|w| w.contains(needle))
}

fn sentence(doc: &Document) -> String {
    doc.policies
        .iter()
        .flat_map(|p| p.rules.iter().map(|r| r.sentence.clone()))
        .collect::<Vec<_>>()
        .join(" | ")
}

// --- finding 1: a twin is never silently dropped, on any object -----------

#[test]
fn a_twin_on_a_constraint_is_reported() {
    let doc = render(json!({
        "@type": "Set",
        "permission": [{
            "action": "use", "target": "urn:a",
            "constraint": [{
                "leftOperand": "purpose", "operator": "eq", "rightOperand": "x",
                "odrl:rightOperand": "y"
            }]
        }]
    }));
    assert!(warned(&doc, "\"odrl:rightOperand\""), "{:?}", doc.warnings);
}

#[test]
fn a_twin_on_an_action_refinement_is_reported() {
    // The permission would otherwise read broader than written.
    let doc = render(json!({
        "@type": "Set",
        "permission": [{
            "target": "urn:a",
            "action": {
                "rdf:value": { "@id": "odrl:print" },
                "refinement": [],
                "odrl:refinement": [{ "leftOperand": "count", "operator": "lteq", "rightOperand": 1 }]
            }
        }]
    }));
    assert!(warned(&doc, "\"odrl:refinement\""), "{:?}", doc.warnings);
}

#[test]
fn a_twin_on_a_party_is_reported() {
    let doc = render(json!({
        "@type": "Set",
        "permission": [{
            "action": "use", "target": "urn:a",
            "assignee": { "source": "urn:c1", "odrl:source": "urn:c2" }
        }]
    }));
    assert!(warned(&doc, "\"odrl:source\""), "{:?}", doc.warnings);
}

#[test]
fn the_twin_warning_says_the_bare_key_wins_instead_of_calling_it_unknown() {
    // "unknown" is wrong for a known term that lost a conflict.
    let doc = render(json!({
        "@type": "Set",
        "prohibition": [],
        "odrl:prohibition": [{ "action": "distribute", "target": "urn:a" }]
    }));
    let w = doc
        .warnings
        .iter()
        .find(|w| w.contains("\"odrl:prohibition\""))
        .unwrap_or_else(|| panic!("no warning names the twin: {:?}", doc.warnings));
    assert!(
        w.contains("\"prohibition\""),
        "names the key that wins: {w}"
    );
    assert!(!w.contains("unknown"), "not 'unknown': {w}");
}

#[test]
fn of_two_prefixed_spellings_the_first_in_document_order_is_read() {
    let first_odrl = render(json!({
        "@type": "Set",
        "permission": [{
            "odrl:action": "use",
            format!("{ODRL}action"): "print",
            "target": "urn:a"
        }]
    }));
    assert!(
        sentence(&first_odrl).contains("use urn:a"),
        "{}",
        sentence(&first_odrl)
    );
    assert!(!sentence(&first_odrl).contains("print"));
    assert!(
        warned(&first_odrl, &format!("\"{ODRL}action\"")),
        "{:?}",
        first_odrl.warnings
    );

    let first_iri = render(json!({
        "@type": "Set",
        "permission": [{
            format!("{ODRL}action"): "print",
            "odrl:action": "use",
            "target": "urn:a"
        }]
    }));
    assert!(
        sentence(&first_iri).contains("print urn:a"),
        "{}",
        sentence(&first_iri)
    );
    assert!(
        warned(&first_iri, "\"odrl:action\""),
        "{:?}",
        first_iri.warnings
    );
}

// --- finding 5: literal values are the author's, not ours to rewrite ------

#[test]
fn a_literal_value_object_keeps_its_keys() {
    let doc = render(json!({
        "@type": "Set",
        "permission": [{
            "action": "use", "target": "urn:a",
            "constraint": [{
                "leftOperand": "purpose", "operator": "eq",
                "rightOperand": {
                    "@value": { "odrl:target": "a", "http://www.w3.org/ns/odrl/2/action": "b" },
                    "@type": "@json"
                }
            }]
        }]
    }));
    let text = &doc.policies[0].rules[0].conditions[0].text;
    assert!(text.contains("odrl:target"), "{text}");
    assert!(
        text.contains("http://www.w3.org/ns/odrl/2/action"),
        "{text}"
    );
}

// --- finding 3: hostile depth never takes the page down -------------------

/// `render_value` takes a value that is already parsed, so the parser's own
/// depth limit does not protect it. Run on a small stack, like wasm's.
fn on_a_small_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .expect("the render overflowed its stack or panicked");
}

/// `depth` levels of `{"x": ...}`, built by direct insertion: `json!` would
/// serialise the inner value again at every level, recursing as deep as the
/// value is.
fn nested(depth: usize) -> Value {
    let mut v = Value::String("leaf".into());
    for _ in 0..depth {
        let mut m = serde_json::Map::new();
        m.insert("x".into(), v);
        v = Value::Object(m);
    }
    v
}

/// A value this deep cannot even be dropped on a small stack (serde_json's
/// `Drop` recurses), so the tests leak it: the thread ends anyway. What is
/// under test is the reader, not serde_json's destructor.
fn read_leaking(v: Value) -> Document {
    let doc = render_value(&v).expect("a policy");
    std::mem::forget(v);
    doc
}

#[test]
fn a_deep_extension_payload_is_not_walked() {
    on_a_small_stack(|| {
        let mut v = json!({
            "@type": "Set",
            "odrl:permission": [{ "odrl:action": "use", "odrl:target": "urn:a" }]
        });
        v["odrl:permission"][0]["ext:blob"] = nested(3_000);
        let doc = read_leaking(v);
        assert!(sentence(&doc).contains("use urn:a"), "{}", sentence(&doc));
        assert!(warned(&doc, "deeper than"), "{:?}", doc.warnings);
    });
}

#[test]
fn a_deep_context_is_not_walked() {
    on_a_small_stack(|| {
        let mut v = json!({
            "@context": { "odrl": ODRL },
            "@type": "Set",
            "odrl:permission": [{ "odrl:action": "use", "odrl:target": "urn:a" }]
        });
        v["@context"]["ex:deep"] = nested(3_000);
        let doc = read_leaking(v);
        assert!(sentence(&doc).contains("use urn:a"), "{}", sentence(&doc));
        assert!(warned(&doc, "deeper than"), "{:?}", doc.warnings);
    });
}

// --- finding 6: the @context guard --------------------------------------

#[test]
fn a_type_scoped_context_that_rebinds_odrl_is_not_odrl() {
    let doc = render(json!({
        "@context": {
            "MyPolicy": {
                "@id": "https://example.org/MyPolicy",
                "@context": { "odrl": "https://evil.example/" }
            }
        },
        "@type": ["Set", "MyPolicy"],
        "odrl:prohibition": [{ "odrl:action": "use", "odrl:target": "urn:a" }]
    }));
    assert!(doc.policies[0].rules.is_empty(), "{}", sentence(&doc));
    assert!(warned(&doc, "\"odrl:prohibition\""), "{:?}", doc.warnings);
}

// --- what mutation testing showed nothing pinned -----------------------

fn prefixed_prohibition(context: Value) -> Document {
    render(json!({
        "@context": context,
        "@type": "Set",
        "odrl:prohibition": [{ "odrl:action": "use", "odrl:target": "urn:a" }]
    }))
}

fn reads_as_odrl(doc: &Document) -> bool {
    sentence(doc).contains("No one may use urn:a")
}

#[test]
fn only_the_properties_the_reader_knows_are_renamed() {
    let doc = render(json!({
        "@type": "Set",
        "permission": [{
            "odrl:action": "use", "odrl:target": "urn:a",
            "odrl:madeUp": "x"
        }]
    }));
    assert!(sentence(&doc).contains("use urn:a"), "{}", sentence(&doc));
    assert!(warned(&doc, "\"odrl:madeUp\""), "{:?}", doc.warnings);
}

#[test]
fn an_expanded_term_definition_is_judged_by_its_iri() {
    let right = prefixed_prohibition(json!({ "odrl": { "@id": ODRL } }));
    assert!(reads_as_odrl(&right), "{:?}", right.warnings);
    let wrong = prefixed_prohibition(json!({ "odrl": { "@id": "https://evil.example/" } }));
    assert!(!reads_as_odrl(&wrong));
    assert!(warned(&wrong, "\"odrl:prohibition\""));
}

#[test]
fn an_array_context_is_judged_member_by_member() {
    let right = prefixed_prohibition(json!(["http://www.w3.org/ns/odrl.jsonld", { "odrl": ODRL }]));
    assert!(reads_as_odrl(&right), "{:?}", right.warnings);
    let wrong = prefixed_prohibition(
        json!(["http://www.w3.org/ns/odrl.jsonld", { "odrl": "https://evil.example/" }]),
    );
    assert!(!reads_as_odrl(&wrong));
}

#[test]
fn a_rebinding_anywhere_in_the_document_refuses_the_prefix_everywhere() {
    // Documented, conservative: there is no JSON-LD processing to say where a
    // nested context applies, so it is reported, not guessed at.
    let doc = render(json!({
        "@context": { "odrl": ODRL },
        "@type": "Set",
        "odrl:prohibition": [{
            "odrl:action": "use", "odrl:target": "urn:a",
            "@context": { "odrl": "https://evil.example/" }
        }]
    }));
    assert!(!reads_as_odrl(&doc), "{}", sentence(&doc));
    assert!(warned(&doc, "\"odrl:prohibition\""), "{:?}", doc.warnings);
}

#[test]
fn full_iri_keys_are_read_whatever_the_context_says() {
    let doc = render(json!({
        "@context": { "odrl": "https://evil.example/" },
        "@type": "Set",
        format!("{ODRL}prohibition"): [{ format!("{ODRL}action"): "use", format!("{ODRL}target"): "urn:a" }]
    }));
    assert!(reads_as_odrl(&doc), "{:?}", doc.warnings);
}

#[test]
fn prefixed_keys_are_read_inside_a_graph() {
    let doc = render(json!({
        "@context": { "odrl": ODRL },
        "@graph": [
            { "@type": "Set", "odrl:prohibition": [{ "odrl:action": "use", "odrl:target": "urn:a" }] },
            { "@type": "Set", "odrl:permission": [{ "odrl:action": "print", "odrl:target": "urn:b" }] }
        ]
    }));
    assert_eq!(doc.policies.len(), 2);
    assert!(
        sentence(&doc).contains("No one may use urn:a"),
        "{}",
        sentence(&doc)
    );
    assert!(sentence(&doc).contains("print urn:b"), "{}", sentence(&doc));
}
