//! Profiles written in Turtle (the `turtle` feature): read into the same
//! triple list as JSON-LD, so a profile and its JSON-LD twin are the same
//! vocabulary, under the same limits.
#![cfg(feature = "turtle")]
use prose_core::profile::{ProfileError, ProfileLimits, ProfileVocab, TermKind};

const ODRL: &str = "http://www.w3.org/ns/odrl/2/";

fn profile_text(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/profiles/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn limits(f: impl FnOnce(&mut ProfileLimits)) -> ProfileLimits {
    let mut l = ProfileLimits::default();
    f(&mut l);
    l
}

#[test]
fn turtle_equals_its_jsonld_twin() {
    let ttl = ProfileVocab::parse(&profile_text("research.ttl")).unwrap();
    let jsonld = ProfileVocab::parse(&profile_text("research.jsonld")).unwrap();
    assert!(ttl.warnings().is_empty(), "{:?}", ttl.warnings());
    assert_eq!(ttl.terms().count(), 14);
    assert_eq!(ttl, jsonld);
    assert_eq!(ttl.summary(), jsonld.summary());
    let explicit =
        ProfileVocab::parse_turtle(&profile_text("research.ttl"), &ProfileLimits::default())
            .unwrap();
    assert_eq!(explicit, jsonld);
}

#[test]
fn turtle_written_by_another_tool_equals_its_jsonld_twin() {
    // rdflib wrote this twin in its own order: subjects sorted, objects
    // grouped, language pairs and hint lists in another order.
    let ttl = ProfileVocab::parse(&profile_text("research-rdflib.ttl")).unwrap();
    let jsonld = ProfileVocab::parse(&profile_text("research.jsonld")).unwrap();
    assert!(ttl.warnings().is_empty(), "{:?}", ttl.warnings());
    let order = |p: &ProfileVocab| p.terms().map(|t| t.iri.clone()).collect::<Vec<_>>();
    assert_ne!(order(&ttl), order(&jsonld));
    assert_eq!(ttl, jsonld);
    assert_eq!(ttl.summary(), jsonld.summary());
}

#[test]
fn turtle_is_recognised_from_its_first_characters() {
    let body = format!("<https://example.org/a> a <{ODRL}Action> .\n");
    for text in [
        format!("@prefix odrl: <{ODRL}> .\n<https://example.org/a> a odrl:Action ."),
        format!("PREFIX odrl: <{ODRL}>\n<https://example.org/a> a odrl:Action ."),
        format!("# a comment\n{body}"),
        format!("\u{FEFF}{body}"),
        body.clone(),
    ] {
        let v = ProfileVocab::parse(&text).unwrap_or_else(|e| panic!("{text:?}: {e}"));
        assert_eq!(
            v.term("https://example.org/a").map(|t| t.kinds.clone()),
            Some(vec![TermKind::Action]),
            "{text:?}"
        );
    }
}

#[test]
fn markup_and_rdf_xml_are_still_refused() {
    let html = profile_text("hostile/landing.html");
    assert_eq!(ProfileVocab::parse(&html), Err(ProfileError::LooksLikeHtml));
    assert_eq!(
        ProfileVocab::parse("<html><body>x</body></html>"),
        Err(ProfileError::LooksLikeHtml)
    );
    assert_eq!(
        ProfileVocab::parse("<?xml version=\"1.0\"?><rdf:RDF/>"),
        Err(ProfileError::UnsupportedFormat)
    );
}

#[test]
fn turtle_syntax_error_is_reported_not_guessed() {
    let missing_dot = format!("@prefix odrl: <{ODRL}> .\n<https://example.org/a> a odrl:Action");
    let e = ProfileVocab::parse(&missing_dot).unwrap_err();
    assert!(matches!(e, ProfileError::Turtle(_)), "{e:?}");
    assert!(e.to_string().contains("not valid Turtle"), "{e}");

    let prose = ProfileVocab::parse("Research data profile, version 1").unwrap_err();
    assert!(matches!(prose, ProfileError::Turtle(_)), "{prose:?}");

    // A relative IRI with no @base cannot be made absolute.
    let relative = format!("<train> a <{ODRL}Action> .");
    assert!(matches!(
        ProfileVocab::parse(&relative),
        Err(ProfileError::Turtle(_))
    ));
    let based = format!("@base <https://example.org/p/> .\n<train> a <{ODRL}Action> .");
    let v = ProfileVocab::parse(&based).unwrap();
    assert!(v.term("https://example.org/p/train").is_some());
}

#[test]
fn turtle_too_large_before_parse() {
    let text = "@prefix x: <".repeat(20);
    assert_eq!(
        ProfileVocab::parse_with(&text, &limits(|l| l.max_bytes = 100)),
        Err(ProfileError::TooLarge {
            bytes: 240,
            limit: 100
        })
    );
    assert_eq!(
        ProfileVocab::parse_turtle(&text, &limits(|l| l.max_bytes = 100)),
        Err(ProfileError::TooLarge {
            bytes: 240,
            limit: 100
        })
    );
}

#[test]
fn turtle_limits_terms_and_values_per_key() {
    let text = profile_text("research.ttl");
    assert_eq!(
        ProfileVocab::parse_with(&text, &limits(|l| l.max_terms = 2)),
        Err(ProfileError::LimitExceeded("max_terms"))
    );
    let labels = ["aa", "bb", "cc", "dd", "ee"]
        .iter()
        .map(|l| format!("\"label {l}\"@{l}"))
        .collect::<Vec<_>>()
        .join(", ");
    let text = format!(
        "<https://example.org/t> a <{ODRL}Action> ; <http://www.w3.org/2000/01/rdf-schema#label> {labels} ."
    );
    let v = ProfileVocab::parse_with(&text, &limits(|l| l.max_values_per_key = 3)).unwrap();
    assert_eq!(
        v.term("https://example.org/t")
            .unwrap()
            .label
            .iter()
            .count(),
        3
    );
    assert!(
        v.warnings().iter().any(|w| w.contains("first 3 values")),
        "{:?}",
        v.warnings()
    );
}

#[test]
fn turtle_hostile_text_is_plain() {
    let text = format!(
        "<https://example.org/t> a <{ODRL}Action> ;\n  <http://www.w3.org/2000/01/rdf-schema#label> \"\\u202Eevil <b>tune</b> &amp; more\"@EN ."
    );
    let v = ProfileVocab::parse(&text).unwrap();
    let t = v.term("https://example.org/t").unwrap();
    assert_eq!(
        t.label.iter().collect::<Vec<_>>(),
        vec![(Some("en"), "evil tune & more")]
    );
}

#[test]
fn turtle_blank_node_subject_is_not_a_term() {
    let text = format!(
        "[] a <{ODRL}Action> .\n<https://example.org/a> a <{ODRL}Action> ; <{ODRL}includedIn> [ a <{ODRL}Action> ] ."
    );
    let v = ProfileVocab::parse(&text).unwrap();
    let iris: Vec<_> = v.terms().map(|t| t.iri.as_str()).collect();
    assert_eq!(iris, vec!["https://example.org/a"]);
}

#[test]
fn turtle_with_blank_nodes_equals_its_jsonld_twin() {
    // Turtle names blank nodes at random, JSON-LD in order: equal only if
    // no blank node label reaches the vocabulary.
    let ttl = format!(
        "@prefix odrl: <{ODRL}> .\n@prefix owl: <http://www.w3.org/2002/07/owl#> .\n@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\
         [] a owl:Ontology ; rdfs:label \"Anonymous\"@en .\n\
         <https://example.org/a> a odrl:Action ; rdfs:subClassOf [ a owl:Restriction ] ; odrl:includedIn [ a odrl:Action ] .\n"
    );
    let jsonld = format!(
        r#"[{{"@type":"http://www.w3.org/2002/07/owl#Ontology","http://www.w3.org/2000/01/rdf-schema#label":{{"@value":"Anonymous","@language":"en"}}}},
           {{"@id":"https://example.org/a","@type":"{ODRL}Action","http://www.w3.org/2000/01/rdf-schema#subClassOf":{{"@type":"http://www.w3.org/2002/07/owl#Restriction"}},"{ODRL}includedIn":{{"@type":"{ODRL}Action"}}}}]"#
    );
    let ttl = ProfileVocab::parse(&ttl).unwrap();
    let jsonld = ProfileVocab::parse(&jsonld).unwrap();
    assert_eq!(ttl.iri(), None);
    assert_eq!(ttl, jsonld);
}

