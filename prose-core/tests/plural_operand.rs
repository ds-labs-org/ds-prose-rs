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
