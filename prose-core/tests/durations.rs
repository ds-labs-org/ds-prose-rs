//! A duration right operand reads as English (#47): `PT24H` is "24 hours",
//! `P1Y6M` is "1 year and 6 months". A plain string that is strictly an
//! ISO 8601 / xsd:duration, or a literal typed xsd:duration (however the
//! datatype IRI is spelled), is worded; anything else stays as written.
use prose_core::edit::{Segment, read_model, sentences};
use prose_core::render;
use serde_json::{Value, json};

const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

/// A Set with one permission limited by `elapsedTime lteq <right>`.
fn policy(right: Value) -> Value {
    json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"x": XSD}],
        "@type": "Set",
        "uid": "urn:policy:d",
        "permission": [{
            "target": "urn:asset:1",
            "action": "use",
            "constraint": [{"leftOperand": "elapsedTime", "operator": "lteq", "rightOperand": right}]
        }]
    })
}

/// The condition's text, as `render` reads it.
fn reads(right: Value) -> String {
    let doc = render(&policy(right).to_string()).unwrap();
    assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
    doc.policies[0].rules[0].conditions[0].text.clone()
}

fn at_most(value: &str) -> String {
    format!("the elapsed time is at most {value}")
}

fn typed(value: &str, datatype: &str) -> Value {
    json!({"@value": value, "@type": datatype})
}

#[test]
fn catalog_case_reads_hours() {
    // The compliance case the catalog shows: `elapsedTime lteq "PT24H"^^xsd:duration`.
    assert_eq!(reads(typed("PT24H", "xsd:duration")), at_most("24 hours"));
}

#[test]
fn every_spelling_of_the_datatype() {
    for dt in [
        "xsd:duration".to_string(),
        "xs:duration".to_string(),
        "x:duration".to_string(),
        format!("{XSD}duration"),
    ] {
        assert_eq!(reads(typed("P30D", &dt)), at_most("30 days"), "{dt}");
    }
}

#[test]
fn plain_string_durations() {
    for (written, worded) in [
        ("P30D", "30 days"),
        ("P1D", "1 day"),
        ("PT24H", "24 hours"),
        ("PT1H", "1 hour"),
        ("PT10H", "10 hours"),
        ("P1Y", "1 year"),
        ("P2Y", "2 years"),
        ("P1M", "1 month"),
        ("P60M", "60 months"),
        ("PT1M", "1 minute"),
        ("PT90M", "90 minutes"),
        ("PT1S", "1 second"),
        ("PT45S", "45 seconds"),
        ("P1W", "1 week"),
        ("P2W", "2 weeks"),
        ("P1W2D", "1 week and 2 days"),
        ("P1Y6M", "1 year and 6 months"),
        ("PT2H30M", "2 hours and 30 minutes"),
        ("P1DT12H", "1 day and 12 hours"),
        (
            "P1Y2M10DT2H30M",
            "1 year, 2 months, 10 days, 2 hours and 30 minutes",
        ),
        (
            "P1Y2M3DT4H5M6S",
            "1 year, 2 months, 3 days, 4 hours, 5 minutes and 6 seconds",
        ),
    ] {
        assert_eq!(reads(json!(written)), at_most(worded), "{written}");
    }
}

#[test]
fn zero_parts_and_leading_zeros() {
    for (written, worded) in [
        ("P0D", "0 days"),
        ("PT0S", "0 seconds"),
        ("P0Y0M", "0 months"),
        ("P1YT0S", "1 year"),
        ("P007D", "7 days"),
        ("P01D", "1 day"),
        // The W3C vocabulary's example7, as a duration.
        ("P2011Y06M28DT00H00M00S", "2011 years, 6 months and 28 days"),
    ] {
        assert_eq!(reads(json!(written)), at_most(worded), "{written}");
    }
}

#[test]
fn fractional_seconds_are_kept() {
    for (written, worded) in [
        ("PT0.5S", "0.5 seconds"),
        ("PT1.25S", "1.25 seconds"),
        ("PT1.0S", "1.0 seconds"),
        ("PT1M30.5S", "1 minute and 30.5 seconds"),
    ] {
        assert_eq!(reads(json!(written)), at_most(worded), "{written}");
    }
}

#[test]
fn negative_durations() {
    assert_eq!(reads(json!("-P3D")), at_most("minus 3 days"));
    assert_eq!(
        reads(typed("-PT1H30M", "xsd:duration")),
        at_most("minus 1 hour and 30 minutes")
    );
}

#[test]
fn not_a_duration_stays_as_written() {
    // Plain strings are quoted as before; nothing is guessed.
    for written in [
        "P", "PT", "P30DX", "P1DT", "P1H", "PT1D", "P1.5Y", "PT1.5M", "P1M1Y", "P1Y1Y", "P-1D",
        "p30d", "P30d", "30D", "P 30D", "P1D T1H", "+P1D", "--P1D", "PT.5S", "PT1.S", "P1W1W",
        "P1D1W", "PTH", "PT1H ", " PT1H",
    ] {
        let want = format!("\"{written}\"");
        assert_eq!(reads(json!(written)), at_most(&want), "{written}");
    }
    // A typed duration that does not parse keeps its text.
    assert_eq!(reads(typed("PT", "xsd:duration")), at_most("PT"));
    assert_eq!(reads(typed("P30DX", "xsd:duration")), at_most("P30DX"));
}

