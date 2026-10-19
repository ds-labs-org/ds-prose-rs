//! "the date and time" is read as two nouns and takes the plural; the other
//! operands stay singular, and a profile's own label for an operand keeps the
//! singular.
use prose_core::profile::{ProfileLimits, ProfileVocab, Wording};
use prose_core::{render, render_with};
use serde_json::{Value, json};

fn offer(left: &str, op: &str, right: Value) -> String {
    json!({
        "@context": {"@vocab": "http://www.w3.org/ns/odrl/2/",
                     "ex": "https://example.org/odrl/profile/research/",
                     "xsd": "http://www.w3.org/2001/XMLSchema#"},
        "@type": "Offer",
        "uid": "urn:p",
        "permission": [{
            "action": "use", "target": "urn:t",
            "constraint": [{"leftOperand": left, "operator": op, "rightOperand": right}]
        }]
    })
    .to_string()
}

fn condition(text: &str) -> String {
    render(text).unwrap().policies[0].rules[0].conditions[0]
        .text
        .clone()
}

#[test]
fn the_date_and_time_takes_the_plural() {
    assert_eq!(
        condition(&offer("dateTime", "eq", json!("2026-12-31T10:00:00Z"))),
        "the date and time are equal to 2026-12-31T10:00:00Z"
    );
    assert_eq!(
        condition(&offer(
            "dateTime",
            "lt",
            json!({"@value": "2026-12-31T10:00:00Z", "@type": "xsd:dateTime"})
        )),
        "the date and time are before 2026-12-31T10:00:00Z"
    );
    assert_eq!(
        condition(&offer("count", "lteq", json!(5))),
        "the number of times used is at most 5"
    );
}

#[test]
fn a_profile_label_for_the_operand_keeps_the_singular() {
    let moment = ProfileVocab::from_value(
        &json!({
            "@context": {"odrl": "http://www.w3.org/ns/odrl/2/",
                         "ex": "https://example.org/odrl/profile/research/",
                         "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
                         "owl": "http://www.w3.org/2002/07/owl#"},
            "@graph": [
                {"@id": "https://example.org/moment", "@type": "owl:Ontology"},
                {"@id": "ex:dateTime", "@type": "odrl:LeftOperand", "rdfs:label": "Moment"}
            ]
        }),
        &ProfileLimits::default(),
    )
    .unwrap();
    let w = Wording::new().with_profile(moment);
    let text = offer(
        "ex:dateTime",
        "lt",
        json!({"@value": "2026-12-31T10:00:00Z", "@type": "xsd:dateTime"}),
    );
    let got = render_with(&text, &w).unwrap().document.policies[0].rules[0].conditions[0]
        .text
        .clone();
    assert_eq!(got, "the moment is before 2026-12-31T10:00:00Z");
}

/// A profile with operator labels of each leading verb, and a relabelled
/// `ex:dateTime`.
fn operator_profile() -> Wording {
    let profile = ProfileVocab::from_value(
        &json!({
            "@context": {"odrl": "http://www.w3.org/ns/odrl/2/",
                         "ex": "https://example.org/odrl/profile/research/",
                         "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
                         "owl": "http://www.w3.org/2002/07/owl#"},
            "@graph": [
                {"@id": "https://example.org/ops", "@type": "owl:Ontology"},
                {"@id": "ex:dateTime", "@type": "odrl:LeftOperand", "rdfs:label": "Moment"},
                {"@id": "ex:earlierThan", "@type": "odrl:Operator", "rdfs:label": "Earlier than"},
                {"@id": "ex:hasOverlap", "@type": "odrl:Operator", "rdfs:label": "Has overlap with"},
                {"@id": "ex:doesNotOverlap", "@type": "odrl:Operator", "rdfs:label": "Does not overlap"},
                {"@id": "ex:containsDay", "@type": "odrl:Operator", "rdfs:label": "Contains"},
                {"@id": "ex:matchesWindow", "@type": "odrl:Operator", "rdfs:label": "Matches window"},
                {"@id": "ex:areNear", "@type": "odrl:Operator", "rdfs:label": "Are near"}
            ]
        }),
        &ProfileLimits::default(),
    )
    .unwrap();
    Wording::new().with_profile(profile)
}

#[test]
fn a_profile_operator_label_agrees_with_the_date_and_time() {
    use prose_core::edit::{read_model, sentences_with};
    use prose_core::profile::Prefixes;
    use prose_core::wording::{OperatorQuery, operator_phrase};
    let w = operator_profile();
    let prefixes = Prefixes::of_document(
        &json!({"@context": {"ex": "https://example.org/odrl/profile/research/"}}),
    );
    let value = json!("2026-12-31T10:00:00Z");
    for (left, cases) in [
        (
            "dateTime",
            [
                ("ex:earlierThan", "are earlier than"),
                ("ex:hasOverlap", "have overlap with"),
                ("ex:doesNotOverlap", "do not overlap"),
                ("ex:containsDay", "contain"),
                ("ex:matchesWindow", "match window"),
                ("ex:areNear", "are near"),
            ],
        ),
        (
            "ex:dateTime",
            [
                ("ex:earlierThan", "is earlier than"),
                ("ex:hasOverlap", "has overlap with"),
                ("ex:doesNotOverlap", "does not overlap"),
                ("ex:containsDay", "contains"),
                ("ex:matchesWindow", "matches window"),
                ("ex:areNear", "are near"),
            ],
        ),
    ] {
        let subject = if left == "dateTime" {
            "the date and time"
        } else {
            "the moment"
        };
        for (op, phrase) in cases {
            let want = format!("{subject} {phrase} 2026-12-31T10:00:00Z");
            let text = offer(left, op, value.clone());
            let read = render_with(&text, &w).unwrap().document.policies[0].rules[0].conditions[0]
                .text
                .clone();
            assert_eq!(read, want, "reader {left} {op}");
            let doc = read_model(&text).unwrap();
            assert_eq!(
                sentences_with(&doc, &w)[0].rules[0].conditions.items[0]
                    .sentence
                    .plain(),
                want,
                "editor {left} {op}"
            );
            assert_eq!(
                operator_phrase(&OperatorQuery::new(left, op).wording(&w, &prefixes)),
                phrase,
                "{left} {op}"
            );
        }
    }
}

#[test]
fn operator_with_date_words_a_day_bound_and_agrees_with_a_profile_label() {
    use prose_core::profile::Prefixes;
    use prose_core::wording::{OperatorQuery, ValueShape, operator_phrase};
    let w = operator_profile();
    let prefixes = Prefixes::of_document(
        &json!({"@context": {"ex": "https://example.org/odrl/profile/research/"}}),
    );
    // A built-in phrase with a date-only value names the start of the day.
    assert_eq!(
        operator_phrase(
            &OperatorQuery::new("dateTime", "lt")
                .value(ValueShape::DateOnly)
                .wording(&w, &prefixes)
        ),
        "are before the start of"
    );
    assert_eq!(
        operator_phrase(&OperatorQuery::new("dateTime", "lt").wording(&w, &prefixes)),
        "are before"
    );
    // A duration left operand takes the quantity wording whatever the value.
    assert_eq!(
        operator_phrase(
            &OperatorQuery::new("timeInterval", "lt")
                .value(ValueShape::DateOnly)
                .wording(&w, &prefixes)
        ),
        "is less than"
    );
    // A profile label keeps its own words and still agrees in number.
    assert_eq!(
        operator_phrase(
            &OperatorQuery::new("dateTime", "ex:earlierThan")
                .value(ValueShape::DateOnly)
                .wording(&w, &prefixes)
        ),
        "are earlier than"
    );
}
