//! Text that is not a profile, or a profile written to hurt: each is refused
//! with a specific error or read within bounds, never with a panic.
use prose_core::profile::{ProfileError, ProfileLimits, ProfileVocab};
use serde_json::{Map, Value, json};

fn profile_text(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/profiles/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn err(text: &str) -> ProfileError {
    ProfileVocab::parse(text).expect_err("expected an error")
}

fn limits(f: impl FnOnce(&mut ProfileLimits)) -> ProfileLimits {
    let mut l = ProfileLimits::default();
    f(&mut l);
    l
}

fn en() -> Vec<String> {
    vec!["en".to_string()]
}

#[test]
fn too_large_before_parse() {
    // Not JSON at all: the size is refused before any parse is tried.
    let text = "x".repeat(200);
    let l = limits(|l| l.max_bytes = 100);
    assert_eq!(
        ProfileVocab::parse_with(&text, &l),
        Err(ProfileError::TooLarge {
            bytes: 200,
            limit: 100
        })
    );
    // A byte-order mark does not count.
    let text = format!("\u{FEFF}{}", profile_text("research.jsonld"));
    let l = limits(|l| l.max_bytes = text.len() - 3);
    assert!(ProfileVocab::parse_with(&text, &l).is_ok());
}

#[test]
fn context_only() {
    assert_eq!(
        err(&profile_text("hostile/context-only.jsonld")),
        ProfileError::ContextOnly
    );
}

#[test]
fn json_schema() {
    assert_eq!(
        err(&profile_text("hostile/json-schema.json")),
        ProfileError::JsonSchema
    );
}

#[test]
fn html() {
    assert_eq!(
        err(&profile_text("hostile/landing.html")),
        ProfileError::LooksLikeHtml
    );
    assert_eq!(
        err("\u{FEFF}  \n<html><body>x</body></html>"),
        ProfileError::LooksLikeHtml
    );
    assert_eq!(err("<!-- a comment -->"), ProfileError::LooksLikeHtml);
}

#[test]
fn policy_given_as_profile() {
    assert_eq!(
        err(&profile_text("research-policy.json")),
        ProfileError::LooksLikePolicy
    );
    assert_eq!(
        err(r#"{"@type": "Set", "odrl:permission": []}"#),
        ProfileError::LooksLikePolicy
    );
}

#[test]
fn empty_graph_no_terms() {
    assert_eq!(
        err(r#"{"@context": {"odrl": "http://www.w3.org/ns/odrl/2/"}, "@graph": []}"#),
        ProfileError::NoTerms
    );
    assert_eq!(err("[]"), ProfileError::NoTerms);
    assert!(matches!(err("{\"a\": "), ProfileError::Json(_)));
    assert!(matches!(err(""), ProfileError::Json(_)));
}

#[cfg(not(feature = "turtle"))]
#[test]
fn turtle_text_unsupported() {
    let ttl =
        "@prefix odrl: <http://www.w3.org/ns/odrl/2/> .\n<https://example.org/a> a odrl:Action .\n";
    assert_eq!(err(ttl), ProfileError::UnsupportedFormat);
    assert_eq!(
        err("PREFIX odrl: <http://www.w3.org/ns/odrl/2/>"),
        ProfileError::UnsupportedFormat
    );
    assert_eq!(
        err("<https://example.org/a> a <http://www.w3.org/ns/odrl/2/Action> ."),
        ProfileError::UnsupportedFormat
    );
    assert_eq!(
        err("<?xml version=\"1.0\"?><rdf:RDF/>"),
        ProfileError::UnsupportedFormat
    );
}

#[test]
fn remote_context_read_with_a_warning() {
    let v = ProfileVocab::parse(&profile_text("hostile/remote-context.jsonld")).unwrap();
    assert_eq!(v.terms().count(), 1);
    assert!(
        v.warnings().iter().any(|w| w.contains("not fetched")),
        "{:?}",
        v.warnings()
    );
}

#[test]
fn deep_value_built_in_code_does_not_overflow() {
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(|| {
            let mut v = json!({"@id": "https://example.org/leaf", "@type": "http://www.w3.org/ns/odrl/2/Action"});
            for i in 0..50_000 {
                let mut m = Map::new();
                m.insert("@id".into(), Value::String(format!("https://example.org/n{i}")));
                m.insert(
                    "http://www.w3.org/ns/odrl/2/includedIn".into(),
                    Value::Array(vec![v]),
                );
                v = Value::Object(m);
            }
            let mut top = Map::new();
            top.insert("@id".into(), "https://example.org/top".into());
            top.insert(
                "@type".into(),
                "http://www.w3.org/ns/odrl/2/Action".into(),
            );
            top.insert("http://www.w3.org/2000/01/rdf-schema#seeAlso".into(), v);
            let top = Value::Object(top);
            let got = ProfileVocab::from_value(&top, &ProfileLimits::default());
            // serde_json's Drop recurses as deep as the value: leak it.
            std::mem::forget(top);
            let v = got.unwrap();
            assert!(v.term("https://example.org/top").is_some());
            assert!(
                v.warnings().iter().any(|w| w.contains("deeper than 64")),
                "{:?}",
                v.warnings()
            );
        })
        .unwrap()
        .join()
        .expect("the walk overflowed its stack or panicked");
}

#[test]
fn includedin_cycle_terminates() {
    let v = ProfileVocab::parse(&profile_text("hostile/cycle.jsonld")).unwrap();
    assert_eq!(v.terms().count(), 4);
    let cycles: Vec<_> = v
        .warnings()
        .iter()
        .filter(|w| w.contains("cycle"))
        .collect();
    assert_eq!(cycles.len(), 2, "{:?}", v.warnings());
    assert!(
        cycles
            .iter()
            .any(|w| w.contains("https://example.org/cycle/x"))
    );
}

#[test]
fn max_terms_limit_exceeded() {
    let text = profile_text("research.jsonld");
    let l = limits(|l| l.max_terms = 2);
    assert_eq!(
        ProfileVocab::parse_with(&text, &l),
        Err(ProfileError::LimitExceeded("max_terms"))
    );
    let l = limits(|l| l.max_terms = 14);
    assert!(ProfileVocab::parse_with(&text, &l).is_ok());
}

#[test]
fn values_per_key_capped() {
    let labels: Vec<Value> = ["a", "b", "c", "d", "e"]
        .iter()
        .map(|l| json!({"@value": format!("label {l}"), "@language": l}))
        .collect();
    let doc = json!({
        "@id": "https://example.org/t",
        "@type": "http://www.w3.org/ns/odrl/2/Action",
        "http://www.w3.org/2000/01/rdf-schema#label": labels
    });
    let l = limits(|l| l.max_values_per_key = 3);
    let v = ProfileVocab::from_value(&doc, &l).unwrap();
    let t = v.term("https://example.org/t").unwrap();
    assert_eq!(t.label.iter().count(), 3);
    assert!(
        v.warnings().iter().any(|w| w.contains("first 3 values")),
        "{:?}",
        v.warnings()
    );
}

#[test]
fn hostile_label_is_plain_and_capped() {
    let v = ProfileVocab::parse(&profile_text("hostile/bidi-html-label.jsonld")).unwrap();
    let t = v.term("https://example.org/hostile/fineTune").unwrap();
    let label = t.label.pick(&en()).unwrap();
    assert!(
        label.starts_with("exe.gnp alert(1) Fine-tune & train pad"),
        "{label}"
    );
    // Cut at 200 characters; a space before the ellipsis is dropped.
    assert!((199..=200).contains(&label.chars().count()), "{label}");
    assert!(label.ends_with('\u{2026}'));
    assert!(!label.contains(['<', '>', '\u{202E}', '\u{200B}']));
    assert_eq!(
        t.definition.pick(&en()),
        Some("To adapt a model using the Asset.")
    );
    let l = limits(|l| l.max_label_chars = 10);
    let v = ProfileVocab::parse_with(&profile_text("hostile/bidi-html-label.jsonld"), &l).unwrap();
    let label = v.terms().next().unwrap().display_label(&en());
    assert!(label.chars().count() <= 10, "{label}");
    assert!(label.ends_with('\u{2026}'), "{label}");
}

#[test]
fn relabelled_core_term_is_read_as_data() {
    let v = ProfileVocab::parse(&profile_text("hostile/relabel-core.jsonld")).unwrap();
    let t = v.term("http://www.w3.org/ns/odrl/2/distribute").unwrap();
    assert_eq!(t.display_label(&en()), "never distribute");
}

#[test]
fn display_messages_name_the_fix() {
    let msg = |e: ProfileError| e.to_string();
    let too_large = msg(ProfileError::TooLarge {
        bytes: 5_000_000,
        limit: 4_194_304,
    });
    assert!(
        too_large.contains("5000000") && too_large.contains("4194304"),
        "{too_large}"
    );
    assert!(msg(ProfileError::Json("expected value".into())).contains("not valid JSON"));
    let html = msg(ProfileError::LooksLikeHtml);
    assert!(html.contains("HTML") && html.contains("JSON-LD"), "{html}");
    let ctx = msg(ProfileError::ContextOnly);
    assert!(
        ctx.contains("@context") && ctx.contains("vocabulary"),
        "{ctx}"
    );
    let schema = msg(ProfileError::JsonSchema);
    assert!(schema.contains("JSON Schema"), "{schema}");
    let policy = msg(ProfileError::LooksLikePolicy);
    assert!(
        policy.contains("policy") && policy.contains("profile"),
        "{policy}"
    );
    let none = msg(ProfileError::NoTerms);
    assert!(none.contains("odrl:Action"), "{none}");
    let format = msg(ProfileError::UnsupportedFormat);
    assert!(
        format.contains("Turtle") && format.contains("ODRL22.json"),
        "{format}"
    );
    let limit = msg(ProfileError::LimitExceeded("max_terms"));
    assert!(limit.contains("max_terms"), "{limit}");
    // Sentences, not debug output.
    for m in [too_large, html, ctx, schema, policy, none, format, limit] {
        assert!(!m.contains("ProfileError") && !m.contains('{'), "{m}");
    }
}

#[test]
fn values_past_max_depth_are_not_read() {
    let doc = json!({
        "@id": "https://example.org/t",
        "@type": "http://www.w3.org/ns/odrl/2/Action",
        "http://www.w3.org/2000/01/rdf-schema#label": [["shallow"]],
        "http://www.w3.org/2004/02/skos/core#definition": [[[["deep"]]]]
    });
    let l = limits(|l| l.max_depth = 4);
    let v = ProfileVocab::from_value(&doc, &l).unwrap();
    let t = v.term("https://example.org/t").unwrap();
    assert_eq!(t.label.pick(&en()), Some("shallow"));
    assert!(t.definition.is_empty(), "{:?}", t.definition);
    assert!(
        v.warnings().iter().any(|w| w.contains("deeper than 4")),
        "{:?}",
        v.warnings()
    );
}

/// `n` actions named through the prefix `p:`, bound to a namespace of
/// `ns_len` characters: a short text that is `n * ns_len` bytes expanded.
fn long_namespace(ns_len: usize, n: usize, extra: &str) -> String {
    let ns = format!("https://example.org/{}#", "a".repeat(ns_len));
    let nodes: Vec<String> = (0..n)
        .map(|i| format!(r#"{{"@id":"p:t{i}","@type":"odrl:Action"{extra}}}"#))
        .collect();
    format!(
        r#"{{"@context":{{"p":"{ns}","odrl":"http://www.w3.org/ns/odrl/2/"}},"@graph":[{}]}}"#,
        nodes.join(",")
    )
}

#[test]
fn short_prefix_long_namespace_refused_not_expanded() {
    // About 70 KB of text, 13 MB once expanded.
    let text = long_namespace(64 * 1024, 200, "");
    let l = limits(|l| l.max_expanded_bytes = 1024 * 1024);
    assert_eq!(
        ProfileVocab::parse_with(&text, &l),
        Err(ProfileError::LimitExceeded("max_expanded_bytes"))
    );
    // Within a raised limit the same text reads.
    let l = limits(|l| l.max_expanded_bytes = 64 * 1024 * 1024);
    assert_eq!(
        ProfileVocab::parse_with(&text, &l).unwrap().terms().count(),
        200
    );
}

#[test]
fn expanded_keys_count_even_when_nothing_is_stored() {
    // Two thousand keys with null values: no triple, but each key is
    // expanded to 64 KiB, 128 MiB in all under the default limits.
    let keys: String = (0..2000).map(|i| format!(r#","p:k{i}":null"#)).collect();
    let text = long_namespace(64 * 1024, 1, &keys);
    assert_eq!(
        ProfileVocab::parse(&text),
        Err(ProfileError::LimitExceeded("max_expanded_bytes"))
    );
}

#[test]
fn chained_term_definitions_refused_not_expanded() {
    // Each definition extends the one before it, so 300 definitions of
    // 4 000 characters expand to 180 MB: past the default limits.
    let mut ctx = Map::new();
    ctx.insert("odrl".into(), json!("http://www.w3.org/ns/odrl/2/"));
    ctx.insert(
        "t0".into(),
        json!(format!("https://example.org/{}/", "a".repeat(4000))),
    );
    for i in 1..300 {
        ctx.insert(
            format!("t{i}"),
            json!(format!("t{}:{}/", i - 1, "b".repeat(4000))),
        );
    }
    let text = json!({
        "@context": Value::Object(ctx),
        "@id": "t299:x", "@type": "odrl:Action"
    })
    .to_string();
    assert_eq!(
        ProfileVocab::parse(&text),
        Err(ProfileError::LimitExceeded("max_expanded_bytes"))
    );
}

#[test]
fn ordinary_profiles_are_far_below_the_expanded_limit() {
    let l = limits(|l| l.max_expanded_bytes = 256 * 1024);
    for name in [
        "research.jsonld",
        "research-expanded.json",
        "odrl22-excerpt.json",
    ] {
        assert!(
            ProfileVocab::parse_with(&profile_text(name), &l).is_ok(),
            "{name}"
        );
    }
}

#[test]
fn long_subject_copied_into_each_triple_counts() {
    // A 1 MiB @id written out in full, with 300 properties: nothing is
    // expanded, but each triple holds a copy of the subject.
    let mut node = Map::new();
    node.insert(
        "@id".into(),
        json!(format!("https://example.org/{}", "a".repeat(1024 * 1024))),
    );
    node.insert("@type".into(), json!("http://www.w3.org/ns/odrl/2/Action"));
    for i in 0..300 {
        node.insert(format!("https://example.org/k{i}"), json!("x"));
    }
    assert_eq!(
        ProfileVocab::parse(&Value::Object(node).to_string()),
        Err(ProfileError::LimitExceeded("max_expanded_bytes"))
    );
}

/// Characters an IRI cannot hold (RFC 3987): bidi controls and other
/// invisible ones, whitespace, and the delimiters `<>"{}|\^` and backquote.
const BAD_IRI_CHARS: &[char] = &[
    '\u{202E}',
    '\u{2066}',
    '\u{200B}',
    '\u{E0041}',
    ' ',
    '<',
    '>',
    '"',
    '{',
    '}',
    '|',
    '\\',
    '^',
    '`',
];

#[test]
fn an_iri_that_holds_what_an_iri_cannot_is_skipped() {
    const ODRL: &str = "http://www.w3.org/ns/odrl/2/";
    for &c in BAD_IRI_CHARS {
        let bad = format!("https://example.org/ev{c}il");
        let v = ProfileVocab::from_value(
            &json!([
                {"@id": bad, "@type": format!("{ODRL}Action")},
                {"@id": "https://example.org/ok", "@type": format!("{ODRL}Action"),
                 format!("{ODRL}includedIn"): {"@id": bad},
                 format!("https://example.org/p{c}"): "x"}
            ]),
            &ProfileLimits::default(),
        )
        .unwrap();
        let iris: Vec<&str> = v.terms().map(|t| t.iri.as_str()).collect();
        assert_eq!(iris, vec!["https://example.org/ok"], "U+{:04X}", c as u32);
        assert!(v.terms().next().unwrap().included_in.is_empty());
        assert!(
            v.warnings().iter().any(|w| w.contains("cannot hold")),
            "{:?}",
            v.warnings()
        );
        // The warning shows the character escaped, never as itself.
        if c != ' ' {
            assert!(
                v.warnings().iter().all(|w| !w.contains(c)),
                "{:?}",
                v.warnings()
            );
        }
    }
}

#[test]
fn an_unlabelled_term_never_shows_a_bidi_control() {
    // The case that reached the prose: no label, so the display label is
    // the IRI's local name.
    let text = "{\"@id\":\"https://example.org/\\u202Eevil\\u202C\",\
                \"@type\":\"http://www.w3.org/ns/odrl/2/Action\"}";
    assert_eq!(err(text), ProfileError::NoTerms);
}

#[test]
fn warnings_are_capped() {
    // Each distinct unknown keyword is a warning that quotes it; a profile
    // can hold hundreds of thousands. A host lists them, so they stop at
    // 100, with one line that says more were left out.
    let keys: Vec<String> = (0..300_000).map(|i| format!(r#""@k{i}":0"#)).collect();
    let text = format!(
        r#"{{"@id":"https://e/x","@type":"http://www.w3.org/ns/odrl/2/Action",{}}}"#,
        keys.join(",")
    );
    let v = ProfileVocab::parse(&text).unwrap();
    assert_eq!(v.warnings().len(), 101);
    assert_eq!(v.warnings()[0], "the key \"@k0\" is not followed");
    assert_eq!(
        v.warnings().last().map(String::as_str),
        Some("more warnings were not listed")
    );
    assert_eq!(v.summary().warnings, 101);
}
