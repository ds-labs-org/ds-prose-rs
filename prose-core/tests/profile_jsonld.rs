//! Reading a profile written as JSON-LD, without a JSON-LD processor: which
//! shapes and which `@context` features are followed, and what is warned
//! about instead.
use prose_core::profile::{ProfileError, ProfileLimits, ProfileVocab, TermKind};
use serde_json::{Value, json};

const ODRL: &str = "http://www.w3.org/ns/odrl/2/";
const SKOS: &str = "http://www.w3.org/2004/02/skos/core#";
const EX: &str = "https://example.org/odrl/profile/research/";

fn profile_text(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/profiles/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn parse(name: &str) -> ProfileVocab {
    ProfileVocab::parse(&profile_text(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn from(v: Value) -> Result<ProfileVocab, ProfileError> {
    ProfileVocab::from_value(&v, &ProfileLimits::default())
}

fn count(v: &ProfileVocab, k: TermKind) -> usize {
    v.terms_of(k).count()
}

fn iris(v: &ProfileVocab) -> Vec<&str> {
    v.terms().map(|t| t.iri.as_str()).collect()
}

fn kinds(v: &ProfileVocab, iri: &str) -> Vec<TermKind> {
    v.term(iri)
        .unwrap_or_else(|| panic!("no term {iri} in {:?}", iris(v)))
        .kinds
        .clone()
}

#[test]
fn expanded_array_like_odrl22_json() {
    let v = parse("odrl22-excerpt.json");
    assert_eq!(v.iri(), Some(ODRL));
    assert_eq!(count(&v, TermKind::Action), 3, "{:?}", iris(&v));
    assert_eq!(count(&v, TermKind::LeftOperand), 5);
    assert_eq!(count(&v, TermKind::Operator), 1);
    assert_eq!(count(&v, TermKind::ConflictTerm), 1);
    assert_eq!(count(&v, TermKind::RightOperand), 0);
    assert_eq!(kinds(&v, &format!("{ODRL}use")), vec![TermKind::Action]);
    assert!(v.warnings().is_empty(), "{:?}", v.warnings());
}

#[test]
fn graph_object_and_single_node() {
    let graph = from(json!({
        "@context": {"odrl": ODRL, "ex": EX},
        "@graph": [
            {"@id": "ex:a", "@type": "odrl:Action"},
            {"@id": "ex:b", "@type": "odrl:LeftOperand"}
        ]
    }))
    .unwrap();
    assert_eq!(iris(&graph), vec![format!("{EX}a"), format!("{EX}b")]);
    assert_eq!(
        kinds(&graph, &format!("{EX}b")),
        vec![TermKind::LeftOperand]
    );

    let single = from(json!({
        "@context": {"odrl": ODRL},
        "@id": "https://example.org/op",
        "@type": "odrl:Operator"
    }))
    .unwrap();
    assert_eq!(iris(&single), vec!["https://example.org/op"]);
    assert!(single.warnings().is_empty(), "{:?}", single.warnings());
}

#[test]
fn embedded_node_objects_are_walked() {
    let v = from(json!({
        "@context": {"odrl": ODRL, "ex": EX},
        "@id": "ex:a",
        "@type": "odrl:Action",
        "odrl:includedIn": {"@id": "ex:b", "@type": "odrl:Action"}
    }))
    .unwrap();
    assert_eq!(count(&v, TermKind::Action), 2, "{:?}", iris(&v));
}

#[test]
fn compacted_equals_expanded() {
    let compacted = parse("research.jsonld");
    let expanded = parse("research-expanded.json");
    assert!(compacted.terms().count() > 0);
    assert!(
        compacted.warnings().is_empty(),
        "{:?}",
        compacted.warnings()
    );
    assert!(expanded.warnings().is_empty(), "{:?}", expanded.warnings());
    assert_eq!(compacted, expanded);
}

/// `v` with every array and every object's keys in reverse order: the same
/// graph, written in another order.
fn reversed(v: &Value) -> Value {
    match v {
        Value::Array(a) => Value::Array(a.iter().rev().map(reversed).collect()),
        Value::Object(o) => Value::Object(
            o.iter()
                .rev()
                .map(|(k, x)| (k.clone(), reversed(x)))
                .collect(),
        ),
        other => other.clone(),
    }
}

#[test]
fn equality_ignores_the_order_the_graph_is_written_in() {
    let written = parse("research.jsonld");
    let v: Value = serde_json::from_str(&profile_text("research.jsonld")).unwrap();
    let other = from(reversed(&v)).unwrap();
    // The order really differs: terms, labels in two languages, hint lists.
    assert_ne!(iris(&written), iris(&other));
    let train = |p: &ProfileVocab| {
        p.term(&format!("{EX}train"))
            .unwrap()
            .label
            .iter()
            .map(|(l, t)| (l.map(String::from), t.to_string()))
            .collect::<Vec<_>>()
    };
    assert_ne!(train(&written), train(&other));
    assert_eq!(written, other);
    // A real difference still tells them apart.
    let mut changed = reversed(&v);
    let s = serde_json::to_string(&changed)
        .unwrap()
        .replace("Trainieren", "Lernen");
    changed = serde_json::from_str(&s).unwrap();
    assert_ne!(written, from(changed).unwrap());
}

#[test]
fn string_prefix_context_and_vocab() {
    let v = from(json!({
        "@context": {"@vocab": ODRL, "ex": EX},
        "@graph": [
            {"@id": "ex:train", "@type": "Action"},
            {"@id": "ex:op", "@type": ["Operator"]}
        ]
    }))
    .unwrap();
    assert_eq!(kinds(&v, &format!("{EX}train")), vec![TermKind::Action]);
    assert_eq!(kinds(&v, &format!("{EX}op")), vec![TermKind::Operator]);
    assert!(v.warnings().is_empty(), "{:?}", v.warnings());

    // An array of contexts merges left to right: the later binding wins.
    let v = from(json!({
        "@context": [{"o": ODRL, "ex": "https://wrong.example/"}, {"ex": EX}],
        "@id": "ex:train",
        "@type": "o:Action"
    }))
    .unwrap();
    assert_eq!(iris(&v), vec![format!("{EX}train")]);
}

#[test]
fn term_definition_type_id() {
    let v = from(json!({
        "@context": {
            "ex": EX,
            "odrl": ODRL,
            "Action": "odrl:Action",
            "kind": {"@id": "http://www.w3.org/1999/02/22-rdf-syntax-ns#type", "@type": "@id"},
            "kindv": {"@id": "http://www.w3.org/1999/02/22-rdf-syntax-ns#type", "@type": "@vocab"},
            "tags": {"@id": "skos:note", "@container": "@set"},
            "skos": SKOS
        },
        "@graph": [
            {"@id": "ex:a", "@type": "Action"},
            {"@id": "ex:b", "kind": "odrl:LeftOperand"},
            {"@id": "ex:c", "kindv": "Action"},
            {"@id": "ex:d", "kind": "odrl:Operator", "tags": ["x"]}
        ]
    }))
    .unwrap();
    assert_eq!(kinds(&v, &format!("{EX}a")), vec![TermKind::Action]);
    assert_eq!(kinds(&v, &format!("{EX}b")), vec![TermKind::LeftOperand]);
    assert_eq!(kinds(&v, &format!("{EX}c")), vec![TermKind::Action]);
    assert_eq!(kinds(&v, &format!("{EX}d")), vec![TermKind::Operator]);
    assert!(
        v.warnings().iter().any(|w| w.contains("@container")),
        "{:?}",
        v.warnings()
    );
}

#[test]
fn remote_context_warns_not_fetched() {
    let v = from(json!({
        "@context": ["https://example.org/context.jsonld", {"odrl": ODRL, "@import": "https://example.org/more.jsonld"}],
        "@id": "https://example.org/t",
        "@type": "odrl:Action"
    }))
    .unwrap();
    assert_eq!(iris(&v), vec!["https://example.org/t"]);
    let w = v.warnings();
    assert!(
        w.iter()
            .any(|w| w.contains("https://example.org/context.jsonld") && w.contains("not fetched")),
        "{w:?}"
    );
    assert!(w.iter().any(|w| w.contains("@import")), "{w:?}");
}

#[test]
fn odrl_remote_context_enables_builtins_silently() {
    let v = from(json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@graph": [
            {"@id": "https://example.org/t", "@type": "Action",
             "skos:definition": "x", "rdfs:label": "T", "dct:title": "T"},
            {"uid": "https://example.org/u", "type": "odrl:LeftOperand"}
        ]
    }))
    .unwrap();
    assert_eq!(kinds(&v, "https://example.org/t"), vec![TermKind::Action]);
    assert_eq!(
        kinds(&v, "https://example.org/u"),
        vec![TermKind::LeftOperand]
    );
    assert!(v.warnings().is_empty(), "{:?}", v.warnings());
}

#[test]
fn unbound_skos_prefix_falls_back_with_one_warning() {
    let v = from(json!({
        "@context": {"odrl": ODRL},
        "@graph": [
            {"@id": "https://example.org/a", "@type": ["odrl:Action", "skos:Concept"], "skos:definition": "A"},
            {"@id": "https://example.org/b", "@type": ["odrl:Action", "skos:Concept"], "skos:note": "B"}
        ]
    }))
    .unwrap();
    assert_eq!(count(&v, TermKind::Action), 2);
    assert_eq!(v.warnings().len(), 1, "{:?}", v.warnings());
    assert!(v.warnings()[0].contains("\"skos\""), "{:?}", v.warnings());
    assert!(v.warnings()[0].contains(SKOS), "{:?}", v.warnings());

    // No context at all: `odrl:` falls back too, once.
    let v = from(json!([
        {"@id": "https://example.org/a", "@type": "odrl:Action"},
        {"@id": "https://example.org/b", "@type": "odrl:Action"}
    ]))
    .unwrap();
    assert_eq!(count(&v, TermKind::Action), 2);
    assert_eq!(v.warnings().len(), 1, "{:?}", v.warnings());
}

#[test]
fn author_binding_of_odrl_elsewhere_respected() {
    let v = from(json!({
        "@context": {"odrl": "https://example.org/not-odrl/"},
        "@graph": [
            {"@id": "https://example.org/a", "@type": "odrl:Action"},
            {"@id": "https://example.org/b", "@type": "http://www.w3.org/ns/odrl/2/Action"}
        ]
    }))
    .unwrap();
    assert_eq!(iris(&v), vec!["https://example.org/b"]);
    assert!(v.warnings().is_empty(), "{:?}", v.warnings());
}

#[test]
fn relative_id_without_base_skipped() {
    let v = from(json!({
        "@context": {"odrl": ODRL},
        "@graph": [
            {"@id": "train", "@type": "odrl:Action"},
            {"@id": "https://example.org/b", "@type": "odrl:Action"}
        ]
    }))
    .unwrap();
    assert_eq!(iris(&v), vec!["https://example.org/b"]);
    assert!(
        v.warnings()
            .iter()
            .any(|w| w.contains("relative") && w.contains("\"train\"")),
        "{:?}",
        v.warnings()
    );

    let v = from(json!({
        "@context": {"odrl": ODRL, "@base": "https://example.org/p/"},
        "@graph": [
            {"@id": "train", "@type": "odrl:Action"},
            {"@id": "#tune", "@type": "odrl:Action"}
        ]
    }))
    .unwrap();
    assert_eq!(
        iris(&v),
        vec!["https://example.org/p/train", "https://example.org/p/#tune"]
    );
    assert!(v.warnings().is_empty(), "{:?}", v.warnings());
}

#[test]
fn null_base_in_a_nested_context_clears_it() {
    let v = from(json!({
        "@context": {"odrl": ODRL, "@base": "https://example.org/p/"},
        "@graph": [
            {"@context": {"@base": null}, "@id": "train", "@type": "odrl:Action"},
            {"@id": "tune", "@type": "odrl:Action"}
        ]
    }))
    .unwrap();
    assert_eq!(iris(&v), vec!["https://example.org/p/tune"]);
    assert!(
        v.warnings().iter().any(|w| w.contains("\"train\"")),
        "{:?}",
        v.warnings()
    );
}

#[test]
fn every_documented_well_known_prefix_falls_back() {
    // The README lists these as the prefixes read without a binding.
    for (prefix, ns) in [
        ("odrl", ODRL),
        ("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"),
        ("rdfs", "http://www.w3.org/2000/01/rdf-schema#"),
        ("owl", "http://www.w3.org/2002/07/owl#"),
        ("skos", SKOS),
        ("dct", "http://purl.org/dc/terms/"),
        ("dcterms", "http://purl.org/dc/terms/"),
        ("xsd", "http://www.w3.org/2001/XMLSchema#"),
        ("prof", "http://www.w3.org/ns/dx/prof/"),
        (
            "prose",
            "https://ds-labs-org.github.io/ds-prose-rs/profile#",
        ),
    ] {
        let v = from(json!({
            "@id": format!("{prefix}:t"),
            "@type": format!("{ODRL}Action")
        }))
        .unwrap_or_else(|e| panic!("{prefix}: {e}"));
        assert_eq!(iris(&v), vec![format!("{ns}t")], "{prefix}");
        assert_eq!(v.warnings().len(), 1, "{prefix}: {:?}", v.warnings());
        assert!(
            v.warnings()[0].contains(&format!("\"{prefix}\"")),
            "{prefix}: {:?}",
            v.warnings()
        );
    }
}

#[test]
fn odrl_context_binds_only_its_own_prefixes() {
    // The W3C context defines dct but not dcterms, prof or prose.
    let silent = from(json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@id": "dct:t", "@type": "odrl:Action"
    }))
    .unwrap();
    assert_eq!(iris(&silent), vec!["http://purl.org/dc/terms/t"]);
    assert!(silent.warnings().is_empty(), "{:?}", silent.warnings());
    for prefix in ["dcterms", "prof", "prose"] {
        let v = from(json!({
            "@context": "http://www.w3.org/ns/odrl.jsonld",
            "@id": format!("{prefix}:t"), "@type": "odrl:Action"
        }))
        .unwrap();
        assert_eq!(v.warnings().len(), 1, "{prefix}: {:?}", v.warnings());
    }
}

