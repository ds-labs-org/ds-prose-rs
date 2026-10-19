//! `prose:plural`, the ds-prose hint that a left operand's label is a plural
//! noun phrase, so its operators agree in the plural ("the time windows are
//! earlier than"). A ds-prose convention, not ODRL; read on left operands
//! only, in JSON-LD and Turtle alike.
use prose_core::edit::{read_model, sentences_with};
use prose_core::profile::{Prefixes, ProfileLimits, ProfileVocab, Term, Wording};
use prose_core::render_with;
use prose_core::wording::{OperatorQuery, operator_phrase};
use serde_json::{Value, json};

const EX: &str = "https://example.org/odrl/profile/research/";
const PROSE: &str = "https://ds-labs-org.github.io/ds-prose-rs/profile#";

fn profile_text(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/profiles/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn from(v: Value) -> ProfileVocab {
    ProfileVocab::from_value(&v, &ProfileLimits::default()).unwrap()
}

fn term<'a>(v: &'a ProfileVocab, local: &str) -> &'a Term {
    v.term(&format!("{EX}{local}"))
        .unwrap_or_else(|| panic!("no term {local}"))
}

fn wording() -> Wording {
    Wording::new()
        .with_profile(ProfileVocab::parse(&profile_text("plural-operand.jsonld")).unwrap())
}

fn policy(left: &str, op: &str) -> String {
    json!({
        "@context": {"@vocab": "http://www.w3.org/ns/odrl/2/", "ex": EX},
        "@type": "Offer", "uid": "urn:p",
        "permission": [{"action": "use", "target": "urn:t",
            "constraint": [{"leftOperand": left, "operator": op, "rightOperand": 5}]}]
    })
    .to_string()
}

fn read(left: &str, op: &str) -> String {
    render_with(&policy(left, op), &wording())
        .unwrap()
        .document
        .policies[0]
        .rules[0]
        .conditions[0]
        .text
        .clone()
}

fn edited(left: &str, op: &str) -> String {
    let doc = read_model(&policy(left, op)).unwrap();
    sentences_with(&doc, &wording())[0].rules[0]
        .conditions
        .items[0]
        .sentence
        .plain()
}

#[test]
fn the_hint_is_read_on_marked_left_operands() {
    let v = ProfileVocab::parse(&profile_text("plural-operand.jsonld")).unwrap();
    assert!(v.warnings().is_empty(), "{:?}", v.warnings());
    assert!(term(&v, "timeWindows").hints.plural);
    assert!(term(&v, "dateTime").hints.plural);
    assert!(!term(&v, "window").hints.plural);
    assert!(!term(&v, "earlierThan").hints.plural);
}

#[test]
fn the_hint_is_a_boolean_like_prose_closed() {
    let ctx = json!({"odrl": "http://www.w3.org/ns/odrl/2/", "ex": EX, "prose": PROSE,
                     "xsd": "http://www.w3.org/2001/XMLSchema#"});
    let v = from(json!({
        "@context": ctx,
        "@graph": [
            {"@id": "ex:a", "@type": "odrl:LeftOperand", "prose:plural": true},
            {"@id": "ex:b", "@type": "odrl:LeftOperand", "prose:plural": "true"},
            {"@id": "ex:c", "@type": "odrl:LeftOperand",
             "prose:plural": {"@value": "1", "@type": "xsd:boolean"}},
            {"@id": "ex:d", "@type": "odrl:LeftOperand", "prose:plural": false},
            {"@id": "ex:e", "@type": "odrl:LeftOperand", "prose:plural": "yes"},
            {"@id": "ex:f", "@type": "odrl:LeftOperand", "prose:plural": {"@id": "ex:true"}},
            {"@id": "ex:g", "@type": "odrl:LeftOperand", "prose:plural": 7},
            // Only a left operand carries hints.
            {"@id": "ex:h", "@type": "odrl:Action", "prose:plural": true}
        ]
    }));
    for (local, want) in [
        ("a", true),
        ("b", true),
        ("c", true),
        ("d", false),
        ("e", false),
        ("f", false),
        ("g", false),
        ("h", false),
    ] {
        assert_eq!(term(&v, local).hints.plural, want, "{local}");
    }
}