#[test]
fn turtle_deep_nesting_does_not_overflow() {
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(|| {
            let n = 50_000;
            let p = "<https://example.org/p>";
            let mut text =
                format!("<https://example.org/t> a <{ODRL}Action> .\n<https://example.org/s> {p} ");
            text.push_str(&format!("[ {p} ").repeat(n));
            text.push_str("[] ");
            text.push_str(&"] ".repeat(n));
            text.push_str(".\n<https://example.org/u> ");
            text.push_str(p);
            text.push(' ');
            text.push_str(&"( ".repeat(n));
            text.push_str(&") ".repeat(n));
            text.push_str(".\n");
            let v = ProfileVocab::parse(&text).unwrap();
            assert!(v.term("https://example.org/t").is_some());
        })
        .unwrap()
        .join()
        .expect("the Turtle reader overflowed its stack or panicked");
}

#[test]
fn turtle_short_prefix_long_namespace_refused() {
    let mut text = format!(
        "@prefix odrl: <{ODRL}> .\n@prefix p: <https://example.org/{}#> .\n",
        "a".repeat(64 * 1024)
    );
    for i in 0..200 {
        text.push_str(&format!("p:t{i} a odrl:Action .\n"));
    }
    let l = limits(|l| l.max_expanded_bytes = 1024 * 1024);
    assert_eq!(
        ProfileVocab::parse_with(&text, &l),
        Err(ProfileError::LimitExceeded("max_expanded_bytes"))
    );
}