#[test]
fn context_language_tags_plain_strings() {
    let rdfs = "http://www.w3.org/2000/01/rdf-schema#";
    let v = from(json!({
        "@context": {
            "@language": "en", "odrl": ODRL, "rdfs": rdfs, "skos": SKOS,
            "count": {"@id": "rdfs:comment", "@type": "http://www.w3.org/2001/XMLSchema#integer"}
        },
        "@graph": [
            {"@id": "https://example.org/a", "@type": "odrl:Action",
             "rdfs:label": "Train",
             "skos:definition": {"@value": "Untagged as written."},
             "skos:note": {"@value": "Tagged as written.", "@language": "de"}},
            {"@context": {"@language": null},
             "@id": "https://example.org/b", "@type": "odrl:Action", "rdfs:label": "Bee"},
            {"@context": {"@language": "FR"},
             "@id": "https://example.org/c", "@type": "odrl:Action", "rdfs:label": "Cé",
             "count": "3"}
        ]
    }))
    .unwrap();
    assert!(v.warnings().is_empty(), "{:?}", v.warnings());
    let pairs = |iri: &str, f: fn(&prose_core::profile::Term) -> &prose_core::profile::LangText| {
        f(v.term(iri).unwrap())
            .iter()
            .map(|(l, t)| (l.map(String::from), t.to_string()))
            .collect::<Vec<_>>()
    };
    let some = |l: &str, t: &str| vec![(Some(l.to_string()), t.to_string())];
    // A plain string takes the default language; a value object keeps its own.
    assert_eq!(
        pairs("https://example.org/a", |t| &t.label),
        some("en", "Train")
    );
    assert_eq!(
        pairs("https://example.org/a", |t| &t.definition),
        vec![(None, "Untagged as written.".to_string())]
    );
    assert_eq!(
        pairs("https://example.org/a", |t| &t.note),
        some("de", "Tagged as written.")
    );
    // `null` clears it in a nested context; another value replaces it.
    assert_eq!(
        pairs("https://example.org/b", |t| &t.label),
        vec![(None, "Bee".to_string())]
    );
    assert_eq!(
        pairs("https://example.org/c", |t| &t.label),
        some("fr", "Cé")
    );
    // A typed term (here an integer read as rdfs:comment) is not a
    // language string: it stays untagged.
    assert_eq!(
        pairs("https://example.org/c", |t| &t.definition),
        vec![(None, "3".to_string())]
    );
}