#[test]
fn another_datatype_is_not_read_as_a_duration() {
    // The W3C vocabulary's example7 types "P30D" as a dateTime.
    assert_eq!(reads(typed("P30D", "xsd:dateTime")), at_most("P30D"));
    assert_eq!(reads(typed("PT24H", "xsd:string")), at_most("PT24H"));
    assert_eq!(
        reads(typed("P1D", "http://example.com/dt#duration")),
        at_most("P1D")
    );
}

#[test]
fn several_durations_and_other_values() {
    let doc = json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Set",
        "uid": "urn:policy:d",
        "permission": [{
            "target": "urn:asset:1",
            "action": "use",
            "constraint": [
                {"leftOperand": "timeInterval", "operator": "isAnyOf", "rightOperand": ["P1D", "P7D"]},
                {"leftOperand": "delayPeriod", "operator": "eq", "rightOperand": "PT60M"},
                {"leftOperand": "meteredTime", "operator": "lteq",
                 "rightOperand": {"@value": "PT10H", "@type": "xsd:duration"}},
                {"leftOperand": "count", "operator": "lteq", "rightOperand": 30},
                {"leftOperand": "dateTime", "operator": "lt", "rightOperand": "2026-12-31"}
            ]
        }]
    });
    let r = render(&doc.to_string()).unwrap();
    let texts: Vec<&str> = r.policies[0].rules[0]
        .conditions
        .iter()
        .map(|c| c.text.as_str())
        .collect();
    assert_eq!(
        texts,
        [
            "the time interval is any of 1 day or 7 days",
            "the delay period is equal to 60 minutes",
            "the metered time is at most 10 hours",
            "the number of times used is at most 30",
            "the date and time are before the start of 2026-12-31",
        ]
    );
}

#[test]
fn a_duration_with_a_unit_keeps_the_unit() {
    let mut doc = policy(json!("P30D"));
    doc["permission"][0]["constraint"][0]["unit"] = json!("http://example.com/units#calendarTime");
    let r = render(&doc.to_string()).unwrap();
    assert_eq!(
        r.policies[0].rules[0].conditions[0].text,
        at_most("30 days calendar time")
    );
}

/// The editor's right-operand slot: raw text as written, display worded.
fn slot(right: Value) -> (String, String) {
    let doc = read_model(&policy(right).to_string()).unwrap();
    let p = &sentences(&doc)[0];
    let c = &p.rules[0].conditions.items[0];
    for seg in &c.sentence.segments {
        if let Segment::List(l) = seg {
            let s = &l.items[0].slot;
            return (s.raw.clone(), s.display.clone());
        }
    }
    panic!("no right operand list in {:?}", c.sentence)
}

#[test]
fn editor_slot_reads_the_same() {
    for (right, raw, display) in [
        (typed("PT24H", "xsd:duration"), "PT24H", "24 hours"),
        (typed("P1Y6M", "x:duration"), "P1Y6M", "1 year and 6 months"),
        (typed("P1D", &format!("{XSD}duration")), "P1D", "1 day"),
        (json!("PT2H30M"), "PT2H30M", "2 hours and 30 minutes"),
        (json!("P30DX"), "P30DX", "\"P30DX\""),
        (typed("PT", "xsd:duration"), "PT", "PT"),
        (typed("P30D", "xsd:dateTime"), "P30D", "P30D"),
    ] {
        assert_eq!(slot(right.clone()), (raw.into(), display.into()), "{right}");
    }
    let doc = read_model(&policy(json!("PT24H")).to_string()).unwrap();
    assert_eq!(
        sentences(&doc)[0].rules[0].conditions.items[0]
            .sentence
            .plain(),
        at_most("24 hours")
    );
}

#[test]
fn untyped_value_object_reads_like_a_plain_string() {
    // `{"@value": "P30D"}` (no @type, or only @language) is the same plain
    // string literal as `"P30D"`: render and the editor word it alike.
    for right in [
        json!({"@value": "P30D"}),
        json!({"@value": "P30D", "@language": "en"}),
    ] {
        assert_eq!(reads(right.clone()), at_most("30 days"), "{right}");
        assert_eq!(
            slot(right.clone()),
            ("P30D".into(), "30 days".into()),
            "{right}"
        );
        let doc = read_model(&policy(right.clone()).to_string()).unwrap();
        assert_eq!(
            sentences(&doc)[0].rules[0].conditions.items[0]
                .sentence
                .plain(),
            reads(right.clone()),
            "{right}"
        );
    }
    // Still strict: a non-duration stays quoted as written on both paths.
    let right = json!({"@value": "P30DX"});
    assert_eq!(reads(right.clone()), at_most("\"P30DX\""));
    assert_eq!(slot(right), ("P30DX".into(), "\"P30DX\"".into()));
}
