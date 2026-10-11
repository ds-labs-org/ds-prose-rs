//! What a hostile profile or policy may cost: every text within the size
//! limit is read in time that grows with its size, not with its square. Each
//! input below took tens of seconds or more with a quadratic step; read in
//! linear time it takes well under a second, even in a debug build.
use prose_core::profile::{Prefixes, ProfileLimits, ProfileVocab};
use serde_json::Value;
use std::sync::mpsc;
use std::time::Duration;

const ODRL: &str = "http://www.w3.org/ns/odrl/2/";
const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";

/// Runs `f` on its own thread and fails if it takes longer than the bound.
/// The bound is generous: it only tells linear from quadratic.
fn quick<T: Send + 'static>(what: &str, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(Duration::from_secs(15)) {
        Ok(v) => v,
        Err(_) => panic!("{what}: still reading after 15 s"),
    }
}

fn big() -> ProfileLimits {
    let mut l = ProfileLimits::default();
    l.max_bytes = 16 * 1024 * 1024;
    l
}

fn action(extra: &str) -> String {
    format!(r#"{{"@id":"https://example.org/x","@type":"{ODRL}Action"{extra}}}"#)
}

#[test]
fn many_ampersands_in_a_label() {
    let text = action(&format!(r#","{RDFS}label":"{}""#, "&".repeat(2_000_000)));
    let v = quick("ampersands", move || ProfileVocab::parse(&text).unwrap());
    let label = v.terms().next().unwrap().label.iter().next().unwrap().1;
    assert!(label.starts_with("&&&") && label.ends_with('\u{2026}'));
}

#[test]
fn many_small_arrays_under_one_key() {
    let items = vec!["[1]"; 600_000].join(",");
    let text = action(&format!(r#","https://example.org/p":[{items}]"#));
    let v = quick("arrays", move || ProfileVocab::parse(&text).unwrap());
    assert!(
        v.warnings().iter().any(|w| w.contains("first 256 values")),
        "{:?}",
        v.warnings()
    );
}

#[test]
fn many_prefixes_in_a_context() {
    let ctx: Vec<String> = (0..400_000)
        .map(|i| format!(r#""p{i}":"https://e/{i}/""#))
        .collect();
    let text = format!(
        r#"{{"@context":{{{}}},"@id":"https://example.org/x","@type":"{ODRL}Action"}}"#,
        ctx.join(",")
    );
    let v = quick("context prefixes", move || {
        ProfileVocab::parse_with(&text, &big()).unwrap()
    });
    assert_eq!(v.terms().count(), 1);
}

#[test]
fn many_nested_contexts_under_a_large_one() {
    let n = 40_000;
    let ctx: Vec<String> = (0..n)
        .map(|i| format!(r#""p{i}":"https://e/{i}/""#))
        .collect();
    let nodes: Vec<String> = (0..n)
        .map(|i| format!(r#"{{"@context":{{}},"@id":"https://example.org/n{i}"}}"#))
        .collect();
    let text = format!(
        r#"{{"@context":{{{}}},"@id":"https://example.org/x","@type":"{ODRL}Action","https://example.org/p":[{}]}}"#,
        ctx.join(","),
        nodes.join(",")
    );
    let v = quick("nested contexts", move || {
        ProfileVocab::parse(&text).unwrap()
    });
    assert_eq!(v.terms().count(), 1);
}

#[test]
fn wide_subclass_graph() {
    // Every class is a subclass of the next 256: each reaches all others.
    let n = 1_000;
    let nodes: Vec<String> = (0..n)
        .map(|i| {
            let sup: Vec<String> = (1..=256)
                .map(|j| format!(r#"{{"@id":"https://e/c{}"}}"#, (i + j) % n))
                .collect();
            format!(
                r#"{{"@id":"https://e/c{i}","@type":"{ODRL}Action","{RDFS}subClassOf":[{}]}}"#,
                sup.join(",")
            )
        })
        .collect();
    let text = format!("[{}]", nodes.join(","));
    let v = quick("subclass graph", move || {
        ProfileVocab::parse_with(&text, &big()).unwrap()
    });
    assert_eq!(v.terms().count(), n);
}

#[test]
fn many_distinct_is_defined_by_objects() {
    let nodes: Vec<String> = (0..150_000)
        .map(|i| {
            format!(r#"{{"@id":"https://e/n{i}","{RDFS}isDefinedBy":{{"@id":"https://e/d{i}"}}}}"#)
        })
        .collect();
    let text = format!(
        r#"[{{"@id":"https://e/x","@type":"{ODRL}Action"}},{}]"#,
        nodes.join(",")
    );
    let v = quick("isDefinedBy", move || {
        ProfileVocab::parse_with(&text, &big()).unwrap()
    });
    assert_eq!(v.iri(), Some("https://e/d0"));
}

#[test]
fn many_prefixes_in_a_policy_context() {
    // Chained compact IRIs ("p0": "q1:x") are resolved too.
    let ctx: Vec<String> = (0..100_000)
        .map(|i| format!(r#""p{i}":"q{}:x""#, i + 1))
        .collect();
    let text = format!(r#"{{"@context":{{{}}}}}"#, ctx.join(","));
    let v: Value = serde_json::from_str(&text).unwrap();
    let p = quick("policy context", move || Prefixes::of_document(&v));
    assert_eq!(p.expand("p0:y"), "q1:xy");
}

#[cfg(feature = "turtle")]
#[test]
fn many_prefixes_in_turtle() {
    let mut text = String::new();
    for i in 0..300_000 {
        text.push_str(&format!("@prefix p{i}: <https://e/{i}/> .\n"));
    }
    text.push_str(&format!("<https://example.org/x> a <{ODRL}Action> .\n"));
    let v = quick("turtle prefixes", move || {
        ProfileVocab::parse_with(&text, &big()).unwrap()
    });
    assert_eq!(v.terms().count(), 1);
}

#[test]
fn many_left_operands_suggesting_a_dense_hierarchy() {
    // 256 values, each narrower than every other (the most links one node
    // may have), and 15 000 left operands that all suggest them.
    const PROSE: &str = "https://ds-labs-org.github.io/ds-prose-rs/profile#";
    const SKOS: &str = "http://www.w3.org/2004/02/skos/core#";
    let ids: Vec<String> = (0..256)
        .map(|i| format!(r#"{{"@id":"https://e/v{i}"}}"#))
        .collect();
    let all = ids.join(",");
    let mut nodes: Vec<String> = (0..256)
        .map(|i| {
            format!(
                r#"{{"@id":"https://e/v{i}","@type":"{ODRL}RightOperand","{SKOS}broader":[{all}]}}"#
            )
        })
        .collect();
    for i in 0..15_000 {
        nodes.push(format!(
            r#"{{"@id":"https://e/lo{i}","@type":"{ODRL}LeftOperand","{PROSE}rightOperand":{{"@id":"https://e/v{}"}}}}"#,
            i % 256
        ));
    }
    let text = format!("[{}]", nodes.join(","));
    let v = quick("dense value hierarchy", move || {
        ProfileVocab::parse_with(&text, &big()).unwrap()
    });
    let lo = v.term("https://e/lo0").unwrap();
    assert!(!lo.hints.value_tree.is_empty());
    assert!(lo.hints.value_tree.len() <= 256);
}

#[test]
fn short_prefix_bound_to_a_long_namespace() {
    // 300 KB of text naming 300 terms through a 1 MiB namespace: 300 MB and
    // more once expanded. Refused under the default limits, quickly.
    let ns = format!("https://e/{}#", "a".repeat(1024 * 1024));
    let nodes: Vec<String> = (0..300)
        .map(|i| format!(r#"{{"@id":"p:t{i}","@type":"{ODRL}Action"}}"#))
        .collect();
    let text = format!(
        r#"{{"@context":{{"p":"{ns}"}},"@graph":[{}]}}"#,
        nodes.join(",")
    );
    let r = quick("long namespace", move || ProfileVocab::parse(&text));
    assert!(
        matches!(
            r,
            Err(prose_core::profile::ProfileError::LimitExceeded(
                "max_expanded_bytes"
            ))
        ),
        "{r:?}"
    );
}

#[test]
fn default_limits_are_the_documented_ones() {
    // The README and the npm READMEs quote these numbers.
    let l = ProfileLimits::default();
    assert_eq!(l.max_bytes, 4 * 1024 * 1024);
    assert_eq!(l.max_terms, 20_000);
    assert_eq!(l.max_depth, 64);
    assert_eq!(l.max_values_per_key, 256);
    assert_eq!(l.max_label_chars, 200);
    assert_eq!(l.max_text_chars, 4_000);
    assert_eq!(l.max_expanded_bytes, 32 * 1024 * 1024);
}

#[test]
fn many_empty_contexts_under_a_long_vocab() {
    // Every node with its own @context, even an empty one, used to copy the
    // inherited @vocab and @base: 3 MiB times 400 000 here.
    let vocab = format!("x:{}", "a".repeat(3 * 1024 * 1024));
    let items = vec![r#"{"@context":{}}"#; 400_000].join(",");
    let text = format!(
        r#"{{"@context":{{"@vocab":"{vocab}"}},"@graph":[{{"@id":"https://e/x","@type":"{ODRL}Action"}},{items}]}}"#
    );
    let v = quick("vocab per context", move || {
        ProfileVocab::parse_with(&text, &big()).unwrap()
    });
    assert_eq!(v.terms().count(), 1);
}

#[test]
fn a_vocab_counts_against_the_expansion_limit() {
    // An @vocab written as a compact IRI with an unbound prefix is kept as
    // written; it is read, so it is counted, used or not.
    let vocab = format!("x:{}", "a".repeat(2 * 1024 * 1024));
    let text = format!(
        r#"{{"@context":{{"@vocab":"{vocab}"}},"@id":"https://e/x","@type":"{ODRL}Action"}}"#
    );
    let mut l = ProfileLimits::default();
    l.max_expanded_bytes = 1024 * 1024;
    assert_eq!(
        ProfileVocab::parse_with(&text, &l),
        Err(prose_core::profile::ProfileError::LimitExceeded(
            "max_expanded_bytes"
        ))
    );
}