#[test]
fn the_hint_follows_the_values_limit_of_the_other_hints() {
    let ctx = json!({"odrl": "http://www.w3.org/ns/odrl/2/", "ex": EX, "prose": PROSE});
    let many = |key: &str| {
        let mut values = vec![json!("no"); 400];
        values.push(json!(true));
        from(json!({
            "@context": ctx,
            "@graph": [{"@id": "ex:a", "@type": "odrl:LeftOperand", key: values}]
        }))
    };
    let (plural, closed) = (many("prose:plural"), many("prose:closed"));
    assert_eq!(
        term(&plural, "a").hints.plural,
        term(&closed, "a").hints.closed
    );
}

#[test]
fn the_hint_changes_the_vocabulary() {
    let marked = ProfileVocab::parse(&profile_text("plural-operand.jsonld")).unwrap();
    let text =
        profile_text("plural-operand.jsonld").replace("\"prose:plural\": true", "\"x:y\": 1");
    let unmarked =
        ProfileVocab::parse(&text.replace("\"prose\":", "\"x\": \"urn:x#\", \"prose\":")).unwrap();
    assert_ne!(marked, unmarked);
}

#[cfg(feature = "turtle")]
#[test]
fn the_hint_reads_the_same_in_turtle_and_json_ld() {
    let ttl = ProfileVocab::parse(&profile_text("plural-operand.ttl")).unwrap();
    let jsonld = ProfileVocab::parse(&profile_text("plural-operand.jsonld")).unwrap();
    assert!(ttl.warnings().is_empty(), "{:?}", ttl.warnings());
    assert_eq!(ttl, jsonld);
    assert!(term(&ttl, "timeWindows").hints.plural);
    assert!(!term(&ttl, "window").hints.plural);
}

#[test]
fn a_plural_label_takes_plural_operators_in_reader_editor_and_dropdown() {
    let p = Prefixes::of_document(&json!({"@context": {"ex": EX}}));
    let w = wording();
    for (left, subject, verb_is, verb_has) in [
        ("ex:timeWindows", "the time windows", "are", "have"),
        ("ex:window", "the time window", "is", "has"),
        // A relabelled built-in operand is plural only when the profile says so.
        ("ex:dateTime", "the moments", "are", "have"),
    ] {
        for (op, tail) in [
            ("ex:earlierThan", format!("{verb_is} earlier than")),
            ("ex:hasOverlap", format!("{verb_has} overlap with")),
            ("eq", format!("{verb_is} equal to")),
            // A local name of dateTime is a point in time, whoever declares it.
            (
                "lt",
                if left == "ex:dateTime" {
                    format!("{verb_is} before")
                } else {
                    format!("{verb_is} less than")
                },
            ),
        ] {
            let want = format!("{subject} {tail} 5");
            assert_eq!(read(left, op), want, "reader {left} {op}");
            assert_eq!(edited(left, op), want, "editor {left} {op}");
            let dropdown = operator_phrase(&OperatorQuery::new(left, op).wording(&w, &p));
            assert_eq!(dropdown, tail, "dropdown {left} {op}");
        }
    }
}

#[test]
fn the_built_in_plural_applies_only_without_a_profile_label() {
    let p = Prefixes::of_document(&json!({"@context": {"ex": EX}}));
    let w = wording();
    let q = |left: &str| operator_phrase(&OperatorQuery::new(left, "eq").wording(&w, &p));
    // ODRL's own dateTime keeps its built-in plural, the profile's ex:dateTime
    // follows its hint, and a label without the hint is singular.
    assert_eq!(q("dateTime"), "are equal to");
    assert_eq!(q("ex:dateTime"), "are equal to");
    assert_eq!(q("ex:window"), "is equal to");
    assert_eq!(q("count"), "is equal to");
}

#[test]
fn the_hint_is_not_read_on_a_term_in_odrls_namespace() {
    let v = from(json!({
        "@context": {"odrl": "http://www.w3.org/ns/odrl/2/", "prose": PROSE,
                     "rdfs": "http://www.w3.org/2000/01/rdf-schema#"},
        "@graph": [{"@id": "odrl:count", "@type": "odrl:LeftOperand",
                    "rdfs:label": "Counts", "prose:plural": true}]
    }));
    let w = Wording::new().with_profile(v);
    let p = Prefixes::default();
    let phrase = operator_phrase(&OperatorQuery::new("count", "eq").wording(&w, &p));
    assert_eq!(phrase, "is equal to");
}