#[test]
fn relative_ids_resolve_against_a_given_base() {
    let text = format!(
        r##"{{"@context": {{"odrl": "{ODRL}", "owl": "http://www.w3.org/2002/07/owl#"}},
            "@graph": [
              {{"@id": "", "@type": "owl:Ontology"}},
              {{"@id": "#train", "@type": "odrl:Action"}},
              {{"@id": "tune", "@type": "odrl:Action"}},
              {{"@context": {{"@base": "https://other.org/x/"}},
                "@id": "own", "@type": "odrl:Action"}}
            ]}}"##
    );
    let base = "https://example.org/p/profile.jsonld";
    let v = ProfileVocab::parse_with_base(&text, base, &ProfileLimits::default()).unwrap();
    assert!(v.warnings().is_empty(), "{:?}", v.warnings());
    assert_eq!(v.iri(), Some(base));
    // The document's own @base wins over the one given.
    assert_eq!(
        iris(&v),
        vec![
            "https://example.org/p/profile.jsonld#train",
            "https://example.org/p/tune",
            "https://other.org/x/own"
        ]
    );
    // A base that is not absolute is not used, and says so.
    let v =
        ProfileVocab::parse_with_base(&text, "profile.jsonld", &ProfileLimits::default()).unwrap();
    assert_eq!(iris(&v), vec!["https://other.org/x/own"]);
    assert!(
        v.warnings()
            .iter()
            .any(|w| w.contains("\"profile.jsonld\"")),
        "{:?}",
        v.warnings()
    );
}
