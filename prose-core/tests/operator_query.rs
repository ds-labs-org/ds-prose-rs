//! One entry point for operator wording: `wording::operator_phrase` with an
//! `OperatorQuery`. The four older functions are thin wrappers over it.
use prose_core::profile::{Prefixes, ProfileLimits, ProfileVocab, Wording};
use prose_core::wording::{OperatorQuery, ValueShape, operator_phrase};
use serde_json::json;

const EX: &str = "https://example.org/odrl/profile/research/";

fn profile() -> Wording {
    let v = ProfileVocab::from_value(
        &json!({
            "@context": {"odrl": "http://www.w3.org/ns/odrl/2/", "ex": EX,
                         "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
                         "owl": "http://www.w3.org/2002/07/owl#"},
            "@graph": [
                {"@id": "https://example.org/p", "@type": "owl:Ontology"},
                {"@id": "ex:earlierThan", "@type": "odrl:Operator", "rdfs:label": "Earlier than"},
                {"@id": "ex:hasOverlap", "@type": "odrl:Operator", "rdfs:label": "Has overlap with"},
                {"@id": "ex:moment", "@type": "odrl:LeftOperand", "rdfs:label": "Moment"}
            ]
        }),
        &ProfileLimits::default(),
    )
    .unwrap();
    Wording::new().with_profile(v)
}

fn prefixes() -> Prefixes {
    Prefixes::of_document(&json!({"@context": {"ex": EX}}))
}

const SHAPES: [ValueShape; 4] = [
    ValueShape::Instant,
    ValueShape::DateOnly,
    ValueShape::Duration,
    ValueShape::Other,
];

const LEFTS: [&str; 6] = [
    "dateTime",
    "odrl:dateTime",
    "timeInterval",
    "count",
    "ex:moment",
    "ex:unknown",
];

const OPS: [&str; 10] = [
    "lt",
    "lteq",
    "gt",
    "gteq",
    "eq",
    "hasPart",
    "odrl:isAnyOf",
    "ex:near",
    "ex:earlierThan",
    "ex:hasOverlap",
];

#[test]
fn the_query_reads_the_built_in_wording_without_a_profile() {
    let q = OperatorQuery::new("dateTime", "lt");
    assert_eq!(operator_phrase(&q), "are before");
    let q = OperatorQuery::new("dateTime", "lt").value(ValueShape::DateOnly);
    assert_eq!(operator_phrase(&q), "are before the start of");
    let q = OperatorQuery::new("timeInterval", "lt").value(ValueShape::DateOnly);
    assert_eq!(operator_phrase(&q), "is less than");
    let q = OperatorQuery::new("count", "ex:near");
    assert_eq!(operator_phrase(&q), "is related by near to");
}

#[test]
fn the_query_reads_a_profile_label_and_agrees_in_number() {
    let (w, p) = (profile(), prefixes());
    let q = OperatorQuery::new("dateTime", "ex:earlierThan").wording(&w, &p);
    assert_eq!(operator_phrase(&q), "are earlier than");
    let q = OperatorQuery::new("ex:moment", "ex:earlierThan").wording(&w, &p);
    assert_eq!(operator_phrase(&q), "is earlier than");
}

#[test]
#[allow(deprecated)]
fn each_older_function_equals_the_new_entry_point() {
    use prose_core::wording::{operator_for, operator_for_date, operator_with, operator_with_date};
    let (w, p) = (profile(), prefixes());
    for left in LEFTS {
        for op in OPS {
            assert_eq!(
                operator_for(left, op),
                operator_phrase(&OperatorQuery::new(left, op)),
                "operator_for {left} {op}"
            );
            assert_eq!(
                operator_for_date(left, op),
                operator_phrase(&OperatorQuery::new(left, op).value(ValueShape::DateOnly)),
                "operator_for_date {left} {op}"
            );
            assert_eq!(
                operator_with(&w, &p, left, op),
                operator_phrase(&OperatorQuery::new(left, op).wording(&w, &p)),
                "operator_with {left} {op}"
            );
            assert_eq!(
                operator_with_date(&w, &p, left, op),
                operator_phrase(
                    &OperatorQuery::new(left, op)
                        .value(ValueShape::DateOnly)
                        .wording(&w, &p)
                ),
                "operator_with_date {left} {op}"
            );
        }
    }
    // Every shape is accepted.
    for s in SHAPES {
        let _ = operator_phrase(&OperatorQuery::new("dateTime", "lt").value(s));
    }
}