#[test]
fn n_triples_is_read_as_turtle() {
    // N-Triples is a subset of Turtle, so the feature reads it too.
    let text = format!(
        "<https://example.org/a> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <{ODRL}Action> .\n\
         <https://example.org/a> <http://www.w3.org/2000/01/rdf-schema#label> \"Ay\"@en .\n"
    );
    let v = ProfileVocab::parse(&text).unwrap();
    let t = v.term("https://example.org/a").unwrap();
    assert_eq!(t.kinds, vec![TermKind::Action]);
    assert_eq!(t.display_label(&[]), "Ay");
}

#[test]
fn context_language_equals_turtle_language_tags() {
    let jsonld = ProfileVocab::parse(&format!(
        r#"{{"@context": {{"@language": "en", "odrl": "{ODRL}",
              "rdfs": "http://www.w3.org/2000/01/rdf-schema#"}},
            "@id": "https://example.org/train", "@type": "odrl:Action",
            "rdfs:label": "Train"}}"#
    ))
    .unwrap();
    let ttl = ProfileVocab::parse(&format!(
        "@prefix odrl: <{ODRL}> .\n@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\
         <https://example.org/train> a odrl:Action ; rdfs:label \"Train\"@en .\n"
    ))
    .unwrap();
    assert!(jsonld.warnings().is_empty(), "{:?}", jsonld.warnings());
    assert_eq!(jsonld, ttl);
}

const RELATIVE_TTL: &str = "@prefix odrl: <http://www.w3.org/ns/odrl/2/> .\n\
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\
@prefix owl: <http://www.w3.org/2002/07/owl#> .\n\
<> a owl:Ontology ; rdfs:label \"P\"@en .\n\
<#train> a odrl:Action ; rdfs:label \"Train\"@en ; odrl:includedIn <use> .\n\
<https://example.org/abs> a odrl:Action ; odrl:includedIn <#train> .\n";

#[test]
fn turtle_relative_iris_without_a_base_are_skipped_not_refused() {
    // As in JSON-LD: what cannot be made absolute is skipped with a warning.
    let v = ProfileVocab::parse(RELATIVE_TTL).unwrap();
    let iris: Vec<&str> = v.terms().map(|t| t.iri.as_str()).collect();
    assert_eq!(iris, vec!["https://example.org/abs"]);
    assert_eq!(v.iri(), None);
    assert!(
        v.term("https://example.org/abs")
            .unwrap()
            .included_in
            .is_empty()
    );
    assert!(
        v.warnings()
            .iter()
            .any(|w| w.contains("relative") && w.contains("\"#train\"")),
        "{:?}",
        v.warnings()
    );
}

#[test]
fn turtle_relative_iris_resolve_against_a_given_base() {
    let base = "https://example.org/p/profile.ttl";
    let l = ProfileLimits::default();
    let v = ProfileVocab::parse_with_base(RELATIVE_TTL, base, &l).unwrap();
    assert!(v.warnings().is_empty(), "{:?}", v.warnings());
    assert_eq!(v.iri(), Some(base));
    let train = v.term("https://example.org/p/profile.ttl#train").unwrap();
    assert_eq!(
        train.included_in,
        vec!["https://example.org/p/use".to_string()]
    );
    assert_eq!(
        ProfileVocab::parse_turtle_with_base(RELATIVE_TTL, base, &l).unwrap(),
        v
    );
    // The same profile in JSON-LD, read against the same base, is equal.
    let jsonld = format!(
        r##"{{"@context": {{"odrl": "{ODRL}", "owl": "http://www.w3.org/2002/07/owl#",
              "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
              "odrl:includedIn": {{"@type": "@id"}}}},
            "@graph": [
              {{"@id": "", "@type": "owl:Ontology", "rdfs:label": {{"@value": "P", "@language": "en"}}}},
              {{"@id": "#train", "@type": "odrl:Action",
                "rdfs:label": {{"@value": "Train", "@language": "en"}}, "odrl:includedIn": "use"}},
              {{"@id": "https://example.org/abs", "@type": "odrl:Action", "odrl:includedIn": "#train"}}
            ]}}"##
    );
    assert_eq!(ProfileVocab::parse_with_base(&jsonld, base, &l).unwrap(), v);
}
