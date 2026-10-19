//! A date without a time of day is a whole day: a bound on it says whether it
//! means the start or the end of that day. A value with a time of day is an
//! instant and reads as before. The date is never rewritten.
use prose_core::edit::{read_model, sentences};
use prose_core::profile::{ProfileLimits, ProfileVocab, Wording};
use prose_core::{render, render_with};
use serde_json::{Value, json};

fn policy(constraint: Value) -> String {
    json!({
        "@context": {"@vocab": "http://www.w3.org/ns/odrl/2/",
                     "ex": "https://example.org/odrl/profile/research/",
                     "xsd": "http://www.w3.org/2001/XMLSchema#"},
        "@type": "Offer",
        "uid": "urn:p",
        "permission": [{"action": "use", "target": "urn:t", "constraint": [constraint]}]
    })
    .to_string()
}

fn atom(left: &str, op: &str, right: Value) -> String {
    policy(json!({"leftOperand": left, "operator": op, "rightOperand": right}))
}

fn condition(text: &str) -> String {
    render(text).unwrap().policies[0].rules[0].conditions[0]
        .text
        .clone()
}

/// The three spellings of the date-only value 2026-12-31.
fn date_spellings() -> Vec<Value> {
    vec![
        json!({"@value": "2026-12-31", "@type": "xsd:date"}),
        json!("2026-12-31"),
        json!({"@value": "2026-12-31"}),
    ]
}

#[test]
fn date_only_bounds_name_the_start_or_end_of_the_day() {
    let cases = [
        ("lt", "are before the start of"),
        ("lteq", "are on or before the end of"),
        ("gt", "are after the end of"),
        ("gteq", "are on or after the start of"),
    ];
    for (op, phrase) in cases {
        for v in date_spellings() {
            assert_eq!(
                condition(&atom("dateTime", op, v.clone())),
                format!("the date and time {phrase} 2026-12-31"),
                "{op} {v}"
            );
        }
    }
}

#[test]
fn a_time_of_day_keeps_the_instant() {
    for v in [
        json!({"@value": "2026-12-31T10:00:00Z", "@type": "xsd:dateTime"}),
        json!("2026-12-31T10:00:00Z"),
        json!({"@value": "2026-12-31T10:00:00Z", "@type": "xsd:date"}),
    ] {
        assert_eq!(
            condition(&atom("dateTime", "lt", v.clone())),
            "the date and time are before 2026-12-31T10:00:00Z",
            "{v}"
        );
    }
}

#[test]
fn a_dated_zone_is_kept_as_written() {
    assert_eq!(
        condition(&atom(
            "dateTime",
            "lt",
            json!({"@value": "2026-12-31+02:00", "@type": "xsd:date"})
        )),
        "the date and time are before the start of 2026-12-31+02:00"
    );
}

/// A plain string is a date only when it is exactly YYYY-MM-DD: nothing
/// before it, after it, or in the wrong places.
#[test]
fn a_plain_string_must_be_exactly_a_date() {
    for v in [
        "2026-12-319",
        "2026-12-3",
        "2026/12/31",
        "2026-12-31Z",
        "2026-12-31T",
    ] {
        let got = condition(&atom("dateTime", "lt", json!(v)));
        assert!(
            got.starts_with("the date and time are before "),
            "{v}: {got}"
        );
        assert!(!got.contains("the start of"), "{v}: {got}");
    }
}

#[test]
fn eq_and_neq_are_unchanged() {
    assert_eq!(
        condition(&atom("dateTime", "eq", json!("2026-12-31"))),
        "the date and time are equal to 2026-12-31"
    );
    assert_eq!(
        condition(&atom("dateTime", "neq", json!("2026-12-31"))),
        "the date and time are not equal to 2026-12-31"
    );
}

#[test]
fn other_temporal_operands() {
    assert_eq!(
        condition(&atom("timeInterval", "lt", json!("2026-12-31"))),
        "the time interval is before the start of 2026-12-31"
    );
    assert_eq!(
        condition(&atom("delayPeriod", "lt", json!("PT60M"))),
        "the delay period is before 60 minutes"
    );
}

#[test]
fn no_bound_without_one_date() {
    let by_ref = policy(json!({
        "leftOperand": "dateTime", "operator": "lt", "rightOperandReference": "urn:r"
    }));
    assert_eq!(
        condition(&by_ref),
        "the date and time are before the value at urn:r"
    );
    assert_eq!(
        condition(&atom("dateTime", "lt", json!(["2026-12-31", "2027-01-01"]))),
        "the date and time are before 2026-12-31 and 2027-01-01"
    );
    assert_eq!(
        condition(&atom("count", "lt", json!("2026-12-31"))),
        "the number of times used is less than 2026-12-31"
    );
}

#[test]
fn a_profile_label_keeps_the_singular() {
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
    let got = render_with(&atom("ex:dateTime", "lt", json!("2026-12-31")), &w)
        .unwrap()
        .document
        .policies[0]
        .rules[0]
        .conditions[0]
        .text
        .clone();
    assert_eq!(got, "the moment is before the start of 2026-12-31");
}

#[test]
fn editor_and_reader_agree_on_date_constraints() {
    let mut texts = Vec::new();
    for op in ["lt", "lteq", "gt", "gteq", "eq", "neq"] {
        for v in date_spellings() {
            texts.push(atom("dateTime", op, v));
        }
        texts.push(atom("dateTime", op, json!("2026-12-31T10:00:00Z")));
    }
    texts.push(atom("timeInterval", "lt", json!("2026-12-31")));
    texts.push(atom("delayPeriod", "lt", json!("PT60M")));
    texts.push(atom("count", "lt", json!("2026-12-31")));
    texts.push(atom("dateTime", "lt", json!(["2026-12-31", "2027-01-01"])));
    for text in texts {
        let model = read_model(&text).unwrap();
        let editor = sentences(&model)[0].rules[0].conditions.items[0]
            .sentence
            .plain();
        assert_eq!(editor, condition(&text), "{text}");
    }
}
